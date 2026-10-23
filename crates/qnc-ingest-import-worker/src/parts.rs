//! A clip with marked parts (Ingest SubClip, user 2026-10-23): only its parts are brought
//! into the project, each cut from the original in the original's container (MXF by
//! `qnc-mxf-cut`, MP4 and QuickTime by `qnc-mp4-cut`) with one second of handles, from
//! the intra picture at or before the handle on. Each part is described by one probe of
//! the new file (still the Ingest process) and becomes its own clip of the catalog; the
//! clip itself is not imported. The original is only read; parts have no proxy.

use crate::{inside_project, safe_name, MediaOpener};
use qnc_ingest_parts::Part;
use qnc_ingest_store::content::{CatalogClip, ImportedPart, StoredClip};
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_media_metadata::{FrameCount, MediaRepresentation, StreamDetails};
use qnc_media_records::{Binding, Phase, Snapshot};
use std::{fs, path::Path, sync::atomic::AtomicBool};

/// The folder of the project the parts are written to (they are originals of their own).
const FOLDER: crate::Folder = crate::Folder::Original;

pub(crate) fn import_parts(
    clip: &StoredClip,
    parts: &[Part],
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<Vec<ImportedPart>, String> {
    let original = &clip.clip.snapshot.metadata.original;
    let (frames, rate) = picture(original).ok_or(qnc_ui_text::text("import.original_stored_picture_count_frame_rate"))?;
    let source = opener.local_path(&original.media_uri).ok_or(qnc_ui_text::text("cut.needs_local_original"))?;
    let tags: Vec<(String, String)> = original.tags.iter().map(|(key, fact)| (key.clone(), fact.value.clone())).collect();
    let timecode = qnc_source_timecode::SourceTimecode::from_tags(tags.iter().map(|(k, v)| (k.as_str(), v.as_str())), rate);
    let handle = (rate.0 as f64 / rate.1.max(1) as f64).round() as u64; // one second
    let directory = project_dir.join(FOLDER.name());
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut imported = Vec::with_capacity(parts.len());
    for part in parts {
        let from = part.in_frame.saturating_sub(handle);
        let to = (part.out_frame + handle).min(frames);
        let name = safe_name(&part.part_id, &original.media_uri);
        let file = directory.join(&name);
        inside_project(project_dir, &file)?;
        let first = cut(&source, &file, (from, to), rate, timecode.as_ref(), cancel)?;
        let uri = qnc_source_reader::SourceReference::new(
            &plan.settings.project_media_source_uri().map_err(|e| e.to_string())?,
            &format!("{}/{name}", FOLDER.name()),
        )
        .map_err(|e| e.to_string())?
        .uri();
        let described = opener.describe_with_evidence(&uri, &file).and_then(|(media, evidence)| {
            match picture(&media) {
                Some((_, cut_rate)) if cut_rate == rate => Ok((media, evidence)),
                _ => Err(qnc_ui_text::text("import.optimised_copy_picture_count_frame_rate").to_string()),
            }
        });
        let (media, evidence) = match described {
            Ok(described) => described,
            Err(error) => {
                let _ = fs::remove_file(&file);
                return Err(error);
            }
        };
        let mut record = part_clip(clip, part, &uri, media, evidence, plan)?;
        // The poster of the part: the picture at its IN (the intra picture nearest it).
        record.thumbnail_uri = poster(&file, part, first, rate, plan, project_dir, cancel);
        imported.push(ImportedPart { clip: record, media_uri: uri });
    }
    Ok(imported)
}

/// The poster of a part made from its file at its IN; `None` when it cannot be made (the
/// background application makes the missing posters later, from the start of the part).
fn poster(file: &Path, part: &Part, first: u64, rate: (u64, u64), plan: &IngestWorkPlan, project_dir: &Path, cancel: &AtomicBool) -> Option<String> {
    let output = qnc_missing_posters::poster_file(&plan.settings, project_dir, &part.part_id).ok()?;
    let timebase = qnc_frame_timebase::FrameTimebase::new(rate.0 as i64, rate.1 as i64).ok()?;
    let seek = part.in_frame.saturating_sub(first) as f64 * rate.1 as f64 / rate.0.max(1) as f64;
    qnc_poster_create::create_poster(file, seek, timebase, &output, cancel).ok().filter(|made| *made)?;
    Some(format!("{}/{}/poster.jpg", plan.thumbnails_uri.trim_end_matches('/'), part.part_id))
}

/// Cuts frames `from..to` of the original into `file`; the first frame it starts on.
fn cut(source: &Path, file: &Path, (from, to): (u64, u64), rate: (u64, u64), timecode: Option<&qnc_source_timecode::SourceTimecode>, cancel: &AtomicBool) -> Result<u64, String> {
    if let Some(index) = qnc_mxf_index::MxfIndex::read(source)? {
        let plan = qnc_mxf_cut::plan(&index, from, to)?;
        qnc_mxf_cut::write(source, &index, &plan, file, cancel)?;
        return Ok(plan.first_frame);
    }
    let extension = source.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("mp4" | "mov")) {
        return Err(qnc_ui_text::text("cut.container_not_supported").into());
    }
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    let first = qnc_mp4_cut::intra_at_or_before(&ffmpeg, source, from, rate)?;
    let label = timecode.filter(|tc| tc.start_frame.is_some()).map(|tc| tc.label(first));
    qnc_mp4_cut::write(&ffmpeg, source, file, (first, to), rate, label.as_deref(), cancel)?;
    Ok(first)
}

/// The catalog record of a part: the clip's record with the part's name, its media in the
/// project as the original and its one probe as the evidence; no proxy, no poster yet.
fn part_clip(
    clip: &StoredClip,
    part: &Part,
    uri: &str,
    media: MediaRepresentation,
    evidence: qnc_media_metadata::Evidence,
    plan: &IngestWorkPlan,
) -> Result<CatalogClip, String> {
    let parent = &clip.clip;
    let mut metadata = parent.snapshot.metadata.clone();
    metadata.clip_id = part.part_id.clone();
    metadata.original = media;
    metadata.proxy = None;
    metadata.evidence = vec![evidence];
    let report = qnc_media_records::inspect(&metadata, Phase::Final).map_err(|e| e.to_string())?;
    let snapshot = Snapshot {
        binding: Binding { original_uri: uri.to_string(), proxy_uri: None, ..parent.snapshot.binding.clone() },
        revision: 2,
        phase: Phase::Final,
        completeness: qnc_media_records::completeness(&report),
        metadata,
        report,
        recorded_at_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64),
    };
    Ok(CatalogClip {
        name: part.name.clone(),
        source_uri: plan.settings.project_media_source_uri().map_err(|e| e.to_string())?,
        thumbnail_uri: None,
        snapshot,
        ..parent.clone()
    })
}

/// The exact frame count and frame rate of the first picture stream.
fn picture(media: &MediaRepresentation) -> Option<(u64, (u64, u64))> {
    media.streams.iter().find_map(|stream| match &stream.details {
        StreamDetails::Video(video) => {
            let frames = match video.frame_count.as_ref()?.value {
                FrameCount::Exact(frames) => frames,
                _ => return None,
            };
            let rate = &video.frame_rate.as_ref()?.value;
            Some((frames, (u64::try_from(rate.fps_num).ok()?, u64::try_from(rate.fps_den).ok()?)))
        }
        _ => None,
    })
}
