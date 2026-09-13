//! Public adapter from Broadcast Player state to passive Timeline projection.
//!
//! This module owns no UI form, database, source scan, probe, decode, clock,
//! player process, or application workflow.

use qnc_player_contract::{BroadcastPlayerProtocolEvent as PlayerEvent, envelope::EventEnvelope};
use qnc_timeline::TimelineProjection;

pub const MODULE_ID: &str = "qnc.module.player-timeline";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn projection_from_player_reply(reply: Option<&EventEnvelope>) -> TimelineProjection {
    let mut projection = TimelineProjection::default();
    let Some(reply) = reply else {
        return projection;
    };
    let mut has_timebase = false;
    for event in &reply.events {
        match event {
            PlayerEvent::CarrierPositionChanged {
                frame,
                range,
                timebase,
                ..
            } => {
                if let Some(range) = range {
                    projection.range_start_frame = range.start_frame;
                    projection.duration_frames = range.duration_frames().max(1);
                }
                has_timebase = timebase.is_some();
                projection.playhead_frame = Some(*frame);
            }
            PlayerEvent::ExecutionRangeChanged { range } => {
                if let Some(range) = range {
                    projection.range_start_frame = range.start_frame;
                    projection.duration_frames = range.duration_frames().max(1);
                }
            }
            PlayerEvent::PlaybackBoundaryReached { frame } => {
                projection.playhead_frame = Some(*frame);
            }
            PlayerEvent::ActiveSourceChanged { source_id: None } => {
                projection = TimelineProjection::default();
                has_timebase = false;
            }
            _ => {}
        }
    }
    projection.cue_enabled = projection.playhead_frame.is_some() && has_timebase;
    projection
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_player_contract::{
        BroadcastPlayerProtocolEvent as PlayerEvent, FrameRange, Timebase, TransportStatus,
        VERSION as PLAYER_CONTRACT_VERSION,
    };

    fn envelope(events: Vec<PlayerEvent>) -> EventEnvelope {
        EventEnvelope {
            contract_version: PLAYER_CONTRACT_VERSION.into(),
            session_id: "session".into(),
            source_generation: 3,
            sequence: 9,
            events,
        }
    }

    #[test]
    fn carrier_position_is_the_only_playhead_source_for_timeline_projection() {
        let projection = projection_from_player_reply(Some(&envelope(vec![
            PlayerEvent::PlaybackReadinessChanged {
                source_id: Some("clip".into()),
                frame: 77,
                ready: true,
            },
            PlayerEvent::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 12,
                range: Some(FrameRange::new(10, 60).unwrap()),
                timebase: Some(Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Playing,
            },
        ])));

        assert_eq!(projection.range_start(), 10);
        assert_eq!(projection.duration_frames(), 50);
        assert_eq!(projection.confirmed_frame(), Some(12));
        assert!(projection.can_cue());
    }

    #[test]
    fn readiness_without_carrier_does_not_create_timeline_playhead() {
        let projection = projection_from_player_reply(Some(&envelope(vec![
            PlayerEvent::PlaybackReadinessChanged {
                source_id: Some("clip".into()),
                frame: 77,
                ready: true,
            },
        ])));

        assert_eq!(projection.confirmed_frame(), None);
        assert!(!projection.can_cue());
    }

    #[test]
    fn player_error_keeps_timeline_cue_enabled_for_recovery() {
        let projection = projection_from_player_reply(Some(&envelope(vec![
            PlayerEvent::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 12,
                range: Some(FrameRange::new(10, 60).unwrap()),
                timebase: Some(Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Playing,
            },
            PlayerEvent::PlaybackError {
                message: "decode failed".into(),
            },
        ])));

        assert_eq!(projection.confirmed_frame(), Some(12));
        assert!(projection.can_cue());
    }
}
