use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SaveState {
    Pending,
    Failed,
    #[default]
    Saved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThumbStatus {
    Ready,
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ClipFilter {
    New,
    #[default]
    All,
}
