//! Player executor core. Concrete media, output and process adapters are separate.
pub mod engine_contract;
pub mod frame_clock;
pub mod transport_engine;

pub use engine_contract::{
    AudioFramePacket, AudioOutputAdapter, BroadcastEngineError, BroadcastEngineErrorKind,
    DecodedVideoFrame, EngineFrameRequest, EngineSourceHandle, FramePresenter, MonotonicScheduler,
    PlayoutFrame, PlayoutOutput, SourceOpenAdapter, VideoDecodeAdapter,
};
pub use frame_clock::{
    ClockTick, FrameClock, FrameClockConfig, FrameClockDirection, FrameClockRate, ScheduledFrame,
};
pub use qnc_player_contract::*;
pub use transport_engine::{SplitAvPlayoutOutput, TransportEngine, TransportEngineState};
