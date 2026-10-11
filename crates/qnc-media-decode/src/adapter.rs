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

/// A decoder that works inside the calling process (no process start, no pipe): it gives
/// the same records and bytes a decoder process gives, one decoded packet at a time, and
/// the same checks hold for them (user 2026-10-11: the decoder kept in the player).
pub trait PictureSource: Send {
    /// The next decoded packet, `None` at the end of the request.
    fn next(&mut self) -> Result<Option<(PacketHeader, Vec<u8>)>>;
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
    /// The decoder of this request inside the calling process, when the adapter has one
    /// for it; `None` launches the decoder process.
    fn open_in_process(
        &self,
        _request: &DecodeRequest,
        _plan: &DecodePlan,
        _endpoint: &CodecEndpoint,
    ) -> Option<Result<Box<dyn PictureSource>>> {
        None
    }
}
