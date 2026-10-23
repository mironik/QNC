use crate::*;

pub(crate) fn card_id(record: &SourceSelectionRecord) -> Option<String> {
    let serial = record.serial_number.trim();
    let volume = record.volume_name.trim();
    if serial.is_empty() && volume.is_empty() {
        return None;
    }
    let key = format!(
        "{}:{}:{}",
        record.source_kind.trim().to_ascii_lowercase(),
        serial.to_ascii_lowercase(),
        volume.to_ascii_lowercase()
    );
    Some(format!("card_{:016x}", fnv1a64(&key)))
}

pub(crate) fn session_id(source_uri: &str) -> String {
    let key = format!("{}:{}", source_uri, current_utc_nanos_text());
    format!("ingest_session_{:016x}", fnv1a64(&key))
}

pub(crate) fn current_utc_text() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

pub(crate) fn current_utc_nanos_text() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("unix_nanos:{nanos}")
}

pub(crate) fn fnv1a64(value: &str) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
