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
    collections::VecDeque,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
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
/// Runtime entry of the project database: the clip a preview wants next. Whoever
/// shows a clip whose record is not final writes it; completion takes that clip first
/// (v5: the selected clip first).
pub const WANTED: &str = qnc_playback_activity::PLAYBACK_CLIP;
/// How many times one run takes a clip whose probe never read its medium (did not
/// start or ran out of time, v5: such a job goes back to the queue). A clip still
/// interrupted after that waits for the next run.
pub const TRIES: usize = 3;

/// What one run did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Completion {
    /// Completed clips, in the order they were published.
    pub completed: Vec<String>,
    /// Clips that could not be completed and why. A probe that failed on the medium
    /// is never run again; one that never read it is taken again by a later run.
    pub failed: Vec<(String, String)>,
}

/// A probe backend for the source `source_uri` and the media of it that lack
/// something. The composing process binds it to this computer (where the card is
/// mounted, which ffprobe); this module names no application and no host.
pub type MakeBackend<'a> = &'a (dyn Fn(&str, &[SourceReference]) -> Result<Box<dyn ProbeBackend + Send>, String>
         + Sync);

/// How a run goes: how many clips at once (the host parallelism), which clip is
/// wanted first, whether a player works (then it waits) and when to stop.
pub struct Run<'a> {
    pub workers: usize,
    pub wanted: &'a (dyn Fn() -> Option<String> + Sync),
    pub player_works: &'a (dyn Fn() -> bool + Sync),
    pub cancel: &'a AtomicBool,
}

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
    workers: usize,
    player_works: &(dyn Fn() -> bool + Sync),
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
    let parts = Parts {
        records: &records,
        sources: &sources,
        make_backend,
    };
    let wanted = || wanted_clip(&content);
    let run = Run {
        workers,
        wanted: &wanted,
        player_works,
        cancel,
    };
    Ok(complete_clips(&parts, waiting, &content, &run))
}

/// The clip a preview wants next, from the project database.
pub fn wanted_clip(content: &ContentTarget) -> Option<String> {
    let entry = content.open(Access::ReadOnly).ok()?.get_runtime(WANTED).ok()??;
    Some(entry.value).filter(|value| !value.is_empty())
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

/// Completes the given camera clips with `run.workers` at once: the probes run side by
/// side, the records go through the one serial writer of the project database. The
/// wanted clip is taken first; all give way to a working player.
pub fn complete_clips(
    parts: &Parts<'_>,
    waiting: Vec<CatalogClip>,
    content: &ContentTarget,
    run: &Run<'_>,
) -> Completion {
    let queue = Mutex::new(waiting.into_iter().map(|clip| (clip, 1)).collect::<VecDeque<_>>());
    let done = Mutex::new(Completion::default());
    std::thread::scope(|scope| {
        for _ in 0..run.workers.max(1) {
            scope.spawn(|| {
                let mut publisher = match ContentWriteTransport::start(content.clone()) {
                    Ok(publisher) => publisher,
                    Err(error) => {
                        done.lock().expect("completion").failed.push((String::new(), error));
                        return;
                    }
                };
                while let Some((clip, tries)) = next(&queue, run) {
                    let id = clip.id().to_string();
                    let result = complete_clip(parts, clip.clone(), run.cancel)
                        .and_then(|clip| Ok(publish(&mut publisher, clip)?));
                    match result {
                        Ok(()) => done.lock().expect("completion").completed.push(id),
                        Err(error)
                            if error.is_interrupted()
                                && tries < TRIES
                                && !run.cancel.load(Ordering::Relaxed) =>
                        {
                            queue.lock().expect("completion queue").push_back((clip, tries + 1));
                        }
                        Err(error) => done
                            .lock()
                            .expect("completion")
                            .failed
                            .push((id, error.to_string())),
                    }
                }
            });
        }
    });
    done.into_inner().expect("completion")
}

/// The next clip: none when cancelled; waits while a player works; the wanted clip
/// first, else the first in line.
fn next(
    queue: &Mutex<VecDeque<(CatalogClip, usize)>>,
    run: &Run<'_>,
) -> Option<(CatalogClip, usize)> {
    loop {
        if run.cancel.load(Ordering::Relaxed) {
            return None;
        }
        if !(run.player_works)() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let wanted = (run.wanted)();
    let mut queue = queue.lock().expect("completion queue");
    let at = wanted
        .and_then(|id| queue.iter().position(|(clip, _)| clip.id() == id))
        .unwrap_or(0);
    queue.remove(at)
}

/// One clip: its own media record and source record, a probe backend bound to its
/// source only when something is missing, and the final record in its catalog entry.
fn complete_clip(
    parts: &Parts<'_>,
    mut clip: CatalogClip,
    cancel: &AtomicBool,
) -> Result<CatalogClip, qnc_record_probe::Error> {
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
