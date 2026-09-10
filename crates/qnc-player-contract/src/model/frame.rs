use serde::{Deserialize, Serialize};

pub type FrameNumber = u64;
pub type FrameDelta = i64;

pub use qnc_frame_timebase::FrameTimebase as Timebase;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRange {
    pub start_frame: FrameNumber,
    pub end_frame: FrameNumber,
}

impl FrameRange {
    pub fn new(start_frame: FrameNumber, end_frame: FrameNumber) -> Result<Self, String> {
        if end_frame <= start_frame {
            return Err("range end_frame must be greater than start_frame".to_string());
        }
        Ok(Self {
            start_frame,
            end_frame,
        })
    }

    pub fn contains_position(self, frame: FrameNumber) -> bool {
        frame >= self.start_frame && frame < self.end_frame
    }

    pub fn contains_item(self, start_frame: FrameNumber, duration_frames: FrameNumber) -> bool {
        duration_frames > 0
            && start_frame >= self.start_frame
            && start_frame
                .checked_add(duration_frames)
                .is_some_and(|end| end <= self.end_frame)
    }

    pub fn duration_frames(self) -> FrameNumber {
        self.end_frame - self.start_frame
    }
}
