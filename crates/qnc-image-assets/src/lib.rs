//! Bounded in-memory raster decoding. No transport, filesystem, database or media probes.
use std::{
    hash::{Hash, Hasher},
    io::Cursor,
};

pub const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub size: [usize; 2],
    pub pixels: Vec<u8>,
    /// Cache invalidation only, not a public media identity.
    pub content_key: u64,
}

pub fn decode_thumbnail(bytes: &[u8]) -> Result<RgbaImage, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("thumbnail input exceeds limit".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "unknown thumbnail encoding")?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)
    ) {
        return Err("unsupported thumbnail encoding".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| "invalid or oversized thumbnail")?
        .thumbnail(320, 180)
        .into_rgba8();
    let size = [decoded.width() as usize, decoded.height() as usize];
    let mut key = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut key);
    Ok(RgbaImage {
        size,
        pixels: decoded.into_raw(),
        content_key: key.finish(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_real_pixels_and_limits_output() {
        let input = image::RgbaImage::from_pixel(640, 360, image::Rgba([200, 40, 80, 255]));
        let mut bytes = Cursor::new(Vec::new());
        input.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let result = decode_thumbnail(bytes.get_ref()).unwrap();
        assert_eq!(result.size, [320, 180]);
        assert_eq!(&result.pixels[..4], &[200, 40, 80, 255]);
        assert_eq!(result.pixels.len(), 320 * 180 * 4);
        assert_eq!(
            decode_thumbnail(bytes.get_ref()).unwrap().content_key,
            result.content_key
        );
    }
    #[test]
    fn rejects_unknown_and_oversized_input() {
        assert!(decode_thumbnail(b"not an image").is_err());
        assert!(decode_thumbnail(&vec![0; MAX_BYTES + 1]).is_err());
    }
}
