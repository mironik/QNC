use crate::*;
use std::{fs, io::{Read, Write}, path::Path, sync::atomic::{AtomicBool, Ordering}};

const CHUNK: usize = 1024 * 1024;

/// Well inside the store lease, so a slow copy never loses its clip.
const HEARTBEAT_EVERY: std::time::Duration = std::time::Duration::from_secs(20);

pub(crate) fn copy_into(
    opener: &dyn MediaOpener,
    source_uri: &str,
    destination: &Path,
    cancel: &AtomicBool,
    beat: &mut dyn FnMut(),
) -> Result<(), String> {
    let mut source = opener.open(source_uri)?;
    let mut file = fs::File::create(destination).map_err(|e| e.to_string())?;
    let mut buffer = vec![0_u8; CHUNK];
    let mut last_beat = std::time::Instant::now();
    loop {
        while opener.paused() {
            if cancel.load(Ordering::Relaxed) {
                return Err(qnc_ui_text::text("import.import_interrupted").into());
            }
            if last_beat.elapsed() >= HEARTBEAT_EVERY {
                beat();
                last_beat = std::time::Instant::now();
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(qnc_ui_text::text("import.import_interrupted").into());
        }
        let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        if last_beat.elapsed() >= HEARTBEAT_EVERY {
            beat();
            last_beat = std::time::Instant::now();
        }
    }
}

/// Executes the action for one clip and returns the URI of the imported media.
pub fn import_clip(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<String, String> {
    import_clip_beating(clip, plan, project_dir, opener, cancel, &mut || {})
}

/// Like `import_clip`, and calls `beat` regularly while a long copy runs.
pub fn import_clip_beating(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
    beat: &mut dyn FnMut(),
) -> Result<String, String> {
    import_media(clip, plan, project_dir, opener, cancel, beat).map(|imported| imported.media_uri)
}

/// What one import made: the imported media (linked or copied), what it is a copy of,
/// and the optimized copy when the project asks for one.
pub(crate) struct Imported {
    pub(crate) media_uri: String,
    pub(crate) copy_of: Option<ImportedCopy>,
    pub(crate) optimized: Option<MediaRepresentation>,
}

pub(crate) fn import_media(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
    beat: &mut dyn FnMut(),
) -> Result<Imported, String> {
    let plain = |media_uri, copy_of| Imported { media_uri, copy_of, optimized: None };
    match action_for(clip, plan)? {
        Action::Link { media_uri } => Ok(plain(media_uri, None)),
        Action::Optimize { with_original } => {
            let original = clip.clip.snapshot.binding.original_uri.clone();
            let optimized = optimize::make(clip, plan, project_dir, opener, cancel)?;
            let (media_uri, copy_of) = if with_original {
                (copy_media(clip, plan, project_dir, opener, cancel, beat, &original, Folder::Original)?, Some(ImportedCopy::Original))
            } else {
                (original, None)
            };
            Ok(Imported { media_uri, copy_of, optimized: Some(optimized) })
        }
        Action::Copy { source_uri, folder } => {
            copy_media(clip, plan, project_dir, opener, cancel, beat, &source_uri, folder)
                .map(|uri| plain(uri, Some(folder.copy())))
        }
    }
}

/// Copies one source medium into the project folder and returns its project URI.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_media(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
    beat: &mut dyn FnMut(),
    source_uri: &str,
    folder: Folder,
) -> Result<String, String> {
    {
        {
            let name = safe_name(clip.clip.id(), source_uri);
            let directory = project_dir.join(folder.name());
            inside_project(project_dir, &directory.join(&name))?;
            fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            copy_into(opener, source_uri, &directory.join(&name), cancel, beat)?;
            // The copy is read like media on a card: through the project folder as a source.
            let source = plan
                .settings
                .project_media_source_uri()
                .map_err(|e| e.to_string())?;
            let reference = qnc_source_reader::SourceReference::new(
                &source,
                &format!("{}/{name}", folder.name()),
            )
            .map_err(|e| e.to_string())?;
            Ok(reference.uri())
        }
    }
}

/// Writes go only where the project says: below the project folder. A source (a card) is read
/// only, so nothing is ever written, created or changed there.
/// The one guard of where the project may be written (the poster block's).
pub(crate) use qnc_missing_posters::inside_project;

/// The poster of a clip in the project. The card is read only and is never touched.
/// - The card has a poster: the poster is copied into the project thumbnail location, so the
///   clip keeps its picture when the card is gone, even when the media stays linked.
/// - The card has no poster: it is created from a key frame of the media, the way the
///   filmstrip frames are made, at poster resolution.
/// A poster that cannot be made never fails the import.
pub fn import_poster(
    clip: &StoredClip,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Option<String> {
    let clip_id = clip.clip.id();
    let output = qnc_missing_posters::poster_file(&plan.settings, project_dir, clip_id).ok()?;
    let directory = output.parent()?.to_path_buf();
    let made = format!("{}/{clip_id}/poster.jpg", plan.thumbnails_uri.trim_end_matches('/'));
    match clip.clip.thumbnail_uri.as_deref() {
        // Already made in the project (a single file without a card poster): kept.
        Some(uri) if uri == made => {}
        Some(source_uri) => {
            fs::create_dir_all(&directory).ok()?;
            copy_into(opener, source_uri, &output, cancel, &mut || {}).ok()?;
        }
        // The start of the clip, by the one block that makes missing posters.
        None => qnc_missing_posters::poster_from_start(clip, &plan.settings, &|uri: &str| opener.local_path(uri), &output, cancel).ok()?,
    }
    Some(made)
}
