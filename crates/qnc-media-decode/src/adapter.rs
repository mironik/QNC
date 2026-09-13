use crate::*;
use qnc_media_metadata::Rational;
use qnc_media_stream::CodecEndpoint;
use std::process::Command;

pub const MAX_RECORD_BYTES: usize = 4096;
pub const MAX_OPEN_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketHeader {
    pub ordinal: u64,
    pub pts: i64,
    pub time_base: Rational,
    pub size: usize,
}
impl PacketHeader {
    pub fn validate(&self) -> Result<()> {
        if self.pts == i64::MIN
            || self.pts == i64::MAX
            || self.time_base.numerator <= 0
            || self.time_base.denominator <= 0
            || self.size == 0
        {
            return Err(DecodeError::new(ErrorKind::Stream, "invalid packet header"));
        }
        Ok(())
    }
}

pub trait PacketRecordReader: Send {
    fn parse(&mut self, line: &str) -> Result<Option<PacketHeader>>;
    fn finish(&self) -> Result<()> {
        Ok(())
    }
}

pub struct ProcessLaunch {
    pub command: Command,
    /// Bounded initial request on stdin, then EOF. Never sent through a shell.
    pub input: Vec<u8>,
    pub records: Box<dyn PacketRecordReader>,
}

/// A decode adapter has no presentation clock, project policy or DB access.
pub trait DecoderAdapter: std::fmt::Debug + Send + Sync {
    fn validate(&self, request: &DecodeRequest, plan: &DecodePlan) -> Result<()>;
    fn launch(
        &self,
        request: &DecodeRequest,
        plan: &DecodePlan,
        endpoint: &CodecEndpoint,
        storage_stamp: &str,
    ) -> Result<ProcessLaunch>;
}
