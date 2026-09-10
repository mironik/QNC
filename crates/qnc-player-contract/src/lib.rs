//! Neutral frame-based player data contract, separated from the executor.
pub mod envelope;
pub mod event;
pub mod interface;
pub mod model;
pub mod session;

pub use event::BroadcastEvent;
pub use interface::protocol::{
    BroadcastPlaybackRequest, BroadcastPlayerProtocolCommand, BroadcastPlayerProtocolEvent,
    map_broadcast_player_protocol_event, validate_broadcast_player_protocol_command,
    validate_broadcast_player_protocol_event,
};
pub use model::{
    AudioFormat, AudioRuntime, ColorSpace, FieldMode, FrameDelta, FrameNumber, FrameRange,
    PixelAspect, SourceRuntime, Timebase, TransportStatus, VideoFormat,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
