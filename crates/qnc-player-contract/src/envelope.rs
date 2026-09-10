use serde::{Deserialize, Serialize};

use crate::{BroadcastPlayerProtocolCommand, BroadcastPlayerProtocolEvent, VERSION};

/// The receiver supplies its own session/generation/sequence; payloads cannot select it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub contract_version: String,
    pub session_id: String,
    pub request_id: String,
    pub source_generation: u64,
    pub sequence: u64,
    pub command: BroadcastPlayerProtocolCommand,
}

impl CommandEnvelope {
    pub fn validate_for(
        &self,
        session_id: &str,
        source_generation: u64,
        last_sequence: u64,
    ) -> Result<(), String> {
        validate_header(
            &self.contract_version,
            &self.session_id,
            session_id,
            self.sequence,
            last_sequence,
        )?;
        if self.request_id.trim().is_empty() || self.request_id.len() > 128 {
            return Err("request_id must contain 1-128 nonblank bytes".into());
        }
        if self.source_generation != source_generation {
            return Err("stale source generation".into());
        }
        self.command.validate()
    }
}

/// Ordered player output. A transport must not treat command acceptance as presentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub contract_version: String,
    pub session_id: String,
    pub source_generation: u64,
    pub sequence: u64,
    pub events: Vec<BroadcastPlayerProtocolEvent>,
}

impl EventEnvelope {
    pub fn validate_for(
        &self,
        session_id: &str,
        source_generation: u64,
        last_sequence: u64,
    ) -> Result<(), String> {
        validate_header(
            &self.contract_version,
            &self.session_id,
            session_id,
            self.sequence,
            last_sequence,
        )?;
        if self.source_generation != source_generation {
            return Err("stale source generation".into());
        }
        Ok(())
    }
}

fn validate_header(
    version: &str,
    actual_session: &str,
    session: &str,
    sequence: u64,
    last: u64,
) -> Result<(), String> {
    if version != VERSION {
        return Err("unsupported player contract version".into());
    }
    if session.trim().is_empty() || actual_session != session {
        return Err("player session mismatch".into());
    }
    if sequence <= last {
        return Err("duplicate or out-of-order player message".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CommandEnvelope {
        CommandEnvelope {
            contract_version: VERSION.into(),
            session_id: "session-one".into(),
            request_id: "request-one".into(),
            source_generation: 3,
            sequence: 2,
            command: BroadcastPlayerProtocolCommand::Play,
        }
    }

    #[test]
    fn command_round_trip_has_no_os_or_application_context() {
        let value = serde_json::to_value(request()).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 6);
        let decoded: CommandEnvelope = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, request());
        decoded.validate_for("session-one", 3, 1).unwrap();
    }

    #[test]
    fn wrong_session_stale_generation_replay_and_version_are_rejected() {
        assert!(request().validate_for("session-two", 3, 1).is_err());
        assert!(request().validate_for("session-one", 4, 1).is_err());
        assert!(request().validate_for("session-one", 3, 2).is_err());
        let mut bad = request();
        bad.contract_version = "future".into();
        assert!(bad.validate_for("session-one", 3, 1).is_err());
    }

    #[test]
    fn command_ack_identity_is_bounded_before_execution() {
        let mut oversized = request();
        oversized.request_id = "r".repeat(129);
        assert!(oversized.validate_for("session-one", 3, 1).is_err());
        oversized.request_id = "r".repeat(128);
        assert!(oversized.validate_for("session-one", 3, 1).is_ok());
    }

    #[test]
    fn extra_workflow_or_local_path_fields_are_rejected() {
        for field in ["path", "project_id", "active_tab", "ffprobe"] {
            let mut value = serde_json::to_value(request()).unwrap();
            value[field] = "unexpected".into();
            assert!(serde_json::from_value::<CommandEnvelope>(value).is_err());
        }
    }

    #[test]
    fn stale_output_and_cross_session_output_are_rejected() {
        let output = EventEnvelope {
            contract_version: VERSION.into(),
            session_id: "session-one".into(),
            source_generation: 3,
            sequence: 2,
            events: vec![BroadcastPlayerProtocolEvent::FramePresented { frame: 12 }],
        };
        output.validate_for("session-one", 3, 1).unwrap();
        assert!(output.validate_for("session-two", 3, 1).is_err());
        assert!(output.validate_for("session-one", 4, 1).is_err());
        assert!(output.validate_for("session-one", 3, 2).is_err());
    }

    #[test]
    fn readiness_round_trip_cannot_ready_another_clip_or_session() {
        use crate::{BroadcastEvent, TransportStatus, map_broadcast_player_protocol_event};

        let events = [
            BroadcastEvent::TransportStatusChanged {
                status: TransportStatus::Preparing,
            },
            BroadcastEvent::PlaybackReadinessChanged {
                source_id: Some("clip-from-db".into()),
                frame: 25,
                ready: false,
            },
            BroadcastEvent::PlaybackReadinessChanged {
                source_id: Some("clip-from-db".into()),
                frame: 25,
                ready: true,
            },
        ];
        let output = EventEnvelope {
            contract_version: VERSION.into(),
            session_id: "session-one".into(),
            source_generation: 3,
            sequence: 2,
            events: events
                .iter()
                .map(|event| map_broadcast_player_protocol_event(event).unwrap())
                .collect(),
        };
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json["events"][1]["PlaybackReadinessChanged"]["ready"],
            false
        );
        assert_eq!(json["events"][2]["PlaybackReadinessChanged"]["frame"], 25);
        assert_eq!(json["events"][2]["PlaybackReadinessChanged"]["ready"], true);
        let decoded: EventEnvelope = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, output);
        decoded.validate_for("session-one", 3, 1).unwrap();
        assert!(decoded.validate_for("session-one", 4, 1).is_err());
        assert!(decoded.validate_for("session-two", 3, 1).is_err());
        assert!(decoded.validate_for("session-one", 3, 2).is_err());
    }

    #[test]
    fn pre_readiness_protocol_is_rejected_without_compatibility_fallback() {
        let mut command = request();
        command.contract_version = "0.1.0".into();
        assert!(command.validate_for("session-one", 3, 1).is_err());
        let output = EventEnvelope {
            contract_version: "0.1.0".into(),
            session_id: "session-one".into(),
            source_generation: 3,
            sequence: 2,
            events: vec![BroadcastPlayerProtocolEvent::SourceReady {
                source_id: "src".into(),
            }],
        };
        assert!(output.validate_for("session-one", 3, 1).is_err());
    }

    #[test]
    fn submission_is_not_mapped_to_presentation_and_old_protocol_is_rejected() {
        use crate::{BroadcastEvent, map_broadcast_player_protocol_event};
        let mapped =
            map_broadcast_player_protocol_event(&BroadcastEvent::VideoFrameSubmitted { frame: 31 })
                .unwrap();
        assert_eq!(
            mapped,
            BroadcastPlayerProtocolEvent::VideoFrameSubmitted { frame: 31 }
        );
        let mut old = request();
        old.contract_version = "0.2.0".into();
        assert!(old.validate_for("session-one", 3, 1).is_err());
        let output = EventEnvelope {
            contract_version: VERSION.into(),
            session_id: "session-one".into(),
            source_generation: 3,
            sequence: 2,
            events: vec![mapped],
        };
        let decoded: EventEnvelope =
            serde_json::from_str(&serde_json::to_string(&output).unwrap()).unwrap();
        assert_eq!(decoded, output);
        decoded.validate_for("session-one", 3, 1).unwrap();
        assert!(decoded.validate_for("session-one", 4, 1).is_err());
    }
}
