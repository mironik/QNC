use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportStatus {
    Empty,
    Preparing,
    Ready,
    Playing,
    Paused,
    Stopped,
}
