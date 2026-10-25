//! The optimized copy of a clip (user 2026-10-09: `storage.ingest_media` = `optimized` or
//! `optimized_original`): made from the original into the project folder `optimized`,
//! described by one probe of the new file (still the Ingest process; the original is not
//! probed again) and checked against the saved original before it is recorded: the same
//! number of frames, the same frame rate and the same start timecode, so frame N of the
//! copy is frame N of the original and the export can take the original.

use crate::{inside_project, safe_name, MediaOpener};
use qnc_ingest_store::content::StoredClip;
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_media_metadata::{FrameCount, MediaRepresentation, StreamDetails};
use std::{fs, path::Path, sync::atomic::AtomicBool, sync::OnceLock};

/// The folder of the project where optimized copies are written (v4 layout beside
/// `original` and `proxy`).
pub(crate) const FOLDER: &str = "optimized";

pub(crate) fn make(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<MediaRepresentation, String> {
    let original = &clip.clip.snapshot.metadata.original;
    let source = opener
        .local_path(&original.media_uri)
        .ok_or(qnc_ui_text::text("import.optimised_copy_needs_original_this_computer"))?;
    let stem = safe_name(clip.clip.id(), &original.media_uri);
    let name = format!("{}.mov", stem.rsplit_once('.').map_or(stem.as_str(), |(stem, _)| stem));
    let directory = project_dir.join(FOLDER);
    let file = directory.join(&name);
    inside_project(project_dir, &file)?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    // Straight under its name: the protected project folder allows no rename (audit
    // 2026-10-25: a `.partial` renamed was "Access is denied" on every project). A copy cut
    // short is never recorded (the check below needs every frame) and the next import
    // writes over it.
    optimizer()?.make(&source, &file, cancel)?;
    let uri = qnc_source_reader::SourceReference::new(
        &plan.settings.project_media_source_uri().map_err(|e| e.to_string())?,
        &format!("{FOLDER}/{name}"),
    )
    .map_err(|e| e.to_string())?
    .uri();
    let checked = opener
        .describe(&uri, &file)
        .and_then(|media| same_frames(original, &media).map(|()| media));
    if checked.is_err() {
        let _ = fs::remove_file(&file);
    }
    checked
}

/// One optimizer per process: choosing the encoder runs short test encodes.
fn optimizer() -> Result<&'static qnc_media_optimize::Optimizer, String> {
    static OPTIMIZER: OnceLock<Result<qnc_media_optimize::Optimizer, String>> = OnceLock::new();
    OPTIMIZER
        .get_or_init(qnc_media_optimize::Optimizer::installed)
        .as_ref()
        .map_err(Clone::clone)
}

/// Frame N of the copy is frame N of the original: same exact frame count, same frame
/// rate, same start timecode (when the original has one).
pub(crate) fn same_frames(original: &MediaRepresentation, copy: &MediaRepresentation) -> Result<(), String> {
    let (frames, rate) = picture(original).ok_or(qnc_ui_text::text("import.original_stored_picture_count_frame_rate"))?;
    let (copy_frames, copy_rate) = picture(copy).ok_or(qnc_ui_text::text("import.optimised_copy_picture_count_frame_rate"))?;
    if (frames, rate) != (copy_frames, copy_rate) {
        return Err(qnc_ui_text::fill(
            "import.optimized_not_frame_exact",
            &[("copy_frames", &copy_frames.to_string()), ("copy_rate", &format!("{}/{}", copy_rate.0, copy_rate.1)), ("frames", &frames.to_string()), ("rate", &format!("{}/{}", rate.0, rate.1))],
        ));
    }
    let tags = |media: &MediaRepresentation| -> Vec<(String, String)> {
        media.tags.iter().map(|(key, fact)| (key.clone(), fact.value.clone())).collect()
    };
    let (original_tags, copy_tags) = (tags(original), tags(copy));
    let timecode = qnc_source_timecode::SourceTimecode::from_tags(
        original_tags.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        (rate.0, rate.1),
    );
    if timecode.and_then(|tc| tc.proxy_matches(copy_tags.iter().map(|(k, v)| (k.as_str(), v.as_str())))) == Some(false) {
        return Err(qnc_ui_text::text("import.optimised_copy_does_start_at_timecode").into());
    }
    Ok(())
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
