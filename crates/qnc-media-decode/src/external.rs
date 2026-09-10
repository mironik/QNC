//! Versioned process protocol for independently installed decoder adapters.
use crate::*;
use qnc_media_stream::HttpEndpoint;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Command};

pub const PROCESS_PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Clone)]
pub struct ExternalAdapter {
    pub adapter_id: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessOpen {
    pub protocol_version: String,
    pub adapter_id: String,
    pub request_id: String,
    pub request: DecodeRequest,
    pub expected_format: DecodedFormat,
    pub media_url: String,
    pub authorization: String,
    pub storage_stamp: String,
    pub max_packet_bytes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProcessRecord {
    Ready {
        protocol_version: String,
        adapter_id: String,
        request_id: String,
        format: DecodedFormat,
    },
    Packet {
        header: PacketHeader,
    },
    Error {
        message: String,
    },
}

impl DecoderAdapter for ExternalAdapter {
    fn validate(&self, _: &DecodeRequest, _: &DecodePlan) -> Result<()> {
        if self.adapter_id.is_empty()
            || self.adapter_id.len() > 128
            || self.executable.as_os_str().is_empty()
            || self.args.len() > 32
            || self.args.iter().any(|s| s.len() > 4096 || s.contains('\0'))
        {
            return Err(DecodeError::new(
                ErrorKind::Contract,
                "invalid external adapter",
            ));
        }
        Ok(())
    }
    fn launch(
        &self,
        request: &DecodeRequest,
        plan: &DecodePlan,
        endpoint: &HttpEndpoint,
        stamp: &str,
    ) -> Result<ProcessLaunch> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let open = ProcessOpen {
            protocol_version: PROCESS_PROTOCOL_VERSION.into(),
            adapter_id: self.adapter_id.clone(),
            request_id: request_id.clone(),
            request: request.clone(),
            expected_format: plan.format.clone(),
            media_url: endpoint.url().into(),
            authorization: endpoint.authorization_header().into(),
            storage_stamp: stamp.into(),
            max_packet_bytes: plan.max_packet,
        };
        let mut input = serde_json::to_vec(&open)
            .map_err(|_| DecodeError::new(ErrorKind::Contract, "cannot encode decoder request"))?;
        input.push(b'\n');
        let mut command = Command::new(&self.executable);
        command.args(&self.args).arg("--qnc-decode-v1");
        Ok(ProcessLaunch {
            command,
            input,
            records: Box::new(ExternalRecords {
                adapter_id: self.adapter_id.clone(),
                request_id,
                format: plan.format.clone(),
                ready: false,
            }),
        })
    }
}

struct ExternalRecords {
    adapter_id: String,
    request_id: String,
    format: DecodedFormat,
    ready: bool,
}
impl PacketRecordReader for ExternalRecords {
    fn parse(&mut self, line: &str) -> Result<Option<PacketHeader>> {
        let bad = || DecodeError::new(ErrorKind::Contract, "invalid decoder protocol or identity");
        match serde_json::from_str::<ProcessRecord>(line).map_err(|_| bad())? {
            ProcessRecord::Ready {
                protocol_version,
                adapter_id,
                request_id,
                format,
            } if !self.ready
                && protocol_version == PROCESS_PROTOCOL_VERSION
                && adapter_id == self.adapter_id
                && request_id == self.request_id
                && format == self.format =>
            {
                self.ready = true;
                Ok(None)
            }
            ProcessRecord::Packet { header } if self.ready => {
                header.validate()?;
                Ok(Some(header))
            }
            ProcessRecord::Error { .. } => Err(DecodeError::new(
                ErrorKind::Process,
                "external decoder reported an error",
            )),
            _ => Err(bad()),
        }
    }
    fn finish(&self) -> Result<()> {
        if self.ready {
            Ok(())
        } else {
            Err(DecodeError::new(
                ErrorKind::Contract,
                "missing decoder handshake",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reader() -> ExternalRecords {
        ExternalRecords {
            adapter_id: "vendor.decoder".into(),
            request_id: "session-1".into(),
            format: DecodedFormat::Video {
                width: 16,
                height: 16,
                pixel_format: "yuv420p".into(),
            },
            ready: false,
        }
    }
    fn ready() -> ProcessRecord {
        ProcessRecord::Ready {
            protocol_version: PROCESS_PROTOCOL_VERSION.into(),
            adapter_id: "vendor.decoder".into(),
            request_id: "session-1".into(),
            format: reader().format,
        }
    }
    fn line(record: &ProcessRecord) -> String {
        serde_json::to_string(record).unwrap()
    }
    #[test]
    fn packet_requires_identity_version_and_format_handshake() {
        let packet = ProcessRecord::Packet {
            header: PacketHeader {
                ordinal: 0,
                pts: -10,
                time_base: qnc_media_metadata::Rational {
                    numerator: 1,
                    denominator: 1000,
                },
                size: 384,
            },
        };
        assert!(reader().parse(&line(&packet)).is_err());
        assert!(reader().finish().is_err());
        let mut r = reader();
        assert!(r.parse(&line(&ready())).unwrap().is_none());
        assert_eq!(r.parse(&line(&packet)).unwrap().unwrap().pts, -10);
        r.finish().unwrap();
        assert!(r.parse(&line(&ready())).is_err());
        for field in ["protocol_version", "adapter_id", "request_id"] {
            let mut value = serde_json::to_value(ready()).unwrap();
            value[field] = "wrong".into();
            assert!(reader().parse(&value.to_string()).is_err());
        }
        let mut value = serde_json::to_value(ready()).unwrap();
        value["format"]["width"] = 32.into();
        assert!(reader().parse(&value.to_string()).is_err());
    }
    #[test]
    fn malformed_messages_fail_and_external_errors_do_not_leak_payload() {
        assert!(reader().parse("not json").is_err());
        let error = reader()
            .parse(&line(&ProcessRecord::Error {
                message: "secret credential".into(),
            }))
            .unwrap_err();
        assert!(!error.to_string().contains("secret"));
    }
}
