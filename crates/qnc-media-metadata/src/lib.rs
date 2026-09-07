//! Portable media facts and side-effect-free validation, not a probe executor.

mod model;
mod validation;

pub use model::*;
pub use qnc_frame_timebase::FrameTimebase;
pub use validation::{inspect, IssueCode, MetadataIssue, MetadataReport};

pub const CONTRACT_ID: &str = "qnc.media.metadata";
pub const CONTRACT_VERSION: &str = "0.2.0";
