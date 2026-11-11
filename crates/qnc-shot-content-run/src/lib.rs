//! What the usable parts of the clips of a project show (user 2026-11-10: sorting the B-roll by
//! content; "for the content of a clip one or two, at most three pictures are enough, after the
//! project's settings or the strength of the computer"). The background worker runs this block
//! after the shot analysis; it knows no form.
//!
//! For each clip whose shot analysis found usable parts and whose content is not kept for the
//! model of this host: per part 1-3 pictures (as many as the AI catalog of this host says), each
//! the intra picture of the original nearest to the part's sharpest picture, then a fifth in from
//! its start and from its end; read through the original's own MXF index (the card is only
//! read), decoded at the model's size through the decoder catalog, given to the shot-content
//! agent (`qnc-shot-content`, its own program), and the mean of a part's vectors kept by
//! `qnc-shot-content-db` under the caller's lease. While the player plays it waits.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Instant,
};

use qnc_db_broker::{ProjectDbTarget, WriteLease};
use qnc_shot_analysis::{LocalMedia, Pace};
use qnc_shot_content_db::{ClipContent, PartContent};
use qnc_shot_db::PartRecord;
use qnc_work_settings::{SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.shot-content-run";

/// The pictures decoded for a shot: its poster and what the model is given (the model makes it
/// square itself).
const PICTURE: [u32; 2] = [384, 216];

/// Looks at every clip of the project whose usable parts are not looked at yet by the model of
/// this host; returns how many it kept. Nothing is done when the project has AI off or this host
/// has no shot content engine.
pub fn look_missing(reader: &SettingsReader, settings: &WorkSettings, local_media: &LocalMedia<'_>, lease: Option<WriteLease>, pace: &Pace<'_>) -> Result<usize, String> {
    if !settings.ai_feature(qnc_ai_features::SHOT_CONTENT).unwrap_or(false) {
        return Ok(0);
    }
    let engine = match qnc_ai_catalog::installed_shot_content() {
        Ok(engine) => engine,
        Err(error) => {
            log(format!("shot-content skipped: {error}"));
            return Ok(0);
        }
    };
    let model = engine.model_name();
    // The vocabulary's vectors, once a model on this computer (user 2026-11-10: not in a project).
    if let Err(error) = keep_vocabulary(&engine, &model) {
        log(format!("shot-content vocabulary failed error={error}"));
    }
    let db = ProjectDbTarget::for_project(reader, settings)?;
    // A clip looked at only through its poster is looked at again once its shots are analysed.
    let looked: Vec<String> = qnc_shot_content_db::read_all(&db, &model)?.into_iter().filter(|clip| !poster_only(clip)).map(|clip| clip.clip_id).collect();
    let mut remaining: Vec<String> = qnc_shot_db::read_analysed(&db)?.into_iter().filter(|clip| !looked.contains(clip)).collect();
    // The label vectors the forms sort the B-roll by (no model in a form): made again only when
    // the list of labels says something else than the kept ones.
    let labels = qnc_shot_labels::installed_path().and_then(|path| qnc_shot_labels::Labels::read(&path)).ok();
    let stored = qnc_shot_content_db::read_labels(&db, &model)?;
    let stale = labels.as_ref().filter(|list| !stored.as_ref().is_some_and(|set| same_labels(set, list)));
    if remaining.is_empty() && stale.is_none() {
        return Ok(0);
    }
    let client = qnc_shot_content::ContentClient::for_program(&engine.program)?;
    let mut catalog = qnc_content_store::ContentTarget::for_project(reader, settings)?.open(qnc_content_store::Access::ReadOnly)?;
    let label_texts = match stale {
        Some(list) => {
            let texts = client.texts(&qnc_shot_content::TextsRequest::new(&engine.model, list.prompts()))?;
            let set = qnc_shot_content_db::LabelSet {
                model: model.clone(),
                scale: texts.scale,
                bias: texts.bias,
                labels: list
                    .labels
                    .iter()
                    .zip(&texts.vectors)
                    .map(|(label, vector)| qnc_shot_content_db::LabelVector { code: label.code.clone(), prompt: label.prompt.clone(), vector: vector.clone() })
                    .collect(),
            };
            qnc_shot_content_db::save_labels(&db, set, lease.clone())?;
            log(format!("shot-content labels model={model} kept={}", list.labels.len()));
            Some(texts)
        }
        None => stored.map(|set| qnc_shot_content::Texts { vectors: set.labels.into_iter().map(|label| label.vector).collect(), scale: set.scale, bias: set.bias }),
    };
    // Where the shots' posters go: the project's thumbnails, a folder a clip.
    let project_dir = reader.local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or(qnc_ui_text::text("import.import_needs_local_access_project_folder"))?;
    let thumbnails = (settings.product_local_dir(&project_dir, qnc_work_settings::ProductArea::Thumbnails), settings.product_uri(qnc_work_settings::ProductArea::Thumbnails));
    // First quickly, every clip: one picture a shot, its poster (user 2026-11-10); then finely,
    // the usable parts with as many pictures as this host says.
    let mut fine: Vec<String> = if engine.pictures > 1 {
        qnc_shot_content_db::read_all(&db, &model)?.into_iter().filter(|clip| clip.parts.iter().any(|part| !part.fine && is_part(&part.kind))).map(|clip| clip.clip_id).collect()
    } else {
        Vec::new()
    };
    let mut kept = 0;
    while !remaining.is_empty() || !fine.is_empty() {
        if !wait_for_player(pace) {
            break;
        }
        let quick = !remaining.is_empty();
        let queue = if quick { &mut remaining } else { &mut fine };
        let at = (pace.wanted)().and_then(|clip| queue.iter().position(|id| *id == clip)).unwrap_or(0);
        let clip_id = queue.remove(at);
        let Some(shots) = qnc_shot_db::read_clip(&db, &clip_id)? else { continue };
        let units = units(&shots);
        if units.is_empty() {
            // A clip the poster sorted keeps that look when its analysis has no shot to look at.
            if qnc_shot_content_db::read_clip(&db, &clip_id)?.is_some_and(|clip| poster_only(&clip)) {
                continue;
            }
            // Nothing to look at (or the analysis failed): kept as looked at, with why.
            let why = if shots.error.is_empty() { qnc_ui_text::text("shots.content_no_parts").to_string() } else { shots.error.clone() };
            qnc_shot_content_db::save(&db, ClipContent { clip_id, model: model.clone(), parts: Vec::new(), error: why, took_ms: 0 }, lease.clone())?;
            continue;
        }
        let Some(path) = local_media(&shots.media_uri) else {
            log(format!("shot-content clip={clip_id} skipped: the original is not on this computer"));
            continue;
        };
        // The picture codec as the clip's saved record has it (never a probe).
        let codec = catalog.read(&clip_id)?.and_then(|clip| qnc_filmstrip::original_picture_codec(&clip.clip.snapshot));
        let Some(codec) = codec else {
            qnc_shot_content_db::save(&db, ClipContent { clip_id, model: model.clone(), parts: Vec::new(), error: qnc_ui_text::text("shots.codec_not_read").into(), took_ms: 0 }, lease.clone())?;
            continue;
        };
        let started = Instant::now();
        let looked = if quick {
            let folder = thumbnails.0.join(&clip_id).join("shots");
            let base = format!("{}/{clip_id}/shots", thumbnails.1.trim_end_matches('/'));
            look_file(&clip_id, codec, &path, &units, 1, &engine.model, &client, pace, Some((&folder, &base)))
        } else {
            // Finely: the parts with more pictures; a shot without one keeps its quick look.
            let before = qnc_shot_content_db::read_clip(&db, &clip_id)?.map(|clip| clip.parts).unwrap_or_default();
            let parts: Vec<PartRecord> = units.iter().filter(|unit| is_part(&unit.camera)).cloned().collect();
            look_file(&clip_id, codec, &path, &parts, engine.pictures, &engine.model, &client, pace, None).map(|finer| {
                before
                    .into_iter()
                    .map(|old| match finer.iter().find(|new| new.from == old.from && new.to == old.to) {
                        Some(new) => PartContent { frames: new.frames.clone(), vector: new.vector.clone(), fine: true, ..old },
                        None => PartContent { fine: true, ..old },
                    })
                    .collect()
            })
        };
        if pace.cancel.load(Ordering::Relaxed) {
            break;
        }
        // A statement is not cut by its pictures (user 2026-11-10: "a shot with A1 must not be cut
        // into many small pieces; if it is cut, by its sound, not by its picture"): it stays a
        // whole shot.
        let looked = looked.map(|parts| match (&labels, &label_texts) {
            (Some(list), Some(texts)) if quick && is_statement(&parts, list, texts, &qnc_transcript_db::read_clip(&db, &clip_id).unwrap_or_default()) => whole_shots(&shots, parts),
            _ => parts,
        });
        let content = match looked {
            Ok(parts) => ClipContent { clip_id: clip_id.clone(), model: model.clone(), parts, error: String::new(), took_ms: 0 },
            Err(error) => ClipContent { clip_id: clip_id.clone(), model: model.clone(), parts: Vec::new(), error, took_ms: 0 },
        };
        let content = ClipContent { took_ms: started.elapsed().as_millis() as u64, ..content };
        log(format!("shot-content clip={clip_id} {} shots={} ms={} error={}", if quick { "quick" } else { "fine" }, content.parts.len(), content.took_ms, content.error));
        if let (Some(list), Some(texts)) = (&labels, &label_texts) {
            for part in &content.parts {
                let best: Vec<String> = list.best(&texts.logits(&part.vector)).into_iter().take(3).map(|(code, score)| format!("{code}:{score:.2}")).collect();
                log(format!("shot-content clip={clip_id} shot={}-{} kind={} frames={:?} labels={}", part.from, part.to, part.kind, part.frames, best.join(",")));
            }
        }
        // A quick look of a clip that has parts goes on to be looked at finely.
        if quick && engine.pictures > 1 && content.parts.iter().any(|part| is_part(&part.kind)) {
            fine.push(clip_id.clone());
        }
        qnc_shot_content_db::save(&db, content, lease.clone())?;
        kept += 1;
    }
    Ok(kept)
}

/// The first, quickest look (user 2026-11-10: "the first AI sorting works from the card's
/// poster, the second from the posters made by the analysis"): every imported clip with a poster
/// and nothing looked at yet gets one shot spanning the whole clip, its vector from that poster
/// (the card's, or its copy in the project; nothing is decoded). The shots of the analysis replace
/// it later. Returns how many it kept.
pub fn look_posters(reader: &SettingsReader, settings: &WorkSettings, local_media: &LocalMedia<'_>, lease: Option<WriteLease>, pace: &Pace<'_>) -> Result<usize, String> {
    if !settings.ai_feature(qnc_ai_features::SHOT_CONTENT).unwrap_or(false) {
        return Ok(0);
    }
    let Ok(engine) = qnc_ai_catalog::installed_shot_content() else { return Ok(0) };
    let model = engine.model_name();
    let db = ProjectDbTarget::for_project(reader, settings)?;
    let looked: Vec<String> = qnc_shot_content_db::read_all(&db, &model)?.into_iter().map(|clip| clip.clip_id).collect();
    let project_dir = reader.local_workspace_dir(settings).map_err(|e| e.to_string())?;
    let root = format!("{}/", settings.output_root_uri.trim_end_matches('/'));
    let local = |uri: &str| match (uri.strip_prefix(&root), &project_dir) {
        (Some(relative), Some(dir)) if !relative.contains("..") => Some(dir.join(relative)).filter(|file| file.is_file()),
        _ => local_media(uri),
    };
    let wanted: Vec<(String, u64, String, PathBuf)> = qnc_content_read::ContentReader::for_project(reader, settings)?
        .summaries()?
        .into_iter()
        // Every clip of the catalog with a poster, imported or not (user 2026-11-10: at Ingest).
        .filter(|clip| !looked.contains(&clip.clip_id))
        .filter_map(|clip| {
            let uri = clip.thumbnail_uri?;
            let file = local(&uri)?;
            Some((clip.clip_id, clip.duration_frames.max(1), uri, file))
        })
        .collect();
    if wanted.is_empty() {
        return Ok(0);
    }
    let client = qnc_shot_content::ContentClient::for_program(&engine.program)?;
    // The posters go to the agent as bytes (contract version 2): the card stays only read and
    // the agent reads no file of the application.
    let mut kept = 0;
    for batch in wanted.chunks(qnc_shot_content::MOST_AT_ONCE) {
        if !wait_for_player(pace) || pace.cancel.load(Ordering::Relaxed) {
            break;
        }
        let files: Vec<PathBuf> = batch.iter().map(|(_, _, _, file)| file.clone()).collect();
        let vectors = client.pictures(&qnc_shot_content::PicturesRequest::of_files(&engine.model, &files)?)?;
        for ((clip_id, frames, uri, _), vector) in batch.iter().zip(vectors) {
            let shot = PartContent { from: 0, to: *frames, kind: POSTER.into(), frames: vec![0], vector, poster: uri.clone(), fine: false };
            qnc_shot_content_db::save(&db, ClipContent { clip_id: clip_id.clone(), model: model.clone(), parts: vec![shot], error: String::new(), took_ms: 0 }, lease.clone())?;
            kept += 1;
        }
    }
    log(format!("shot-content posters model={model} kept={kept}"));
    Ok(kept)
}

/// The kind of the one shot of a clip looked at only through its poster.
pub const POSTER: &str = "poster";

/// Whether a clip's content is only its poster's (the shots of the analysis still to come).
fn poster_only(clip: &ClipContent) -> bool {
    clip.parts.len() == 1 && clip.parts[0].kind == POSTER
}

/// Whether a shot kind is a usable part of the analysis (steady, pan).
fn is_part(kind: &str) -> bool {
    matches!(kind, "steady" | "pan")
}

/// The kind of a shot of a statement (kept whole, never looked at finely, never B-roll).
pub const STATEMENT: &str = "statement";

/// Whether a clip looked at quickly is a statement: most of its frames look like one (a
/// statement label's own score) and its A1 is spoken where a transcript covers it (without a
/// transcript the pictures decide, as in the AI B-roll).
pub fn is_statement(parts: &[PartContent], list: &qnc_shot_labels::Labels, texts: &qnc_shot_content::Texts, transcripts: &[qnc_transcript_db::ClipTranscript]) -> bool {
    let (Some(from), Some(to)) = (parts.iter().map(|part| part.from).min(), parts.iter().map(|part| part.to).max()) else { return false };
    let frames = |part: &PartContent| part.to - part.from;
    let looks: u64 = parts.iter().filter(|part| list.is_statement(&texts.logits(&part.vector))).map(frames).sum();
    let all: u64 = parts.iter().map(frames).sum();
    let spoken = qnc_transcript_db::spoken_share(transcripts, from, to).map(|share| share >= qnc_transcript_db::SPOKEN);
    all > 0 && looks * 2 > all && spoken != Some(false)
}

/// A statement's look as its whole shots: each shot one unit of kind `STATEMENT`, its pictures
/// those of the units inside it and its vector their mean.
pub fn whole_shots(shots: &qnc_shot_db::ClipShots, parts: Vec<PartContent>) -> Vec<PartContent> {
    shots
        .shots
        .iter()
        .filter_map(|shot| {
            let inside: Vec<&PartContent> = parts.iter().filter(|part| shot.from <= part.from && part.to <= shot.to).collect();
            let first = inside.first()?;
            let mut vector = vec![0.0f32; first.vector.len()];
            for part in &inside {
                for (sum, value) in vector.iter_mut().zip(&part.vector) {
                    *sum += value;
                }
            }
            let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
            vector.iter_mut().for_each(|v| *v /= norm);
            let mut frames: Vec<u64> = inside.iter().flat_map(|part| part.frames.iter().copied()).collect();
            frames.sort_unstable();
            frames.dedup();
            Some(PartContent { from: shot.from, to: shot.to, kind: STATEMENT.into(), frames, vector, poster: first.poster.clone(), fine: true })
        })
        .collect()
}

/// The shots a clip is looked at in (user 2026-11-10: "every shot the analysis found: steady,
/// moving, panning"): each usable part, and each shot that has no usable part as it is (its look
/// as its kind: moving, soft, still), except a black one and one shorter than half a second.
pub fn units(shots: &qnc_shot_db::ClipShots) -> Vec<PartRecord> {
    let mut units = Vec::new();
    for shot in &shots.shots {
        if shot.looks.iter().any(|look| look == "black") {
            continue;
        }
        let inside: Vec<&PartRecord> = shots.parts.iter().filter(|part| shot.from <= part.from && part.to <= shot.to).collect();
        if inside.is_empty() {
            if shot.to - shot.from >= SHORTEST_SHOT {
                let kind = ["moving", "soft", "still"].into_iter().find(|look| shot.looks.iter().any(|l| l == look)).unwrap_or("shot");
                units.push(PartRecord { from: shot.from, to: shot.to, camera: kind.into(), key_frame: shot.key_frame });
            }
        } else {
            // The usable parts, and what lies between and around them in the shot (the camera
            // moving or not steady enough): a potential B-roll too.
            let mut at = shot.from;
            for part in inside {
                if part.from >= at + SHORTEST_GAP {
                    units.push(gap(shot, at, part.from));
                }
                units.push(part.clone());
                at = at.max(part.to);
            }
            if shot.to >= at + SHORTEST_GAP {
                units.push(gap(shot, at, shot.to));
            }
        }
    }
    units
}

/// The rest of a shot outside its usable parts, as a unit of the shot's look (moving when it
/// has none), its middle as its picture.
fn gap(shot: &qnc_shot_db::ShotRecord, from: u64, to: u64) -> PartRecord {
    let kind = ["moving", "soft", "still"].into_iter().find(|look| shot.looks.iter().any(|l| l == look)).unwrap_or("moving");
    PartRecord { from, to, camera: kind.into(), key_frame: from + (to - from) / 2 }
}

/// The shortest rest of a shot looked at (frames; two seconds at 50p).
const SHORTEST_GAP: u64 = 100;

/// The shortest shot looked at (frames; half a second at 50p).
const SHORTEST_SHOT: u64 = 25;

/// Whether the kept label vectors are of the same labels and prompts, in the same order.
/// Asks the model for the vectors of the vocabulary (`qnc-shot-vocabulary`) and keeps them on this
/// computer, unless they are kept for this model and this vocabulary already.
fn keep_vocabulary(engine: &qnc_ai_catalog::ShotContentEngine, model: &str) -> Result<(), String> {
    let root = qnc_dev_diagnostics::resolve_qnc_root().ok_or("no QNC root")?;
    let list = qnc_shot_vocabulary::installed()?;
    if qnc_shot_vocabulary::read(&root, model)?.is_some_and(|set| qnc_shot_vocabulary::same(&set, &list)) {
        return Ok(());
    }
    let started = std::time::Instant::now();
    let client = qnc_shot_content::ContentClient::for_program(&engine.program)?;
    let (mut vectors, mut scoring) = (Vec::new(), (0.0, 0.0));
    for prompts in list.prompts().chunks(qnc_shot_content::MOST_AT_ONCE) {
        let texts = client.texts(&qnc_shot_content::TextsRequest::new(&engine.model, prompts.to_vec()))?;
        scoring = (texts.scale, texts.bias);
        vectors.extend(texts.vectors);
    }
    let set = qnc_shot_content_db::LabelSet {
        model: model.to_string(),
        scale: scoring.0,
        bias: scoring.1,
        labels: list.labels.iter().zip(vectors).map(|(term, vector)| qnc_shot_content_db::LabelVector { code: term.code.clone(), prompt: term.prompt.clone(), vector }).collect(),
    };
    qnc_shot_vocabulary::save(&root, &set)?;
    log(format!("shot-content vocabulary model={model} terms={} ms={}", list.labels.len(), started.elapsed().as_millis()));
    Ok(())
}

fn same_labels(set: &qnc_shot_content_db::LabelSet, list: &qnc_shot_labels::Labels) -> bool {
    set.labels.len() == list.labels.len() && set.labels.iter().zip(&list.labels).all(|(kept, label)| kept.code == label.code && kept.prompt == label.prompt)
}


/// The clip frames a part is looked at: its sharpest picture first, then a fifth in from its
/// start and from its end; `count` of them (1-3), each once.
pub fn chosen_frames(part: &PartRecord, count: u8) -> Vec<u64> {
    let length = part.to - part.from;
    let inset = length / 5;
    let candidates = [part.key_frame.clamp(part.from, part.to - 1), part.from + inset, part.to - 1 - inset.min(length - 1)];
    let mut chosen: Vec<u64> = Vec::new();
    for frame in candidates {
        if chosen.len() < usize::from(count) && !chosen.contains(&frame) {
            chosen.push(frame);
        }
    }
    chosen
}

/// The content of the parts of one MXF original at `path` (only read), its pictures `codec`
/// (`h264`, `mpeg2video`), `count` pictures a part, by the agent behind `client`.
#[allow(clippy::too_many_arguments)]
pub fn look_file(clip_id: &str, codec: &str, path: &Path, parts: &[PartRecord], count: u8, model: &Path, client: &qnc_shot_content::ContentClient, pace: &Pace<'_>, posters: Option<(&Path, &str)>) -> Result<Vec<PartContent>, String> {
    let mut keys = qnc_shot_analysis::KeyPictures::open(path)?.ok_or_else(|| qnc_ui_text::text("shots.no_index").to_string())?;
    let codec = keys.codec().unwrap_or(codec);
    // Every part's pictures: the intra picture nearest each chosen frame, inside the part.
    let mut wanted: Vec<(usize, usize, u64)> = Vec::new(); // (part, entry, shown frame)
    for (n, part) in parts.iter().enumerate() {
        let mut taken: Vec<usize> = Vec::new();
        for frame in chosen_frames(part, count) {
            let Some((entry, at)) = keys.nearest(frame) else { continue };
            if (part.from..=part.to).contains(&at) && !taken.contains(&entry) {
                taken.push(entry);
                wanted.push((n, entry, at));
            }
        }
    }
    if wanted.is_empty() {
        return Err(qnc_ui_text::text("shots.no_pictures").into());
    }
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor().map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("shots.no_decoder").to_string())?;
    let temp = scratch_dir(clip_id);
    let result: Result<Vec<Vec<f32>>, String> = (|| {
        let mut vectors = Vec::with_capacity(wanted.len());
        for batch in wanted.chunks(qnc_shot_content::MOST_AT_ONCE) {
            if !wait_for_player(pace) {
                return Err(qnc_ui_text::text("shots.stopped").into());
            }
            let _ = fs::remove_dir_all(&temp);
            fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
            let mut inputs = Vec::with_capacity(batch.len());
            for (n, (_, entry, _)) in batch.iter().enumerate() {
                let picture = keys.picture(*entry)?;
                let input = temp.join(format!("{n:03}.{codec}"));
                fs::write(&input, picture).map_err(|e| e.to_string())?;
                inputs.push(input);
            }
            extractor.decode_pictures_with_cancel(codec, &inputs, PICTURE, &temp, pace.cancel)?;
            let decoded: Vec<PathBuf> = (0..batch.len()).map(|n| temp.join(format!("{n:03}.jpg"))).collect();
            if let Some(missing) = decoded.iter().find(|file| !file.is_file()) {
                return Err(format!("{}: {}", qnc_ui_text::text("shots.no_pictures"), missing.display()));
            }
            // The first picture of a shot is its poster (the sharpest), kept in the project.
            if let Some((folder, _)) = posters {
                fs::create_dir_all(folder).map_err(|e| e.to_string())?;
                for (n, (part, _, _)) in batch.iter().enumerate() {
                    let unit = &parts[*part];
                    let poster = folder.join(poster_name(unit));
                    if !poster.is_file() {
                        fs::copy(&decoded[n], &poster).map_err(|e| e.to_string())?;
                    }
                }
            }
            vectors.extend(client.pictures(&qnc_shot_content::PicturesRequest::of_files(model, &decoded)?)?);
        }
        Ok(vectors)
    })();
    let _ = fs::remove_dir_all(&temp);
    let vectors = result?;
    Ok(parts
        .iter()
        .enumerate()
        .filter_map(|(n, part)| {
            let of_part: Vec<(&Vec<f32>, u64)> = wanted.iter().zip(&vectors).filter(|((p, _, _), _)| *p == n).map(|((_, _, at), vector)| (vector, *at)).collect();
            (!of_part.is_empty()).then(|| PartContent {
                from: part.from,
                to: part.to,
                kind: part.camera.clone(),
                frames: of_part.iter().map(|(_, at)| *at).collect(),
                vector: mean(of_part.iter().map(|(vector, _)| vector.as_slice())),
                poster: posters.map(|(_, base)| format!("{base}/{}", poster_name(part))).unwrap_or_default(),
                fine: count > 1,
            })
        })
        .collect())
}

/// The file name of a shot's poster.
fn poster_name(part: &PartRecord) -> String {
    format!("{}-{}.jpg", part.from, part.to)
}


/// The mean of unit vectors, made unit length again.
fn mean<'a>(vectors: impl Iterator<Item = &'a [f32]>) -> Vec<f32> {
    let mut sum: Vec<f32> = Vec::new();
    for vector in vectors {
        if sum.is_empty() {
            sum = vec![0.0; vector.len()];
        }
        sum.iter_mut().zip(vector).for_each(|(s, v)| *s += v);
    }
    let norm = sum.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
    sum.into_iter().map(|v| v / norm).collect()
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

fn scratch_dir(clip_id: &str) -> PathBuf {
    let name: String = clip_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    std::env::temp_dir().join("qnc-shot-content").join(format!("{name}-{}", std::process::id()))
}

fn log(line: String) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(from: u64, to: u64, key: u64) -> PartRecord {
        PartRecord { from, to, camera: "steady".into(), key_frame: key }
    }

    #[test]
    fn a_statement_is_kept_whole_not_cut_by_its_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("labels.json");
        std::fs::write(&path, r#"{"version":1,"least":0.1,"statement_least":0.5,"labels":[
            {"code":"talk","text":"t","prompt":"talk","statement":true},{"code":"sea","text":"s","prompt":"sea"}]}"#)
        .unwrap();
        let list = qnc_shot_labels::Labels::read(&path).unwrap();
        let texts = qnc_shot_content::Texts { vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]], scale: 10.0, bias: -5.0 };
        let content = |from, to, vector: Vec<f32>| PartContent { from, to, kind: "steady".into(), frames: vec![from], vector, poster: String::new(), fine: false };
        let talking = vec![content(0, 100, vec![1.0, 0.0]), content(100, 300, vec![1.0, 0.0]), content(300, 340, vec![0.0, 1.0])];
        assert!(is_statement(&talking, &list, &texts, &[]), "most frames look like a statement, no transcript");
        assert!(!is_statement(&[content(0, 300, vec![0.0, 1.0])], &list, &texts, &[]));
        let shot = qnc_shot_db::ShotRecord { from: 0, to: 340, looks: Vec::new(), key_frame: 10, brightness: 0.5, sharpness: 0.5, motion: 0.0 };
        let shots = qnc_shot_db::ClipShots { clip_id: "c".into(), media_uri: "qnc://x".into(), pictures: 3, shots: vec![shot], parts: Vec::new(), error: String::new(), took_ms: 0 };
        let whole = whole_shots(&shots, talking);
        assert_eq!(whole.iter().map(|part| (part.from, part.to, part.kind.as_str())).collect::<Vec<_>>(), [(0, 340, STATEMENT)]);
        assert_eq!(whole[0].frames, [0, 100, 300]);
    }

    #[test]
    fn a_clip_seen_only_through_its_poster_is_looked_at_again_by_its_shots() {
        let shot = |kind: &str| PartContent { from: 0, to: 50, kind: kind.into(), frames: vec![0], vector: vec![1.0], poster: String::new(), fine: false };
        let clip = |parts| ClipContent { clip_id: "c".into(), model: "m".into(), parts, error: String::new(), took_ms: 0 };
        assert!(poster_only(&clip(vec![shot(POSTER)])));
        assert!(!poster_only(&clip(vec![shot("steady")])));
        assert!(!poster_only(&clip(Vec::new())));
    }

    #[test]
    fn a_part_is_looked_at_its_sharpest_picture_first_then_in_from_its_ends() {
        assert_eq!(chosen_frames(&part(100, 600, 350), 1), [350]);
        assert_eq!(chosen_frames(&part(100, 600, 350), 2), [350, 200]);
        assert_eq!(chosen_frames(&part(100, 600, 350), 3), [350, 200, 499]);
        assert_eq!(chosen_frames(&part(10, 12, 10), 3), [10, 11], "a short part has no frame twice");
    }

    #[test]
    fn every_shot_is_looked_at_as_its_usable_parts_or_as_itself() {
        let shot = |from, to, looks: &[&str], key| qnc_shot_db::ShotRecord { from, to, looks: looks.iter().map(|l| l.to_string()).collect(), key_frame: key, brightness: 100.0, sharpness: 50.0, motion: 0.0 };
        let shots = qnc_shot_db::ClipShots {
            clip_id: "c".into(),
            media_uri: "qnc://x".into(),
            pictures: 10,
            shots: vec![shot(0, 300, &["still"], 100), shot(300, 400, &["moving"], 350), shot(400, 410, &["moving"], 405), shot(410, 600, &["black"], 500)],
            parts: vec![part(20, 200, 80)],
            error: String::new(),
            took_ms: 0,
        };
        let units = units(&shots);
        let shown: Vec<(u64, u64, &str)> = units.iter().map(|u| (u.from, u.to, u.camera.as_str())).collect();
        assert_eq!(shown, [(20, 200, "steady"), (200, 300, "still"), (300, 400, "moving")], "the part of the first shot and its rest after it (before it too short), the moving shot as it is; too short and black left out");
    }

    #[test]
    fn the_mean_of_unit_vectors_is_unit_length() {
        let m = mean([&[1.0f32, 0.0][..], &[0.0, 1.0][..]].into_iter());
        assert!((m[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6 && (m[1] - m[0]).abs() < 1e-6);
    }
}
