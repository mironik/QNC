use crate::{ConversionError, ConversionSpec, Converter};
use fast_image_resize::{
    FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer,
    images::{Image, ImageRef},
};
use std::time::Instant;

#[derive(Debug, Default, Clone, Copy)]
pub struct RasterTiming {
    pub resize_us: u128,
    pub color_us: u128,
}

/// Prepared raster conversion. Preview dimensions never replace source metadata.
pub struct RasterConverter {
    converter: Converter,
    size: [u32; 2],
    full_rgba: Vec<u8>,
    resizer: Resizer,
    source_spec: ConversionSpec,
    planar: Vec<u8>,
}

impl RasterConverter {
    pub fn prepare(
        mut converter: Converter,
        preview_bounds: Option<[u32; 2]>,
    ) -> Result<Self, ConversionError> {
        let source_spec = converter.spec().clone();
        let source = [source_spec.width, source_spec.height];
        let size = match preview_bounds {
            Some(bounds) => fit(source, bounds)?,
            None => source,
        };
        let mut full_rgba = Vec::new();
        let mut planar = Vec::new();
        if size != source && !source_spec.layout.ten_bit() {
            let mut preview_spec = source_spec.clone();
            preview_spec.width = size[0];
            preview_spec.height = size[1];
            let bytes = preview_spec.input_bytes()?;
            planar
                .try_reserve_exact(bytes)
                .map_err(|_| ConversionError::Budget)?;
            planar.resize(bytes, 0);
            converter = Converter::prepare(preview_spec.clone(), preview_spec.scratch_bytes()?)?;
        } else if size != source {
            let bytes = converter.spec().output_bytes()?;
            full_rgba
                .try_reserve_exact(bytes)
                .map_err(|_| ConversionError::Budget)?;
            full_rgba.resize(bytes, 0);
        }
        Ok(Self {
            converter,
            size,
            full_rgba,
            resizer: Resizer::new(),
            source_spec,
            planar,
        })
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }
    pub fn output_bytes(&self) -> usize {
        self.size[0] as usize * self.size[1] as usize * 4
    }

    pub fn convert(&mut self, input: &[u8], rgba: &mut [u8]) -> Result<(), ConversionError> {
        self.convert_timed(input, rgba).map(|_| ())
    }

    pub fn convert_timed(
        &mut self,
        input: &[u8],
        rgba: &mut [u8],
    ) -> Result<RasterTiming, ConversionError> {
        if rgba.len() != self.output_bytes() || input.len() != self.source_spec.input_bytes()? {
            return Err(ConversionError::Payload);
        }
        if !self.planar.is_empty() {
            let resize_start = Instant::now();
            let source_chroma = self
                .source_spec
                .layout
                .chroma_size(self.source_spec.width, self.source_spec.height);
            let dest_chroma = self
                .source_spec
                .layout
                .chroma_size(self.size[0], self.size[1]);
            let source_sizes = [
                (self.source_spec.width, self.source_spec.height),
                source_chroma,
                source_chroma,
            ];
            let dest_sizes = [(self.size[0], self.size[1]), dest_chroma, dest_chroma];
            let (y, uv) = input.split_at(source_sizes[0].0 as usize * source_sizes[0].1 as usize);
            let (u, v) = uv.split_at(source_chroma.0 as usize * source_chroma.1 as usize);
            let (dy, duv) = self
                .planar
                .split_at_mut(self.size[0] as usize * self.size[1] as usize);
            let (du, dv) = duv.split_at_mut(dest_chroma.0 as usize * dest_chroma.1 as usize);
            for (((input, output), (sw, sh)), (dw, dh)) in [y, u, v]
                .into_iter()
                .zip([dy, du, dv])
                .zip(source_sizes)
                .zip(dest_sizes)
            {
                let source = ImageRef::new(sw, sh, input, PixelType::U8)
                    .map_err(|e| ConversionError::Library(e.to_string()))?;
                let mut dest = Image::from_slice_u8(dw, dh, output, PixelType::U8)
                    .map_err(|e| ConversionError::Library(e.to_string()))?;
                self.resizer
                    .resize(
                        &source,
                        &mut dest,
                        &ResizeOptions::new()
                            .resize_alg(ResizeAlg::Convolution(FilterType::Bilinear)),
                    )
                    .map_err(|e| ConversionError::Library(e.to_string()))?;
            }
            let resize_us = resize_start.elapsed().as_micros();
            let color_start = Instant::now();
            self.converter.convert(&self.planar, rgba)?;
            return Ok(RasterTiming {
                resize_us,
                color_us: color_start.elapsed().as_micros(),
            });
        }
        if self.full_rgba.is_empty() {
            let color_start = Instant::now();
            self.converter.convert(input, rgba)?;
            return Ok(RasterTiming {
                resize_us: 0,
                color_us: color_start.elapsed().as_micros(),
            });
        }
        let color_start = Instant::now();
        self.converter.convert(input, &mut self.full_rgba)?;
        let color_us = color_start.elapsed().as_micros();
        let resize_start = Instant::now();
        let spec = self.converter.spec();
        let source = ImageRef::new(spec.width, spec.height, &self.full_rgba, PixelType::U8x4)
            .map_err(|e| ConversionError::Library(e.to_string()))?;
        let mut dest = Image::from_slice_u8(self.size[0], self.size[1], rgba, PixelType::U8x4)
            .map_err(|e| ConversionError::Library(e.to_string()))?;
        // Converted video is opaque; no alpha multiply/divide pass is needed.
        let options = ResizeOptions::new()
            .resize_alg(ResizeAlg::Convolution(FilterType::Bilinear))
            .use_alpha(false);
        self.resizer
            .resize(&source, &mut dest, &options)
            .map_err(|e| ConversionError::Library(e.to_string()))?;
        Ok(RasterTiming {
            resize_us: resize_start.elapsed().as_micros(),
            color_us,
        })
    }
}

pub fn fit(source: [u32; 2], bounds: [u32; 2]) -> Result<[u32; 2], ConversionError> {
    if source.contains(&0) || bounds.contains(&0) {
        return Err(ConversionError::Dimensions);
    }
    if source[0] <= bounds[0] && source[1] <= bounds[1] {
        return Ok(source);
    }
    let [w, h] = source.map(u64::from);
    let [bw, bh] = bounds.map(u64::from);
    if w * bh > h * bw {
        Ok([bounds[0], (h * bw / w).max(1) as u32])
    } else {
        Ok([(w * bh / h).max(1) as u32, bounds[1]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionSpec, PixelLayout, Range, Transfer, VERSION};

    fn converter(bounds: Option<[u32; 2]>) -> RasterConverter {
        let spec = ConversionSpec {
            version: VERSION.into(),
            width: 4,
            height: 2,
            layout: PixelLayout::Yuv420p,
            primaries: "bt709".into(),
            matrix: "bt709".into(),
            scan_mode: qnc_media_metadata::ScanMode::Progressive,
            range: Range::Limited,
            transfer: Transfer::Bt709,
        };
        RasterConverter::prepare(
            Converter::prepare(spec.clone(), spec.scratch_bytes().unwrap()).unwrap(),
            bounds,
        )
        .unwrap()
    }

    #[test]
    fn bounds_preserve_aspect_without_upscaling_or_cropping() {
        assert_eq!(fit([1920, 1080], [960, 540]).unwrap(), [960, 540]);
        assert_eq!(fit([1080, 1920], [960, 540]).unwrap(), [303, 540]);
        assert_eq!(fit([640, 480], [960, 540]).unwrap(), [640, 480]);
        assert!(fit([1920, 1080], [0, 540]).is_err());
    }

    #[test]
    fn preview_has_exact_payload_and_does_not_change_source_spec() {
        let mut preview = converter(Some([2, 2]));
        assert_eq!(preview.size(), [2, 1]);
        assert_eq!(preview.source_spec.width, 4);
        let input = [16, 16, 16, 16, 16, 16, 16, 16, 128, 128, 128, 128];
        let mut bytes = vec![0; preview.output_bytes()];
        preview.convert(&input, &mut bytes).unwrap();
        assert_eq!(bytes, [0, 0, 0, 255, 0, 0, 0, 255]);
        assert_eq!(
            preview.convert(&input, &mut [0; 32]),
            Err(ConversionError::Payload)
        );
    }

    #[test]
    fn native_raster_is_unchanged_and_needs_no_preview_scratch() {
        let mut native = converter(None);
        assert_eq!(native.size(), [4, 2]);
        assert!(native.full_rgba.is_empty());
        let mut bytes = vec![0; 32];
        native
            .convert(
                &[235, 235, 235, 235, 235, 235, 235, 235, 128, 128, 128, 128],
                &mut bytes,
            )
            .unwrap();
        assert!(bytes.iter().all(|v| *v == 255));
    }

    #[test]
    fn timing_does_not_change_pixels_and_native_has_no_resize() {
        let input = [235, 235, 235, 235, 235, 235, 235, 235, 128, 128, 128, 128];
        for bounds in [None, Some([2, 2])] {
            let mut raster = converter(bounds);
            let mut expected = vec![0; raster.output_bytes()];
            let mut measured = expected.clone();
            raster.convert(&input, &mut expected).unwrap();
            let timing = raster.convert_timed(&input, &mut measured).unwrap();
            assert_eq!(measured, expected);
            if bounds.is_none() {
                assert_eq!(timing.resize_us, 0);
            }
        }
    }

    #[test]
    fn reused_scratch_matches_fresh_conversion_for_each_layout() {
        for layout in [
            PixelLayout::Yuv420p,
            PixelLayout::Yuv422p,
            PixelLayout::Yuv444p,
        ] {
            let mut spec = crate::tests::spec(layout);
            spec.width = 322;
            spec.height = 182;
            let create = || {
                RasterConverter::prepare(
                    Converter::prepare(spec.clone(), spec.scratch_bytes().unwrap()).unwrap(),
                    Some([161, 91]),
                )
                .unwrap()
            };
            let mut a = create();
            let mut input = vec![0; spec.input_bytes().unwrap()];
            let mut expected = vec![0; a.output_bytes()];
            let mut actual = expected.clone();
            for pass in 0..3 {
                for (i, byte) in input.iter_mut().enumerate() {
                    *byte = ((i * 31 + pass * 17) % 256) as u8;
                }
                a.convert(&input, &mut actual).unwrap();
                create().convert(&input, &mut expected).unwrap();
                assert_eq!(expected, actual, "{layout:?}, pass {pass}");
            }
        }
    }
}
