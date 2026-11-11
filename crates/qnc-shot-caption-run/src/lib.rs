//! A sentence of what each shot of a project shows (user 2026-11-10: Florence-2 as the second AI
//! engine beside SigLIP; "it names what it sees without a vocabulary"). The background worker
//! runs this block after the look at the shots; it knows no form.
//!
//! Every shot the shot-content agent looked at has a poster (the card's poster for a clip seen
//! only through it, otherwise the shot's own poster in the project's thumbnails). The posters of
//! the shots without a caption of this host's model are copied into an empty temporary folder,
//! given to the shot-caption agent (`qnc-shot-caption`, its own program) a few at a time, and the
//! captions kept in the captions database of this computer (`qnc-shot-captions`), not in the
//! project: the AI agents are an add-on. Nothing is decoded; the card is only read. While the
//! player plays it waits; the clip a preview has open goes first.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Instant,
};

use qnc_db_broker::ProjectDbTarget;
use qnc_shot_analysis::{LocalMedia, Pace};
use qnc_work_settings::{SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.shot-caption-run";

/// A shot to caption: its key, clip, frames and poster on this computer.
struct Wanted {
    key: String,
    clip_id: String,
    from: u64,
    to: u64,
    poster: PathBuf,
}

/// Captions every looked-at shot of the project that has none of this host's model; returns how
/// many it kept. Nothing is done when the project has AI off or this host has no shot captioner.
pub fn caption_missing(root: &Path, reader: &SettingsReader, settings: &WorkSettings, local_media: &LocalMedia<'_>, pace: &Pace<'_>) -> Result<usize, String> {
    if !settings.ai_feature(qnc_ai_features::SHOT_CAPTION).unwrap_or(false) {
        return Ok(0);
    }
    let captioner = match qnc_ai_catalog::installed_shot_captioner() {
        Ok(captioner) => captioner,
        Err(error) => {
            log(format!("shot-caption skipped: {error}"));
            return Ok(0);
        }
    };
    // The shots are those the shot-content agent looked at (their posters are its).
    let Ok(content_engine) = qnc_ai_catalog::installed_shot_content() else { return Ok(0) };
    let model = captioner.model_name();
    let db = ProjectDbTarget::for_project(reader, settings)?;
    let project = db.uri().to_string();
    let done = qnc_shot_captions::captioned(root, &model, &project)?;
    let project_dir = reader.local_workspace_dir(settings).map_err(|e| e.to_string())?;
    let output_root = format!("{}/", settings.output_root_uri.trim_end_matches('/'));
    let local = |uri: &str| match (uri.strip_prefix(&output_root), &project_dir) {
        (Some(relative), Some(dir)) if !relative.contains("..") => Some(dir.join(relative)).filter(|file| file.is_file()),
        _ => local_media(uri),
    };
    let mut wanted: Vec<Wanted> = qnc_shot_content_db::read_all(&db, &content_engine.model_name())?
        .into_iter()
        .flat_map(|clip| {
            let clip_id = clip.clip_id;
            clip.parts.into_iter().map(move |part| (clip_id.clone(), part))
        })
        .filter_map(|(clip_id, part)| {
            let key = qnc_shot_captions::key(&project, &clip_id, part.from, part.to);
            if part.poster.is_empty() || done.contains(&key) {
                return None;
            }
            let poster = local(&part.poster)?;
            Some(Wanted { key, clip_id, from: part.from, to: part.to, poster })
        })
        .collect();
    if wanted.is_empty() {
        return Ok(0);
    }
    let client = qnc_shot_caption::CaptionClient::new(&qnc_plugin_process::beside_application(&captioner.program)?);
    // The agent reads only copies in a folder of its own (the card stays only read).
    let temp = std::env::temp_dir().join("qnc-shot-caption").join(std::process::id().to_string());
    let mut kept = 0;
    while !wanted.is_empty() {
        if !wait_for_player(pace) {
            break;
        }
        // The clip a preview has open first, then the clips in their order.
        let open = (pace.wanted)();
        let first = open.as_deref().and_then(|clip| wanted.iter().position(|shot| shot.clip_id == clip)).unwrap_or(0);
        let clip_id = wanted[first].clip_id.clone();
        let batch: Vec<Wanted> = {
            let mut taken = Vec::new();
            let mut index = 0;
            while index < wanted.len() && taken.len() < qnc_shot_caption::MOST_AT_ONCE {
                if wanted[index].clip_id == clip_id {
                    taken.push(wanted.remove(index));
                } else {
                    index += 1;
                }
            }
            taken
        };
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
        let mut pictures = Vec::new();
        for (n, shot) in batch.iter().enumerate() {
            let copy = temp.join(format!("{n:02}.jpg"));
            fs::copy(&shot.poster, &copy).map_err(|e| e.to_string())?;
            pictures.push(copy);
        }
        let started = Instant::now();
        let captions = client.captions(&qnc_shot_caption::CaptionsRequest { model: captioner.model.clone(), pictures });
        let _ = fs::remove_dir_all(&temp);
        let captions = captions?;
        let ms = started.elapsed().as_millis();
        for (shot, caption) in batch.iter().zip(captions) {
            log(format!("shot-caption clip={} shot={}-{} caption={caption}", shot.clip_id, shot.from, shot.to));
            if let Err(error) = qnc_shot_captions::save(root, &model, &shot.key, &caption) {
                log(format!("shot-caption clip={} shot={}-{} not kept error={error}", shot.clip_id, shot.from, shot.to));
                continue;
            }
            kept += 1;
        }
        log(format!("shot-caption clip={clip_id} shots={} ms={ms}", batch.len()));
        if pace.cancel.load(Ordering::Relaxed) {
            break;
        }
    }
    log(format!("shot-caption model={model} kept={kept}"));
    Ok(kept)
}

/// Waits while the player plays; false when the run stops.
fn wait_for_player(pace: &Pace<'_>) -> bool {
    while (pace.player_plays)() {
        if pace.cancel.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    !pace.cancel.load(Ordering::Relaxed)
}

fn log(line: String) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}
