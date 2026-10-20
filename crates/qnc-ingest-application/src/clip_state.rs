use super::*;

pub use qnc_clip_row::{SaveState, ThumbStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ClipFilter {
    New,
    #[default]
    All,
}
