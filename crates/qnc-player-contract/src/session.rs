//! One bound player session. State queries do not advance command sequence.
use crate::{
    Timebase, VERSION,
    envelope::{CommandEnvelope, EventEnvelope},
};
use serde::{Deserialize, Serialize};

pub const ENDPOINT: &str = "/v1/player";
pub const MAX_CONTROL_BYTES: usize = 64 * 1024;
pub const FRAME_ENDPOINT: &str = "/v1/player/frame";
pub const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
pub const WIRE_SCHEME: &str = "qnc-player+tcp://";
pub const WIRE_MAGIC: &[u8; 8] = b"QNCPB01\0";
pub const WIRE_VERSION: u8 = 1;
pub const WIRE_KIND_CONTROL: u8 = 1;
pub const WIRE_KIND_FRAME: u8 = 2;
pub const WIRE_STATUS_OK: u8 = 0;
pub const WIRE_STATUS_ACCESS_DENIED: u8 = 1;
pub const WIRE_STATUS_BAD_REQUEST: u8 = 2;
pub const WIRE_STATUS_BUSY: u8 = 3;
pub const WIRE_STATUS_TOO_LARGE: u8 = 4;
pub const WIRE_STATUS_PROTOCOL: u8 = 5;
pub const WIRE_HEADER_BYTES: usize = 16;
pub const WIRE_MAX_TOKEN_BYTES: usize = 4096;

/// Binary response: u32 little-endian JSON header length, header, packed RGBA8 sRGB.
/// Empty body means no submitted picture; a zero header length means unchanged.
/// This is not a scanout acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorHeader {
    pub contract_version: String,
    pub session_id: String,
    pub source_generation: u64,
    pub output_generation: u64,
    pub sequence: u64,
    pub source_id: String,
    pub frame: u64,
    pub timebase: Timebase,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorQuery {
    pub session: SessionQuery,
    pub after: Option<(u64, u64)>,
}
impl MonitorHeader {
    pub fn validate(&self, query: &SessionQuery, source: &str, bytes: usize) -> Result<(), String> {
        query.validate_for(&self.session_id, self.source_generation)?;
        let expected = u64::from(self.width) * u64::from(self.height) * 4;
        if self.contract_version != crate::VERSION
            || self.output_generation == 0
            || self.source_id != source
            || source.is_empty()
            || self.width == 0
            || self.height == 0
            || expected != bytes as u64
            || bytes > MAX_FRAME_BYTES - 8196
        {
            return Err("invalid monitor frame".into());
        }
        Timebase::new(self.timebase.fps_num, self.timebase.fps_den)
            .map_err(|_| "invalid monitor frame timebase".to_string())?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionQuery {
    pub contract_version: String,
    pub session_id: String,
    pub source_generation: u64,
}
impl SessionQuery {
    pub fn validate_for(&self, session: &str, generation: u64) -> Result<(), String> {
        if self.contract_version != VERSION
            || self.session_id != session
            || session.trim().is_empty()
            || self.source_generation != generation
        {
            return Err("player session/version/generation mismatch".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SessionRequest {
    State(SessionQuery),
    Command(CommandEnvelope),
}
pub type SessionReply = Result<EventEnvelope, String>;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_cannot_switch_session_generation_or_version() {
        let query = SessionQuery {
            contract_version: VERSION.into(),
            session_id: "one".into(),
            source_generation: 1,
        };
        assert!(query.validate_for("one", 1).is_ok());
        assert!(query.validate_for("two", 1).is_err());
        assert!(query.validate_for("one", 2).is_err());
        let mut old = query.clone();
        old.contract_version = "0.2.0".into();
        assert!(old.validate_for("one", 1).is_err());
        let request = SessionRequest::State(query);
        assert_eq!(
            serde_json::from_str::<SessionRequest>(&serde_json::to_string(&request).unwrap())
                .unwrap(),
            request
        );
    }

    #[test]
    fn monitor_frame_requires_saved_source_timebase() {
        let query = SessionQuery {
            contract_version: VERSION.into(),
            session_id: "one".into(),
            source_generation: 1,
        };
        let mut header = MonitorHeader {
            contract_version: VERSION.into(),
            session_id: "one".into(),
            source_generation: 1,
            output_generation: 2,
            sequence: 3,
            source_id: "clip".into(),
            frame: 4,
            timebase: Timebase::new(50, 1).unwrap(),
            width: 2,
            height: 1,
        };
        assert!(header.validate(&query, "clip", 8).is_ok());
        header.timebase = Timebase {
            fps_num: 0,
            fps_den: 1,
        };
        assert!(header.validate(&query, "clip", 8).is_err());
    }
}
