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

/// The side of the square picture the model is given.
const PICTURE: [u32; 2] = [224, 224];

/// Looks at every clip of the project whose usable parts are not looked at yet by the model of
/// this host; returns how many it kept. Nothing is done when the project has AI off or this host
/// has no shot content engine.
pub fn look_missing(reader: &SettingsReader, settings: &WorkSettings, local_media: &LocalMedia<'_>, lease: Option<WriteLease>, pace: &Pace<'_>) -> Result<usize, String> {
    if !settings.ai_enabled() {
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
    let db = ProjectDbTarget::for_project(reader, settings)?;
    let looked: Vec<String> = qnc_shot_content_db::read_all(&db, &model)?.into_iter().map(|clip| clip.clip_id).collect();
    let mut remaining: Vec<String> = qnc_shot_db::read_analysed(&db)?.into_iter().filter(|clip| !looked.contains(clip)).collect();
    // The label vectors the forms sort the B-roll by (no model in a form): made again only when
    // the list of labels says something else than the kept ones.
    let labels = qnc_shot_labels::installed_path().and_then(|path| qnc_shot_labels::Labels::read(&path)).ok();
    let stored = qnc_shot_content_db::read_labels(&db, &model)?;
    let stale = labels.as_ref().filter(|list| !stored.as_ref().is_some_and(|set| same_labels(set, list)));
    if remaining.is_empty() && stale.is_none() {
        return Ok(0);
    }
    let program = qnc_plugin_process::beside_application(&engine.program)?;
    let client = qnc_shot_content::ContentClient::new(&program);
    let mut catalog = qnc_content_store::ContentTarget::for_project(reader, settings)?.open(qnc_content_store::Access::ReadOnly)?;
    let label_texts = match stale {
        Some(list) => {
            let texts = client.texts(&qnc_shot_content::TextsRequest { model: engine.model.clone(), texts: list.prompts() })?;
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
    let mut kept = 0;
    while !remaining.is_empty() {
        if !wait_for_player(pace) {
            break;
        }
        let at = (pace.wanted)().and_then(|clip| remaining.iter().position(|id| *id == clip)).unwrap_or(0);
        let clip_id = remaining.remove(at);
        let Some(shots) = qnc_shot_db::read_clip(&db, &clip_id)? else { continue };
        if shots.parts.is_empty() {
            // Nothing usable to look at (or the analysis failed): kept as looked at, with why.
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
        let looked = look_file(&clip_id, codec, &path, &shots.parts, engine.pictures, &engine.model, &client, pace);
        if pace.cancel.load(Ordering::Relaxed) {
            break;
        }
        let content = match looked {
            Ok(parts) => ClipContent { clip_id: clip_id.clone(), model: model.clone(), parts, error: String::new(), took_ms: 0 },
            Err(error) => ClipContent { clip_id: clip_id.clone(), model: model.clone(), parts: Vec::new(), error, took_ms: 0 },
        };
        let content = ClipContent { took_ms: started.elapsed().as_millis() as u64, ..content };
        log(format!("shot-content clip={clip_id} parts={} ms={} error={}", content.parts.len(), content.took_ms, content.error));
        if let (Some(list), Some(texts)) = (&labels, &label_texts) {
            for part in &content.parts {
                let best: Vec<String> = list.best(&texts.logits(&part.vector)).into_iter().take(3).map(|(code, score)| format!("{code}:{score:.2}")).collect();
                log(format!("shot-content clip={clip_id} part={}-{} frames={:?} labels={}", part.from, part.to, part.frames, best.join(",")));
            }
        }
        qnc_shot_content_db::save(&db, content, lease.clone())?;
        kept += 1;
    }
    Ok(kept)
}

/// Whether the kept label vectors are of the same labels and prompts, in the same order.
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
pub fn look_file(clip_id: &str, codec: &str, path: &Path, parts: &[PartRecord], count: u8, model: &Path, client: &qnc_shot_content::ContentClient, pace: &Pace<'_>) -> Result<Vec<PartContent>, String> {
    let index = qnc_mxf_index::MxfIndex::read(path)?.ok_or_else(|| qnc_ui_text::text("shots.no_index").to_string())?;
    let frames = index.entries.len() as u64;
    if frames == 0 {
        return Err(qnc_ui_text::text("shots.no_index").into());
    }
    let shown = |entry: usize| (entry as i64 + index.entries[entry].temporal_offset as i64).clamp(0, frames as i64 - 1) as u64;
    // Every part's pictures: the intra picture nearest each chosen frame, inside the part.
    let mut wanted: Vec<(usize, usize, u64)> = Vec::new(); // (part, entry, shown frame)
    for (n, part) in parts.iter().enumerate() {
        let mut taken: Vec<usize> = Vec::new();
        for frame in chosen_frames(part, count) {
            let Some(entry) = index.intra_nearest(frame) else { continue };
            let at = shown(entry);
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
        let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut vectors = Vec::with_capacity(wanted.len());
        for batch in wanted.chunks(qnc_shot_content::MOST_AT_ONCE) {
            if !wait_for_player(pace) {
                return Err(qnc_ui_text::text("shots.stopped").into());
            }
            let _ = fs::remove_dir_all(&temp);
            fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
            let mut inputs = Vec::with_capacity(batch.len());
            for (n, (_, entry, _)) in batch.iter().enumerate() {
                let picture = qnc_mxf_index::read_package(&mut file, &index, *entry, true, false)?
                    .picture
                    .ok_or_else(|| qnc_ui_text::text("shots.no_index").to_string())?;
                let input = temp.join(format!("{n:03}.{codec}"));
                fs::write(&input, picture).map_err(|e| e.to_string())?;
                inputs.push(input);
            }
            extractor.decode_pictures_with_cancel(codec, &inputs, PICTURE, &temp, pace.cancel)?;
            let decoded: Vec<PathBuf> = (0..batch.len()).map(|n| temp.join(format!("{n:03}.jpg"))).collect();
            if let Some(missing) = decoded.iter().find(|file| !file.is_file()) {
                return Err(format!("{}: {}", qnc_ui_text::text("shots.no_pictures"), missing.display()));
            }
            vectors.extend(client.pictures(&qnc_shot_content::PicturesRequest { model: model.to_path_buf(), pictures: decoded })?);
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
                frames: of_part.iter().map(|(_, at)| *at).collect(),
                vector: mean(of_part.iter().map(|(vector, _)| vector.as_slice())),
            })
        })
        .collect())
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
    fn a_part_is_looked_at_its_sharpest_picture_first_then_in_from_its_ends() {
        assert_eq!(chosen_frames(&part(100, 600, 350), 1), [350]);
        assert_eq!(chosen_frames(&part(100, 600, 350), 2), [350, 200]);
        assert_eq!(chosen_frames(&part(100, 600, 350), 3), [350, 200, 499]);
        assert_eq!(chosen_frames(&part(10, 12, 10), 3), [10, 11], "a short part has no frame twice");
    }

    #[test]
    fn the_mean_of_unit_vectors_is_unit_length() {
        let m = mean([&[1.0f32, 0.0][..], &[0.0, 1.0][..]].into_iter());
        assert!((m[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6 && (m[1] - m[0]).abs() < 1e-6);
    }
}
