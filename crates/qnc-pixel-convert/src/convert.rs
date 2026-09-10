use crate::{ConversionError, ConversionSpec, PixelLayout, Range, Transfer, model::MAX_BYTES};

pub struct Converter {
    spec: ConversionSpec,
    lut: [u8; 1024],
    native: Vec<u16>,
    rgb16: Vec<u16>,
}

impl Converter {
    pub fn prepare(
        spec: ConversionSpec,
        scratch_budget_bytes: usize,
    ) -> Result<Self, ConversionError> {
        spec.validate()?;
        if scratch_budget_bytes > MAX_BYTES || spec.scratch_bytes()? > scratch_budget_bytes {
            return Err(ConversionError::Budget);
        }
        let mut lut = [0; 1024];
        let max = if spec.layout.ten_bit() { 1023 } else { 255 };
        for (i, entry) in lut.iter_mut().enumerate().take(max + 1) {
            let encoded = i as f64 / max as f64;
            let srgb = match spec.transfer {
                Transfer::Srgb => encoded,
                Transfer::Bt709 => {
                    let linear = if encoded < 0.081 {
                        encoded / 4.5
                    } else {
                        ((encoded + 0.099) / 1.099).powf(1.0 / 0.45)
                    };
                    if linear <= 0.003_130_8 {
                        linear * 12.92
                    } else {
                        1.055 * linear.powf(1.0 / 2.4) - 0.055
                    }
                }
            };
            *entry = (srgb * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        let mut native = Vec::new();
        let mut rgb16 = Vec::new();
        if spec.layout.ten_bit() {
            let samples = spec.input_bytes()? / 2;
            native
                .try_reserve_exact(samples)
                .map_err(|_| ConversionError::Budget)?;
            native.resize(samples, 0);
            let elements = spec.output_bytes()?;
            rgb16
                .try_reserve_exact(elements)
                .map_err(|_| ConversionError::Budget)?;
            rgb16.resize(elements, 0);
        }
        Ok(Self {
            spec,
            lut,
            native,
            rgb16,
        })
    }

    pub fn spec(&self) -> &ConversionSpec {
        &self.spec
    }

    /// Both slices must have the exact negotiated length. No per-frame allocation.
    pub fn convert(&mut self, input: &[u8], rgba: &mut [u8]) -> Result<(), ConversionError> {
        if input.len() != self.spec.input_bytes()? || rgba.len() != self.spec.output_bytes()? {
            return Err(ConversionError::Payload);
        }
        let width = self.spec.width;
        let height = self.spec.height;
        let (cw, ch) = self.spec.layout.chroma_size(width, height);
        let y_len = width as usize * height as usize;
        let c_len = cw as usize * ch as usize;
        let range = match self.spec.range {
            Range::Limited => yuv::YuvRange::Limited,
            Range::Full => yuv::YuvRange::Full,
        };
        let matrix = yuv::YuvStandardMatrix::Bt709;
        if self.spec.layout.ten_bit() {
            for (sample, bytes) in self.native.iter_mut().zip(input.chunks_exact(2)) {
                let word = u16::from_le_bytes([bytes[0], bytes[1]]);
                if word > 1023 {
                    return Err(ConversionError::BitDepth);
                }
                *sample = word;
            }
            let image = planar(&self.native, width, height, cw, y_len, c_len);
            let convert = match self.spec.layout {
                PixelLayout::Yuv420p10le => yuv::i010_to_rgba10,
                PixelLayout::Yuv422p10le => yuv::i210_to_rgba10,
                PixelLayout::Yuv444p10le => yuv::i410_to_rgba10,
                _ => return Err(ConversionError::Unsupported("pixel_format")),
            };
            convert(&image, &mut self.rgb16, width * 4, range, matrix)
                .map_err(|e| ConversionError::Library(e.to_string()))?;
            if self.rgb16.iter().any(|v| *v > 1023) {
                return Err(ConversionError::BitDepth);
            }
            for (pixel, source) in rgba.chunks_exact_mut(4).zip(self.rgb16.chunks_exact(4)) {
                pixel[0] = self.lut[source[0] as usize];
                pixel[1] = self.lut[source[1] as usize];
                pixel[2] = self.lut[source[2] as usize];
                pixel[3] = 255;
            }
        } else {
            let image = planar(input, width, height, cw, y_len, c_len);
            let convert = match self.spec.layout {
                PixelLayout::Yuv420p => yuv::yuv420_to_rgba,
                PixelLayout::Yuv422p => yuv::yuv422_to_rgba,
                PixelLayout::Yuv444p => yuv::yuv444_to_rgba,
                _ => return Err(ConversionError::Unsupported("pixel_format")),
            };
            convert(&image, rgba, width * 4, range, matrix)
                .map_err(|e| ConversionError::Library(e.to_string()))?;
            for pixel in rgba.chunks_exact_mut(4) {
                pixel[0] = self.lut[pixel[0] as usize];
                pixel[1] = self.lut[pixel[1] as usize];
                pixel[2] = self.lut[pixel[2] as usize];
                pixel[3] = 255;
            }
        }
        Ok(())
    }
}

fn planar<T: Copy + std::fmt::Debug>(
    data: &[T],
    width: u32,
    height: u32,
    cw: u32,
    y_len: usize,
    c_len: usize,
) -> yuv::YuvPlanarImage<'_, T> {
    yuv::YuvPlanarImage {
        y_plane: &data[..y_len],
        y_stride: width,
        u_plane: &data[y_len..y_len + c_len],
        u_stride: cw,
        v_plane: &data[y_len + c_len..],
        v_stride: cw,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_frames_reuse_scratch_and_keep_monotonic_transfer() {
        let spec = crate::tests::spec(PixelLayout::Yuv422p10le);
        let mut converter =
            Converter::prepare(spec.clone(), spec.scratch_bytes().unwrap()).unwrap();
        let mut output = vec![0; spec.output_bytes().unwrap()];
        let input = crate::tests::gray(&spec, 512);
        let pointers = (converter.native.as_ptr(), converter.rgb16.as_ptr());
        for _ in 0..30 {
            converter.convert(&input, &mut output).unwrap();
            assert_eq!(
                pointers,
                (converter.native.as_ptr(), converter.rgb16.as_ptr())
            );
        }
        assert!(converter.lut.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(converter.lut[0], 0);
        assert_eq!(converter.lut[1023], 255);
    }
}
