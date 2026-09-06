//! Local provenance only. The caller owns transport and persistence; this module
//! never selects a project, contacts a server or writes a database.

use serde::{Deserialize, Serialize};

mod platform;
#[cfg(any(windows, target_os = "linux"))]
mod smbios;

pub const CONTRACT_VERSION: &str = "0.1.0";
pub const MANIFEST: &str =
    include_str!("../../../contracts/modules/workstation-identity.module.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySnapshot {
    pub contract_version: String,
    pub workstation_name: Option<String>,
    /// Local OS account name, not a network authentication identity.
    pub user_name: Option<String>,
    pub hardware_serial: Option<HardwareSerial>,
    pub issues: Vec<ReadIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareSerial {
    pub kind: SerialKind,
    pub value: String,
    pub source: IdentitySource,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SerialKind {
    Device,
    Processor,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentitySource {
    OsHostname,
    OsAccount,
    SmbiosSystemSerial,
    SmbiosProcessorSerial,
    LinuxDmiSystemSerial,
    DeviceTreeSerial,
    IokitPlatformSerial,
    UnsupportedPlatform,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityField {
    WorkstationName,
    UserName,
    HardwareSerial,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadFailure {
    Unavailable,
    PermissionDenied,
    InvalidValue,
    ReadFailed,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadIssue {
    pub field: IdentityField,
    pub source: IdentitySource,
    pub reason: ReadFailure,
}

pub(crate) type Reading = Result<String, ReadFailure>;
pub(crate) type SerialCandidate = (SerialKind, IdentitySource, Reading);

/// A read-only local call. Partial results retain explicit failure reasons.
/// A remote DB endpoint must store this client result, not recollect its own identity.
pub fn read_local_identity() -> IdentitySnapshot {
    collect(
        hostname::get()
            .map_err(io_failure)
            .and_then(|name| name.into_string().map_err(|_| ReadFailure::InvalidValue)),
        whoami::fallible::username().map_err(io_failure),
        platform::serial_candidates(),
    )
}

fn collect(name: Reading, user: Reading, candidates: Vec<SerialCandidate>) -> IdentitySnapshot {
    let mut snapshot = IdentitySnapshot {
        contract_version: CONTRACT_VERSION.into(),
        workstation_name: None,
        user_name: None,
        hardware_serial: None,
        issues: Vec::new(),
    };
    snapshot.workstation_name = field_value(
        name,
        IdentityField::WorkstationName,
        IdentitySource::OsHostname,
        &mut snapshot.issues,
    );
    snapshot.user_name = field_value(
        user,
        IdentityField::UserName,
        IdentitySource::OsAccount,
        &mut snapshot.issues,
    );
    // Device identity has priority even if a provider returned CPU records first.
    for kind in [SerialKind::Device, SerialKind::Processor] {
        for (candidate_kind, source, reading) in &candidates {
            if *candidate_kind != kind {
                continue;
            }
            let result = reading.clone().and_then(normalize_serial);
            match result {
                Ok(value) => {
                    snapshot.hardware_serial = Some(HardwareSerial {
                        kind,
                        value,
                        source: *source,
                    });
                    return snapshot;
                }
                Err(reason) => snapshot.issues.push(ReadIssue {
                    field: IdentityField::HardwareSerial,
                    source: *source,
                    reason,
                }),
            }
        }
    }
    if candidates.is_empty() {
        snapshot.issues.push(ReadIssue {
            field: IdentityField::HardwareSerial,
            source: IdentitySource::UnsupportedPlatform,
            reason: ReadFailure::Unsupported,
        });
    }
    snapshot
}

fn field_value(
    reading: Reading,
    field: IdentityField,
    source: IdentitySource,
    issues: &mut Vec<ReadIssue>,
) -> Option<String> {
    match reading.and_then(normalize_text) {
        Ok(value) => Some(value),
        Err(reason) => {
            issues.push(ReadIssue {
                field,
                source,
                reason,
            });
            None
        }
    }
}

fn normalize_text(value: String) -> Reading {
    let value = value.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if value.is_empty() {
        return Err(ReadFailure::Unavailable);
    }
    if value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(ReadFailure::InvalidValue);
    }
    Ok(value.into())
}

pub(crate) fn normalize_serial(value: String) -> Reading {
    let value = normalize_text(value)?;
    let key: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    if key.is_empty()
        || matches!(
            key.as_str(),
            "none"
                | "null"
                | "unknown"
                | "na"
                | "notavailable"
                | "notspecified"
                | "notapplicable"
                | "defaultstring"
                | "systemserialnumber"
                | "serialnumber"
                | "tobefilledbyoem"
                | "oem"
        )
        || key.chars().all(|c| c == '0')
        || key.chars().all(|c| c == 'f')
    {
        return Err(ReadFailure::InvalidValue);
    }
    // Preserve the manufacturer's spelling/case; normalization is only for rejection.
    Ok(value)
}

pub(crate) fn io_failure(error: std::io::Error) -> ReadFailure {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => ReadFailure::PermissionDenied,
        std::io::ErrorKind::NotFound => ReadFailure::Unavailable,
        std::io::ErrorKind::Unsupported => ReadFailure::Unsupported,
        _ => ReadFailure::ReadFailed,
    }
}

#[cfg(test)]
mod tests;
