//! A voice record of the script as a clip of the project's catalog (user 2026-10-30: "clicking
//! an OFF plays its sound, so the editor can decide which take goes first"; every playback goes
//! through the Broadcast Player, which plays only clips of the catalog). The clip of kind `voice`
//! is the record's MXF in the voice folder of the project, described by what QNC wrote (piece
//! `qnc-voice-clip`), never probed; it is imported where it is and never queued for import. A
//! record with one more take replaces the one before under the same id.
//!
//! It writes through the one writer of the project's content database (`qnc-content-store`)
//! and knows no form or script.

use qnc_content_store::{CatalogClip, ContentTarget, ContentWriteTransport};
use qnc_media_metadata::ClipMetadata;
use qnc_media_records::{Binding, Completeness, Phase, Snapshot};
use qnc_work_settings::{SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.voice-catalog";

pub type Result<T> = std::result::Result<T, String>;

/// The catalog entry of a voice record called `name` whose clip `record` describes.
/// `source_uri` is the project folder as a media source, `project` the project's name.
pub fn entry(source_uri: &str, project: &str, name: &str, record: ClipMetadata, recorded_at_unix_ms: u64) -> Result<CatalogClip> {
    let report = qnc_media_records::inspect(&record, Phase::Final).map_err(|e| e.to_string())?;
    if !report.is_complete() {
        return Err(qnc_ui_text::text("voice.record_incomplete").into());
    }
    let snapshot = Snapshot {
        binding: Binding {
            source_index_uri: qnc_source_index_db::project::URI.into(),
            source_record_id: record.clip_id.clone(),
            original_uri: record.original.media_uri.clone(),
            proxy_uri: None,
        },
        revision: 1,
        phase: Phase::Final,
        completeness: Completeness::Complete,
        report,
        metadata: record,
        recorded_at_unix_ms,
    };
    Ok(CatalogClip {
        name: name.into(),
        source_uri: source_uri.into(),
        source_name: project.into(),
        serial_number: String::new(),
        volume_name: String::new(),
        thumbnail_uri: None,
        media_records_uri: qnc_media_record_db::project::URI.into(),
        snapshot,
    })
}

/// Writes the voice record `name` (its clip described by `record`) into the catalog of the
/// project of `settings` and waits for the writer. Call it off the form thread.
pub fn publish_record(reader: &SettingsReader, settings: &WorkSettings, name: &str, record: ClipMetadata) -> Result<()> {
    let source_uri = settings.project_media_source_uri().map_err(|e| e.message)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let clip = entry(&source_uri, &settings.project_name, name, record, now)?;
    publish(ContentTarget::for_project(reader, settings)?, clip)
}

/// Writes `clip` into the catalog of `target` and waits for the writer.
pub fn publish(target: ContentTarget, clip: CatalogClip) -> Result<()> {
    let mut transport = ContentWriteTransport::start(target)?;
    let key = format!("voice-{}", clip.id());
    transport.publish_voice(key.clone(), clip)?;
    loop {
        for completion in transport.poll() {
            if completion.key == key {
                return completion.result.map(drop);
            }
        }
        if !transport.has_pending() {
            return Err(qnc_ui_text::text("db.content_writer_returned_result").into());
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests;
