//! The posters of the clips in the project, right after Select (user 2026-10-18: "as
//! soon as a card is selected, its posters are checked; the missing ones are made at once,
//! and copied"). Its own block: the one place that brings a poster into the project; the
//! import and the background worker use it, so a change elsewhere does not break it.
//!
//! A poster the card has is copied into the project thumbnails folder; a clip whose card
//! has none (a single file, an XDCAM disc) gets one from the start of the clip. The source
//! stays read only. The project's poster is recorded through the content write transport,
//! under the caller's lease. Where the media is on this computer the caller says (its
//! source bindings); this block reads no card index and runs no probe.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use qnc_content_store::{Access, ContentTarget, ContentWriteTransport, StoredClip, WriteLease};
use qnc_work_settings::{ProductArea, SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.missing-posters";

/// Where a media URI is on this computer; `None` when it is not here.
pub type LocalMedia<'a> = dyn Fn(&str) -> Option<PathBuf> + 'a;

/// The project whose posters are brought in: its settings as read from its database.
#[derive(Debug, Clone)]
pub struct PosterProject {
    reader: SettingsReader,
    settings: WorkSettings,
}

impl PosterProject {
    pub fn new(reader: SettingsReader, settings: WorkSettings) -> Self {
        Self { reader, settings }
    }

    /// The active project of the QNC root, read from its database now.
    pub fn active(root: &Path) -> Result<Self, String> {
        let active_project =
            qnc_active_project_read::ActiveProjectReader::from_root(root).map_err(|e| e.to_string())?;
        let snapshot = active_project.read().map_err(|e| e.to_string())?;
        Ok(Self::new(active_project.settings_reader().clone(), snapshot.settings))
    }
}

/// Starts the block on its own thread (Select runs it when a card is selected and does not
/// wait for it).
pub fn start(project: PosterProject, local_media: Box<dyn Fn(&str) -> Option<PathBuf> + Send>) {
    let _ = std::thread::Builder::new().name("qnc-missing-posters".into()).spawn(move || {
        let cancel = AtomicBool::new(false);
        match make_missing_posters(&project, &*local_media, None, &cancel) {
            Ok(_) => {}
            Err(error) => eprintln!("posters: {error}"),
        }
    });
}

/// Brings into the project the poster of every clip whose poster is not there yet: the
/// card's is copied, a missing one is made. Returns how many; a clip whose poster cannot be
/// had now (its record has no timing yet) is told and asked again by the next call.
pub fn make_missing_posters(
    project: &PosterProject,
    local_media: &LocalMedia<'_>,
    lease: Option<WriteLease>,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    let (reader, settings) = (&project.reader, &project.settings);
    let thumbnails_uri = settings.product_uri(ProductArea::Thumbnails);
    let in_project = format!("{}/", thumbnails_uri.trim_end_matches('/'));
    // The catalog is read through its one public reader.
    let wanted: Vec<(String, Option<String>)> = qnc_content_read::ContentReader::for_project(reader, settings)?
        .clip_posters()?
        .into_iter()
        .filter(|(_, poster)| !poster.as_deref().is_some_and(|uri| uri.starts_with(&in_project)))
        .collect();
    if wanted.is_empty() {
        return Ok(0);
    }
    let project_dir = reader
        .local_workspace_dir(settings)
        .map_err(|e| e.to_string())?
        .ok_or(qnc_ui_text::text("import.import_needs_local_access_project_folder"))?;
    let target = ContentTarget::for_project(reader, settings)?;
    let mut client = target.open(Access::ReadOnly)?;
    let mut writer = Writer::start(target, lease)?;
    let mut made = 0;
    for (clip_id, card_poster) in wanted {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let output = poster_file(settings, &project_dir, &clip_id)?;
        let brought = match card_poster {
            Some(uri) => copy_card_poster(&uri, local_media, &output),
            None => match client.read(&clip_id)? {
                Some(clip) => poster_from_start(&clip, settings, local_media, &output, cancel),
                None => continue,
            },
        };
        match brought {
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

/// The card's poster copied into the project (the card is only read).
pub fn copy_card_poster(uri: &str, local_media: &LocalMedia<'_>, output: &Path) -> Result<(), String> {
    let source = local_media(uri).ok_or(qnc_ui_text::text("import.source_clip_available_this_computer"))?;
    if let Some(folder) = output.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&source, output).map(|_| ()).map_err(|e| e.to_string())
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
    fn the_cards_poster_is_copied_into_the_project_and_the_card_stays_as_it_was() {
        let card = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let poster = card.path().join("T0001.JPG");
        std::fs::write(&poster, b"jpeg").unwrap();
        let output = project.path().join("products/thumbnails/c1/poster.jpg");
        let local = |uri: &str| (uri == "qnc://local/source/card/file/T0001.JPG").then(|| poster.clone());
        copy_card_poster("qnc://local/source/card/file/T0001.JPG", &local, &output).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"jpeg");
        assert_eq!(std::fs::read(&poster).unwrap(), b"jpeg", "the card is only read");
        assert!(copy_card_poster("qnc://local/source/other/file/x.JPG", &local, &output).is_err(), "not on this computer");
    }

    #[test]
    fn a_poster_is_written_only_inside_the_project() {
        let project = Path::new("/p/project");
        assert!(inside_project(project, &project.join("products/thumbnails/c/poster.jpg")).is_ok());
        assert!(inside_project(project, &project.join("../other/poster.jpg")).is_err());
        assert!(inside_project(project, Path::new("/card/poster.jpg")).is_err());
    }
}
