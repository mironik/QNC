//! Neutral streaming decoder: saved facts and media bytes in, timed native packets out.
mod adapter;
mod external;
mod model;
mod plan;
mod process;
pub use adapter::*;
pub use external::*;
pub use model::*;
pub use plan::{DecodePlan, video_bytes};
pub use process::Decoder;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests;
