//! Completes the card records of the active project in the background (QNC v5
//! `ingest_probe/scheduler.rs`: after a scan, every clip of the source whose record
//! lacks what playback reads gets one media probe job).
//!
//! It reads the active project from the database, takes the clips whose media record
//! is still a camera record, completes each once through `qnc-record-probe` (without
//! a probe, and without touching the card, when nothing is missing) and publishes the
//! final record to the project content, so a preview waiting for it plays. It gives
//! way while any player works. Every read and write goes through the public project
//! database modules; it knows no form and no application.

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use qnc_content_store::{Access, CatalogClip, ContentTarget, ContentWriteTransport};
use qnc_db_broker::{ProjectDbTarget, ProjectDbWriter};
use qnc_media_probe::{ProbeBackend, Report, Request as ProbeRequest};
use qnc_media_record_db::contract::Phase;
use qnc_media_record_db::project::{MediaRecordsModule, ProjectMediaRecords};
use qnc_source_index_db::project::{ProjectSourceIndex, SourceIndexModule};
use qnc_source_reader::SourceReference;

pub const MODULE_ID: &str = "qnc.module.record-completion";

/// What one run did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Completion {
    pub completed: usize,
    /// Clips that could not be completed and why (never retried by a new probe).
    pub failed: Vec<(String, String)>,
}

/// A probe backend for the source `source_uri` and the media of it that lack
/// something. The composing process binds it to this computer (where the card is
/// mounted, which ffprobe); this module names no application and no host.
pub type MakeBackend<'a> =
    &'a dyn Fn(&str, &[SourceReference]) -> Result<Box<dyn ProbeBackend + Send>, String>;

/// What completing a clip reads and writes.
pub struct Parts<'a> {
    pub records: &'a ProjectMediaRecords,
    pub sources: &'a ProjectSourceIndex,
    pub make_backend: MakeBackend<'a>,
}

/// Completes every camera record of the active project under `root`. `player_works`
/// says whether a player prepares or plays; this waits while it does.
pub fn complete_active_project(
    root: &Path,
    make_backend: MakeBackend<'_>,
    player_works: &dyn Fn() -> bool,
    cancel: &AtomicBool,
) -> Result<Completion, String> {
    let active = qnc_active_project_read::ActiveProjectReader::from_root(root)
        .map_err(|e| e.to_string())?;
    let snapshot = active.read().map_err(|e| e.to_string())?;
    let reader = active.settings_reader();
    let content = ContentTarget::for_project(reader, &snapshot.settings)?;
    let waiting = camera_clips(&content)?;
    if waiting.is_empty() {
        return Ok(Completion::default());
    }
    let writer = ProjectDbWriter::start(
        ProjectDbTarget::for_project(reader, &snapshot.settings)?,
        vec![MediaRecordsModule::factory(), SourceIndexModule::factory()],
    )?;
    let records = ProjectMediaRecords::new(writer.clone());
    let sources = ProjectSourceIndex::new(writer);
    let mut publisher = ContentWriteTransport::start(content)?;
    let parts = Parts {
        records: &records,
        sources: &sources,
        make_backend,
    };
    Ok(complete_clips(&parts, waiting, &mut publisher, player_works, cancel))
}

/// The clips of the project content whose record is still a camera record.
pub fn camera_clips(content: &ContentTarget) -> Result<Vec<CatalogClip>, String> {
    let mut client = content.open(Access::ReadOnly)?;
    let mut clips = Vec::new();
    let mut after = None;
    loop {
        let page = client.list(after.clone())?;
        let Some(last) = page.last() else {
            return Ok(clips);
        };
        after = Some(last.clip.id().to_string());
        clips.extend(
            page.into_iter()
                .filter(|stored| stored.clip.snapshot.phase == Phase::Camera)
                .map(|stored| stored.clip),
        );
    }
}

/// Completes the given camera clips in order, giving way to a working player.
pub fn complete_clips(
    parts: &Parts<'_>,
    waiting: Vec<CatalogClip>,
    publisher: &mut ContentWriteTransport,
    player_works: &dyn Fn() -> bool,
    cancel: &AtomicBool,
) -> Completion {
    let mut done = Completion::default();
    for clip in waiting {
        while player_works() {
            if cancel.load(Ordering::Relaxed) {
                return done;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let id = clip.id().to_string();
        match complete_clip(parts, clip, cancel).and_then(|clip| publish(publisher, clip)) {
            Ok(()) => done.completed += 1,
            Err(error) => done.failed.push((id, error)),
        }
    }
    done
}

/// One clip: its own media record and source record, a probe backend bound to its
/// source only when something is missing, and the final record in its catalog entry.
fn complete_clip(
    parts: &Parts<'_>,
    mut clip: CatalogClip,
    cancel: &AtomicBool,
) -> Result<CatalogClip, String> {
    let camera = parts
        .records
        .read(clip.id(), None)?
        .ok_or("Zapis medija klipa nije u bazi projekta.")?;
    let source_record = parts
        .sources
        .read(&camera.binding.source_record_id)?
        .ok_or("Izvorni zapis klipa nije u bazi projekta.")?;
    let needed =
        qnc_media_metadata_compose::required_probes(&camera).map_err(|e| format!("{e:?}"))?;
    let media = needed
        .iter()
        .map(|uri| SourceReference::from_uri(uri).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    // Nothing missing: final without a probe, and without touching the card.
    let backend: Box<dyn ProbeBackend + Send> = if media.is_empty() {
        Box::new(NoProbe)
    } else {
        (parts.make_backend)(&clip.source_uri, &media)?
    };
    clip.snapshot = qnc_record_probe::complete(
        parts.records,
        &source_record,
        camera,
        backend.as_ref(),
        cancel,
    )?;
    Ok(clip)
}

fn publish(publisher: &mut ContentWriteTransport, clip: CatalogClip) -> Result<(), String> {
    let key = format!("complete:{}", clip.id());
    publisher.publish_batch(key.clone(), vec![clip])?;
    loop {
        for completion in publisher.poll() {
            if completion.key == key {
                return completion.result.map(|_| ());
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Used when a record lacks nothing: asked for a probe, it refuses.
struct NoProbe;

impl ProbeBackend for NoProbe {
    fn execute(&self, _: &ProbeRequest) -> qnc_media_probe::Result<Report> {
        Err(qnc_media_probe::Error::InvalidRequest)
    }
}

#[cfg(test)]
mod tests;
