//! Shot analysis of the clips of a project (user 2026-10-27: Video AI in AI Media Assist; a
//! background job of its own after the filmstrip, lower than the player, on by the project's
//! setting `ai.enabled` "AI shot analysis and SubClips"). The background worker runs this block
//! after the filmstrips and waves; it knows no form.
//!
//! For each imported clip not yet analysed: one intra picture of its original about every
//! second, read through the original's own MXF index (the card is only read, nothing else of
//! the file), decoded small and grey through the decoder catalog, the shots found by
//! `qnc-shot-detect`, kept in the project database by `qnc-shot-db` under the caller's lease.
//! The clip a preview has open goes first; while the player plays it waits. An original that
//! is not on this computer is left for a later run; one without a usable index (not MXF, a
//! format not read yet) is recorded with that reason.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use qnc_content_store::{Access, ContentTarget};
use qnc_db_broker::{ProjectDbTarget, WriteLease};
use qnc_shot_db::{ClipShots, PartRecord, ShotRecord};
use qnc_work_settings::{SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.shot-analysis";

/// Where a media URI is on this computer; `None` when it is not here.
pub type LocalMedia<'a> = dyn Fn(&str) -> Option<PathBuf> + 'a;

/// The pictures a shot analysis looks at: their size and at most how many a second.
const PICTURE: [u32; 2] = [160, 90];
const MOST_A_SECOND: f64 = 4.0;
/// Pictures decoded in one decoder call.
const BATCH: usize = 16;

/// The project whose clips are analysed: its settings as read from its database.
#[derive(Debug, Clone)]
pub struct ShotProject {
    reader: SettingsReader,
    settings: WorkSettings,
}

impl ShotProject {
    pub fn new(reader: SettingsReader, settings: WorkSettings) -> Self {
        Self { reader, settings }
    }

    /// The active project of the QNC root, read from its database now.
    pub fn active(root: &Path) -> Result<Self, String> {
        let active = qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
        let snapshot = active.read().map_err(|e| e.to_string())?;
        Ok(Self::new(active.settings_reader().clone(), snapshot.settings))
    }
}

/// What the caller says while the analysis runs.
pub struct Pace<'a> {
    /// The player plays now: the analysis waits.
    pub player_plays: &'a dyn Fn() -> bool,
    /// The clip a preview has open, analysed first.
    pub wanted: &'a dyn Fn() -> Option<String>,
    /// The run stops (the project closed, its lease lost).
    pub cancel: &'a AtomicBool,
}

/// Analyses every imported clip of the project not analysed yet; returns how many it kept.
/// Nothing is done when the project has shot analysis off.
pub fn analyse_missing(project: &ShotProject, local_media: &LocalMedia<'_>, lease: Option<WriteLease>, pace: &Pace<'_>) -> Result<usize, String> {
    let (reader, settings) = (&project.reader, &project.settings);
    if !settings.ai_enabled() {
        return Ok(0);
    }
    let shots_db = ProjectDbTarget::for_project(reader, settings)?;
    let analysed = qnc_shot_db::read_analysed(&shots_db)?;
    let catalog = qnc_content_read::ContentReader::for_project(reader, settings)?;
    let waiting = catalog.records_waiting()?;
    let mut remaining: Vec<String> = catalog
        .summaries()?
        .into_iter()
        .filter(|clip| clip.imported && !analysed.contains(&clip.clip_id) && !waiting.contains(&clip.clip_id))
        .map(|clip| clip.clip_id)
        .collect();
    if remaining.is_empty() {
        return Ok(0);
    }
    let content = ContentTarget::for_project(reader, settings)?;
    let mut client = content.open(Access::ReadOnly)?;
    let mut kept = 0;
    while !remaining.is_empty() {
        if !wait_for_player(pace) {
            break;
        }
        let at = (pace.wanted)().and_then(|clip| remaining.iter().position(|id| *id == clip)).unwrap_or(0);
        let clip_id = remaining.remove(at);
        let Some(clip) = client.read(&clip_id)? else { continue };
        let started = Instant::now();
        let original = clip.clip.snapshot.metadata.original.media_uri.clone();
        let Some(path) = local_media(&original) else {
            log(format!("shot-analysis clip={clip_id} skipped: the original is not on this computer"));
            continue;
        };
        let found = analyse(&clip_id, &clip.clip.snapshot, &path, pace);
        if pace.cancel.load(Ordering::Relaxed) {
            break;
        }
        let record = match found {
            Ok(found) => ClipShots { clip_id: clip_id.clone(), media_uri: original, pictures: found.pictures, shots: found.shots, parts: found.parts, error: String::new(), took_ms: 0 },
            Err(error) => ClipShots { clip_id: clip_id.clone(), media_uri: original, pictures: 0, shots: Vec::new(), parts: Vec::new(), error, took_ms: 0 },
        };
        let record = ClipShots { took_ms: started.elapsed().as_millis() as u64, ..record };
        log(format!(
            "shot-analysis clip={clip_id} pictures={} shots={} parts={} ms={} error={}",
            record.pictures,
            record.shots.len(),
            record.parts.len(),
            record.took_ms,
            record.error
        ));
        qnc_shot_db::save(&shots_db, record, lease.clone())?;
        kept += 1;
    }
    Ok(kept)
}

/// Waits while the player plays; false when the run stops.
fn wait_for_player(pace: &Pace<'_>) -> bool {
    while (pace.player_plays)() {
        if pace.cancel.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    !pace.cancel.load(Ordering::Relaxed)
}

/// The shots of one clip and how many pictures they came from; an error when its original
/// cannot be read this way.
fn analyse(clip_id: &str, snapshot: &qnc_media_records::Snapshot, path: &Path, pace: &Pace<'_>) -> Result<Found, String> {
    let codec = qnc_filmstrip::original_picture_codec(snapshot).ok_or_else(|| qnc_ui_text::text("shots.codec_not_read").to_string())?;
    analyse_file(clip_id, codec, path, pace)
}

/// What the analysis of one clip found.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// How many pictures it looked at.
    pub pictures: u32,
    pub shots: Vec<ShotRecord>,
    pub parts: Vec<PartRecord>,
}

/// The shots and usable parts of one MXF original at `path` whose pictures are `codec`
/// (`h264`, `mpeg2video`); the file is only read.
pub fn analyse_file(clip_id: &str, codec: &str, path: &Path, pace: &Pace<'_>) -> Result<Found, String> {
    let index = qnc_mxf_index::MxfIndex::read(path)?.ok_or_else(|| qnc_ui_text::text("shots.no_index").to_string())?;
    let frames = index.entries.len() as u64;
    let (num, den) = index.rate;
    if frames == 0 || num <= 0 || den <= 0 {
        return Err(qnc_ui_text::text("shots.no_index").into());
    }
    let rate = num as f64 / den as f64;
    // Every intra picture (about two a second for a camera's long GOP), at most
    // `MOST_A_SECOND` a second (an all-intra original); its frame is where it is shown.
    let least_apart = (rate / MOST_A_SECOND).round().max(1.0) as usize;
    let mut entries: Vec<usize> = Vec::new();
    for entry in index.intra_frames() {
        if entries.last().is_none_or(|&last| entry >= last + least_apart) {
            entries.push(entry);
        }
    }
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor().map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("shots.no_decoder").to_string())?;
    let temp = scratch_dir(clip_id);
    let result: Result<Vec<qnc_shot_detect::Picture>, String> = (|| {
        let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut pictures = Vec::with_capacity(entries.len());
        for batch in entries.chunks(BATCH) {
            if !wait_for_player(pace) {
                return Err(qnc_ui_text::text("shots.stopped").into());
            }
            let _ = fs::remove_dir_all(&temp);
            fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
            let mut inputs = Vec::with_capacity(batch.len());
            for (n, &entry) in batch.iter().enumerate() {
                let picture = qnc_mxf_index::read_package(&mut file, &index, entry, true, false)?
                    .picture
                    .ok_or_else(|| qnc_ui_text::text("shots.no_index").to_string())?;
                let input = temp.join(format!("{n:03}.{codec}"));
                fs::write(&input, picture).map_err(|e| e.to_string())?;
                inputs.push(input);
            }
            extractor.decode_pictures_with_cancel(codec, &inputs, PICTURE, &temp, pace.cancel)?;
            for (n, &entry) in batch.iter().enumerate() {
                let Ok(image) = image::open(temp.join(format!("{n:03}.jpg"))) else { continue };
                let grey = image.into_luma8();
                let shown = (entry as i64 + index.entries[entry].temporal_offset as i64).clamp(0, frames as i64 - 1) as u64;
                pictures.push(qnc_shot_detect::Picture { frame: shown, width: grey.width() as usize, height: grey.height() as usize, luma: grey.into_raw() });
            }
        }
        Ok(pictures)
    })();
    let _ = fs::remove_dir_all(&temp);
    let pictures = result?;
    if pictures.is_empty() {
        return Err(qnc_ui_text::text("shots.no_pictures").into());
    }
    let count = pictures.len() as u32;
    let found = qnc_shot_detect::shots(pictures.clone(), frames);
    let parts = qnc_shot_detect::parts(pictures, &found, rate)
        .into_iter()
        .map(|part| PartRecord { from: part.from, to: part.to, camera: part.camera.name().into(), key_frame: part.key_frame })
        .collect();
    let shots = found
        .into_iter()
        .map(|shot| ShotRecord {
            from: shot.from,
            to: shot.to,
            looks: shot.looks.iter().map(|look| look.name().to_string()).collect(),
            key_frame: shot.key_frame,
            brightness: shot.brightness,
            sharpness: shot.sharpness,
            motion: shot.motion,
        })
        .collect();
    Ok(Found { pictures: count, shots, parts })
}

fn scratch_dir(clip_id: &str) -> PathBuf {
    let name: String = clip_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    std::env::temp_dir().join("qnc-shot-analysis").join(format!("{name}-{}", std::process::id()))
}

fn log(line: String) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}
