//! Posters for clips whose source has none (a single file, an XDCAM disc): made after
//! Select, before Uvezi, so every form shows the clip with a picture (user 2026-10-09; v5
//! shows a coloured card instead). Its own block (user 2026-10-18): the one place that
//! makes a poster from the start of a clip; the import and the background worker use it,
//! so a change elsewhere does not break it.
//!
//! The poster is the start of the clip, written into the project thumbnails folder (never
//! into the source, which stays read only) and recorded through the content write
//! transport, under the caller's lease, only for a clip that has no poster yet. Where the
//! media is on this computer the caller says (its source bindings); this block reads no
//! card index and runs no probe.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use qnc_content_store::{Access, ContentTarget, ContentWriteTransport, StoredClip, WriteLease};
use qnc_work_settings::{ProductArea, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.missing-posters";

/// Where a media URI is on this computer; `None` when it is not here.
pub type LocalMedia<'a> = dyn Fn(&str) -> Option<PathBuf> + 'a;

/// Makes the posters of the active project's clips that have none. Returns how many were
/// made; a clip whose poster cannot be made is told and skipped.
pub fn make_missing_posters(
    root: &Path,
    local_media: &LocalMedia<'_>,
    lease: Option<WriteLease>,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    let active_project =
        qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
    let snapshot = active_project.read().map_err(|e| e.to_string())?;
    let reader = active_project.settings_reader().clone();
    let settings = snapshot.settings;
    // The catalog is read through its one public reader.
    let wanted = qnc_content_read::ContentReader::for_project(&reader, &settings)?.clips_without_poster()?;
    if wanted.is_empty() {
        return Ok(0);
    }
    let project_dir = reader
        .local_workspace_dir(&settings)
        .map_err(|e| e.to_string())?
        .ok_or(qnc_ui_text::text("import.import_needs_local_access_project_folder"))?;
    let target = ContentTarget::for_project(&reader, &settings)?;
    let mut client = target.open(Access::ReadOnly)?;
    let mut writer = Writer::start(target, lease)?;
    let thumbnails_uri = settings.product_uri(ProductArea::Thumbnails);
    let mut made = 0;
    for clip_id in wanted {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Some(clip) = client.read(&clip_id)? else { continue };
        let output = poster_file(&settings, &project_dir, &clip_id)?;
        match poster_from_start(&clip, &settings, local_media, &output, cancel) {
            Ok(()) => {
                let uri = format!("{}/{clip_id}/poster.jpg", thumbnails_uri.trim_end_matches('/'));
                writer.set_poster(clip_id, uri)?;
                made += 1;
            }
            Err(error) => eprintln!("poster {clip_id}: {error}"),
        }
    }
    Ok(made)
}

/// The poster file of a clip in the project thumbnails folder; never outside the project.
pub fn poster_file(settings: &WorkSettings, project_dir: &Path, clip_id: &str) -> Result<PathBuf, String> {
    let output = settings
        .product_local_dir(project_dir, ProductArea::Thumbnails)
        .join(clip_id)
        .join("poster.jpg");
    inside_project(project_dir, &output)?;
    Ok(output)
}

/// The poster of a clip from the start of its media (the medium its filmstrip uses), at
/// poster resolution, into `output`.
pub fn poster_from_start(
    clip: &StoredClip,
    settings: &WorkSettings,
    local_media: &LocalMedia<'_>,
    output: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let filmstrip = qnc_filmstrip::plan_from_snapshot_with_artifact_name(
        &clip.clip.snapshot,
        &settings.product_uri(ProductArea::Filmstrip),
        Some(&clip.clip.name),
    )?;
    let source = local_media(&filmstrip.source_uri)
        .ok_or(qnc_ui_text::text("import.source_clip_available_this_computer"))?;
    match qnc_poster_create::create_poster(&source, 0.0, filmstrip.source_timebase, output, cancel)? {
        true => Ok(()),
        false => Err(qnc_ui_text::text("import.this_computer_local_picture_extractor").into()),
    }
}

/// A path the project may be written at: inside its folder, no step back out of it.
pub fn inside_project(project_dir: &Path, path: &Path) -> Result<(), String> {
    let plain = path
        .components()
        .all(|part| !matches!(part, std::path::Component::ParentDir));
    if plain && path.starts_with(project_dir) {
        Ok(())
    } else {
        Err(qnc_ui_text::text("import.writing_allowed_only_into_project_folder").into())
    }
}

/// The poster records, through the serialized content write transport.
struct Writer {
    transport: ContentWriteTransport,
    sequence: u64,
}

impl Writer {
    fn start(target: ContentTarget, lease: Option<WriteLease>) -> Result<Self, String> {
        Ok(Self {
            transport: ContentWriteTransport::start_leased(target, lease)?,
            sequence: 0,
        })
    }

    fn set_poster(&mut self, clip_id: String, uri: String) -> Result<(), String> {
        self.sequence += 1;
        let key = format!("poster-{}", self.sequence);
        self.transport.set_poster(key.clone(), clip_id, uri)?;
        loop {
            for completion in self.transport.poll() {
                if completion.key == key {
                    return completion.result.map(|_| ());
                }
            }
            if !self.transport.has_pending() {
                return Err(qnc_ui_text::text("db.content_writer_returned_result").into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poster_is_written_only_inside_the_project() {
        let project = Path::new("/p/project");
        assert!(inside_project(project, &project.join("products/thumbnails/c/poster.jpg")).is_ok());
        assert!(inside_project(project, &project.join("../other/poster.jpg")).is_err());
        assert!(inside_project(project, Path::new("/card/poster.jpg")).is_err());
    }
}
