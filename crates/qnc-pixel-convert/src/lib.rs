//! Saved pixel/color description + native raster bytes -> display pixels.
//! No clock, I/O, media discovery or application knowledge.
mod convert;
mod model;
mod preview;
pub use preview::{RasterConverter, RasterTiming, fit as fit_raster_size};

pub use convert::Converter;
pub use model::{ConversionError, ConversionSpec, PixelLayout, Range, Transfer, VERSION};

#[cfg(test)]
mod tests;
