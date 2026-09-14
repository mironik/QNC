//! Public output device edge. No clock, media decode, DB or application workflow.
mod gpu;
mod model;

pub use gpu::{PreparedFrame, VideoOutput};
pub use model::{
    FrameHeader, GpuCompletion, MAX_OUTPUT_SLOTS, MAX_POOL_BYTES, OutputConfig, OutputError,
    PixelFormat, Submission, SubmissionTarget, VERSION,
};

#[cfg(test)]
mod tests;
