//! Pure composition of already acquired facts. Never opens media or invokes a producer.
use qnc_ffprobe_metadata::Parsed;
use qnc_media_metadata::*;
use qnc_media_records::{Phase, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub path: String,
    pub existing: Value,
    pub incoming: Value,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidInput,
    Finalized,
    DuplicateProbe,
    UnrelatedMedia,
    AmbiguousStream,
    Conflicts(Vec<Conflict>),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "metadata composition: {self:?}")
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// A finalized DB snapshot is never a new probe plan, even when it remains partial.
pub fn required_probes(snapshot: &Snapshot) -> Result<Vec<String>> {
    snapshot.validate().map_err(|_| Error::InvalidInput)?;
    if snapshot.phase == Phase::Final {
        return Err(Error::Finalized);
    }
    let report = inspect(&snapshot.metadata);
    Ok(std::iter::once(("original", &snapshot.metadata.original))
        .chain(snapshot.metadata.proxy.as_ref().map(|p| ("proxy", p)))
        .filter(|(prefix, _)| {
            report
                .issues
                .iter()
                .any(|i| i.code == IssueCode::Missing && i.path.starts_with(&format!("{prefix}.")))
        })
        .map(|(_, m)| m.media_uri.clone())
        .collect())
}

/// Conflicting facts are returned with both provenances. No partial mutation escapes on error.
pub fn compose(camera: &ClipMetadata, probes: &[Parsed]) -> Result<ClipMetadata> {
    qnc_media_records::inspect(camera, Phase::Camera).map_err(|_| Error::InvalidInput)?;
    if probes.len() > 2 {
        return Err(Error::DuplicateProbe);
    }
    let mut result = camera.clone();
    let mut seen = BTreeSet::new();
    let mut conflicts = Vec::new();
    for probe in probes {
        if !seen.insert(&probe.media.media_uri) {
            return Err(Error::DuplicateProbe);
        }
        let evidence = &probe.evidence;
        if evidence.kind != EvidenceKind::Ffprobe
            || evidence.media_uri != probe.media.media_uri
            || result.evidence.iter().any(|e| e.id == evidence.id)
        {
            return Err(Error::InvalidInput);
        }
        let validation = ClipMetadata {
            contract_id: CONTRACT_ID.into(),
            contract_version: CONTRACT_VERSION.into(),
            clip_id: camera.clip_id.clone(),
            evidence: vec![evidence.clone()],
            original: probe.media.clone(),
            proxy: None,
        };
        qnc_media_records::inspect(&validation, Phase::Final).map_err(|_| Error::InvalidInput)?;
        let target = if result.original.media_uri == probe.media.media_uri {
            &mut result.original
        } else {
            result
                .proxy
                .as_mut()
                .filter(|p| p.media_uri == probe.media.media_uri)
                .ok_or(Error::UnrelatedMedia)?
        };
        merge_media(target, &probe.media, &mut conflicts)?;
        result.evidence.push(evidence.clone());
    }
    if !conflicts.is_empty() {
        return Err(Error::Conflicts(conflicts));
    }
    qnc_media_records::inspect(&result, Phase::Final).map_err(|_| Error::InvalidInput)?;
    Ok(result)
}

fn merge_media(
    camera: &mut MediaRepresentation,
    probe: &MediaRepresentation,
    conflicts: &mut Vec<Conflict>,
) -> Result<()> {
    let mut merged = to_value(camera)?;
    let mut incoming = to_value(probe)?;
    let mut streams = probe.streams.clone();
    let mut claimed = BTreeSet::new();
    for old in &camera.streams {
        let candidates: Vec<_> = streams
            .iter()
            .enumerate()
            .filter(|(_, new)| match &old.index {
                Some(index) => new.index.as_ref().is_some_and(|i| i.value == index.value),
                None => kind(old) == kind(new),
            })
            .map(|(i, _)| i)
            .collect();
        if candidates.len() != 1 {
            return Err(Error::AmbiguousStream);
        }
        let i = candidates[0];
        if !claimed.insert(i) || kind(old) != kind(&streams[i]) {
            return Err(Error::AmbiguousStream);
        }
        let mut value = to_value(old)?;
        merge_node(
            &mut value,
            &to_value(&streams[i])?,
            &format!("{}/streams/{i}", camera.media_uri),
            "stream",
            conflicts,
        );
        streams[i] = serde_json::from_value(value).map_err(|_| Error::InvalidInput)?;
    }
    if camera.streams_complete.as_ref().is_some_and(|f| f.value) && claimed.len() != streams.len() {
        return Err(Error::AmbiguousStream);
    }
    merged
        .as_object_mut()
        .ok_or(Error::InvalidInput)?
        .remove("streams");
    incoming
        .as_object_mut()
        .ok_or(Error::InvalidInput)?
        .remove("streams");
    merge_node(
        &mut merged,
        &incoming,
        &camera.media_uri,
        "media",
        conflicts,
    );
    merged["streams"] = to_value(&streams)?;
    *camera = serde_json::from_value(merged).map_err(|_| Error::InvalidInput)?;
    Ok(())
}
fn kind(s: &MediaStream) -> &str {
    match &s.details {
        StreamDetails::Video(_) => "video",
        StreamDetails::Audio(_) => "audio",
        StreamDetails::Other { stream_type } => &stream_type.value,
    }
}
fn to_value(value: &impl Serialize) -> Result<Value> {
    serde_json::to_value(value).map_err(|_| Error::InvalidInput)
}

// Work on the serialized Fact schema, not field-name string rewrites. Each Fact is atomic:
// its value and evidence locator must move together, including when missing data is filled.
fn merge_node(
    old: &mut Value,
    new: &Value,
    path: &str,
    field: &str,
    conflicts: &mut Vec<Conflict>,
) {
    if new.is_null() {
        return;
    }
    if old.is_null() {
        *old = new.clone();
        return;
    }
    if old.get("evidence_id").is_some() && new.get("evidence_id").is_some() {
        let a = &old["value"];
        let b = &new["value"];
        if equivalent(a, b, field) {
            return;
        }
        if field == "frame_rate_mode" {
            if b == "unknown" {
                return;
            }
            if a == "unknown" {
                *old = new.clone();
                return;
            }
        }
        if field == "streams_complete" && a == false && b == true {
            *old = new.clone();
            return;
        }
        if field == "frame_count" && a["accuracy"] == "exact" && b["accuracy"] == "estimated" {
            return;
        }
        if field == "frame_count" && a["accuracy"] == "estimated" && b["accuracy"] == "exact" {
            *old = new.clone();
            return;
        }
        if a["state"] == "known" && b["state"] == "unspecified" {
            return;
        }
        if a["state"] == "unspecified" && b["state"] == "known" {
            *old = new.clone();
            return;
        }
        conflicts.push(Conflict {
            path: path.into(),
            existing: old.clone(),
            incoming: new.clone(),
        });
        return;
    }
    if let (Some(a), Some(b)) = (old.as_object_mut(), new.as_object()) {
        for (key, value) in b {
            merge_node(
                a.entry(key.clone()).or_insert(Value::Null),
                value,
                &format!("{path}/{key}"),
                key,
                conflicts,
            );
        }
    } else if old != new {
        conflicts.push(Conflict {
            path: path.into(),
            existing: old.clone(),
            incoming: new.clone(),
        });
    }
}
fn equivalent(a: &Value, b: &Value, field: &str) -> bool {
    if a == b {
        return true;
    }
    for (n, d) in [("numerator", "denominator"), ("fps_num", "fps_den")] {
        if let (Some(an), Some(ad), Some(bn), Some(bd)) =
            (a[n].as_i64(), a[d].as_i64(), b[n].as_i64(), b[d].as_i64())
        {
            return ad > 0
                && bd > 0
                && i128::from(an) * i128::from(bd) == i128::from(bn) * i128::from(ad);
        }
    }
    if field == "container" {
        if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
            return b.split(',').any(|alias| alias == a);
        }
    }
    false
}

#[cfg(test)]
mod tests;
