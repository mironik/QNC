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
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use qnc_content_store::{Access, ContentTarget, ContentWriteTransport, StoredClip, WriteLease};
use qnc_work_settings::{ProductArea, SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.missing-posters";

/// Where a media URI is on this computer; `None` when it is not here.
pub type LocalMedia<'a> = dyn Fn(&str) -> Option<PathBuf> + 'a;

/// The bytes of a medium, read only and seekable (a medium on LAN/Intranet through its
/// transport).
pub trait MediaBytes: Read + Seek + Send {}
impl<T: Read + Seek + Send + ?Sized> MediaBytes for T {}

/// The bytes of a media URI through its source's transport; `None` when the caller cannot
/// open it (user 2026-11-10: posters of clips on LAN/Intranet).
pub type StreamMedia<'a> = dyn Fn(&str) -> Option<Box<dyn MediaBytes>> + 'a;

/// The head of an MXF read for its first picture: OP1a keeps the header and the first pictures
/// at its start (measured on an FX6 original: 16 MB give the poster).
const MXF_HEAD: u64 = 16 << 20;
/// A card poster read through the transport is a small JPEG.
const POSTER_BYTES: u64 = 16 << 20;

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
pub fn start(
    project: PosterProject,
    local_media: Box<dyn Fn(&str) -> Option<PathBuf> + Send>,
    stream_media: Box<dyn Fn(&str) -> Option<Box<dyn MediaBytes>> + Send>,
) {
    let _ = std::thread::Builder::new().name("qnc-missing-posters".into()).spawn(move || {
        let cancel = AtomicBool::new(false);
        match make_missing_posters(&project, &*local_media, &*stream_media, None, &cancel) {
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
    stream_media: &StreamMedia<'_>,
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
    // A medium the project made itself (a recorded OFF, a smooth join) is in the project
    // folder, which no card binding of the caller names (2026-11-10: every OFF poster
    // failed with "the clip's source is not on this computer").
    let project_source = settings.project_media_source_uri().map_err(|e| e.message)?;
    let local_media = |uri: &str| in_project_folder(uri, &project_source, &project_dir).or_else(|| local_media(uri));
    let local_media: &LocalMedia<'_> = &local_media;
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
            Some(uri) => copy_card_poster(&uri, local_media, &output).or_else(|error| {
                stream_media(&uri).map_or(Err(error), |mut media| copy_streamed_poster(&mut *media, &output))
            }),
            None => match client.read(&clip_id)? {
                Some(clip) => poster_from_start(&clip, settings, local_media, &output, cancel)
                    .or_else(|_| streamed_poster(&clip, settings, stream_media, &output, cancel)),
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

/// The poster of a clip whose medium is not on this computer (LAN/Intranet), from the bytes
/// its transport gives (user 2026-11-10). Only what the first picture needs is read into a
/// file of its own in the system's temp folder, removed afterwards: the head of an MXF, or
/// the first key picture of an MP4/QuickTime from its index (which may be at the file's end).
pub fn streamed_poster(
    clip: &StoredClip,
    settings: &WorkSettings,
    stream_media: &StreamMedia<'_>,
    output: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let filmstrip = qnc_filmstrip::plan_from_snapshot_with_artifact_name(
        &clip.clip.snapshot,
        &settings.product_uri(ProductArea::Filmstrip),
        Some(&clip.clip.name),
    )?;
    let mut media = stream_media(&filmstrip.source_uri)
        .ok_or(qnc_ui_text::text("import.source_clip_available_this_computer"))?;
    poster_from_bytes(&mut *media, filmstrip.source_timebase, output, cancel)
}

/// The poster of a medium read only through its bytes, into `output`.
pub fn poster_from_bytes(
    media: &mut dyn MediaBytes,
    timebase: qnc_frame_timebase::FrameTimebase,
    output: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let scratch = Scratch::new()?;
    let made = match first_picture_file(media, &scratch.0)? {
        Start::Medium(file) => qnc_poster_create::create_poster(&file, 0.0, timebase, output, cancel)?,
        Start::Picture(file, codec) => qnc_poster_create::create_poster_from_picture(&file, codec, output, cancel)?,
    };
    match made {
        true => Ok(()),
        false => Err(qnc_ui_text::text("import.this_computer_local_picture_extractor").into()),
    }
}

/// What the first picture of a medium is decoded from.
#[derive(Debug, PartialEq, Eq)]
pub enum Start {
    /// The head of the medium, decoded as the medium.
    Medium(PathBuf),
    /// One encoded key picture of this codec.
    Picture(PathBuf, &'static str),
}

/// A file in `folder` a decoder takes the first picture of the medium from.
pub fn first_picture_file(media: &mut dyn MediaBytes, folder: &Path) -> Result<Start, String> {
    let size = media.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let mut head = [0u8; 8];
    media.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    media.read_exact(&mut head).map_err(|e| e.to_string())?;
    if head[..4] == [0x06, 0x0e, 0x2b, 0x34] {
        let file = folder.join("start.mxf");
        media.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(&file).map_err(|e| e.to_string())?;
        std::io::copy(&mut media.take(MXF_HEAD.min(size)), &mut out).map_err(|e| e.to_string())?;
        return Ok(Start::Medium(file));
    }
    if &head[4..8] == b"ftyp" {
        let tracks = qnc_mp4_sound::read_index_from(&mut *media, size)?;
        let (picture, codec) = qnc_mp4_sound::first_key_picture(&mut *media, &tracks)?;
        let file = folder.join(format!("start.{codec}"));
        std::fs::File::create(&file)
            .and_then(|mut out| out.write_all(&picture))
            .map_err(|e| e.to_string())?;
        return Ok(Start::Picture(file, codec));
    }
    Err(qnc_ui_text::text("posters.stream_container_unknown").into())
}

/// A card's poster read through its transport into the project.
pub fn copy_streamed_poster(media: &mut dyn MediaBytes, output: &Path) -> Result<(), String> {
    if let Some(folder) = output.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let mut bytes = Vec::new();
    media.take(POSTER_BYTES).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    std::fs::write(output, bytes).map_err(|e| e.to_string())
}

/// An empty folder of its own in the system's temp folder, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, String> {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let folder = std::env::temp_dir().join(format!("qnc-stream-poster-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        Ok(Self(folder))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

/// The file of a medium under the project's own media source, inside the project folder.
pub fn in_project_folder(uri: &str, project_source: &str, project_dir: &Path) -> Option<PathBuf> {
    let reference = qnc_source_contract::SourceReference::from_uri(uri).ok()?;
    if reference.source_uri() != project_source {
        return None;
    }
    let file = project_dir.join(reference.relative_path());
    (inside_project(project_dir, &file).is_ok() && file.is_file()).then_some(file)
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
    fn a_medium_of_the_project_itself_is_found_in_the_project_folder() {
        let project = tempfile::tempdir().unwrap();
        let voice = project.path().join("products/voice");
        std::fs::create_dir_all(&voice).unwrap();
        std::fs::write(voice.join("P_OFF1_T1.mxf"), b"mxf").unwrap();
        let source = "qnc://local/source/project-p1";
        let uri = qnc_source_contract::SourceReference::new(source, "products/voice/P_OFF1_T1.mxf").unwrap().uri();
        assert_eq!(in_project_folder(&uri, source, project.path()), Some(voice.join("P_OFF1_T1.mxf")));
        let card = qnc_source_contract::SourceReference::new("qnc://local/source/card", "products/voice/P_OFF1_T1.mxf").unwrap().uri();
        assert_eq!(in_project_folder(&card, source, project.path()), None, "a card's medium is the caller's");
        let missing = qnc_source_contract::SourceReference::new(source, "products/voice/none.mxf").unwrap().uri();
        assert_eq!(in_project_folder(&missing, source, project.path()), None);
    }

    #[test]
    fn a_streamed_medium_gives_the_head_of_an_mxf_and_refuses_what_it_does_not_know() {
        let folder = tempfile::tempdir().unwrap();
        let mut mxf = vec![0x06, 0x0e, 0x2b, 0x34];
        mxf.resize(1000, 7);
        let Start::Medium(file) = first_picture_file(&mut std::io::Cursor::new(mxf.clone()), folder.path()).unwrap() else {
            panic!("an MXF is decoded as its head");
        };
        assert_eq!(std::fs::read(file).unwrap(), mxf, "a short MXF is taken whole");
        let other = first_picture_file(&mut std::io::Cursor::new(vec![1u8; 64]), folder.path());
        assert!(other.is_err(), "no guess for an unknown container");
        let output = folder.path().join("thumbs/c/poster.jpg");
        copy_streamed_poster(&mut std::io::Cursor::new(b"jpeg".to_vec()), &output).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"jpeg");
    }

    /// A clip read only through its bytes, as a LAN/Intranet transport gives them. Runs on a
    /// real medium: `QNC_STREAM_POSTER_SAMPLE` names an MXF or MP4 (read only).
    #[test]
    fn a_real_medium_read_only_as_bytes_gives_a_poster() {
        let Some(sample) = std::env::var_os("QNC_STREAM_POSTER_SAMPLE") else {
            return eprintln!("QNC_STREAM_POSTER_SAMPLE not set");
        };
        let folder = tempfile::tempdir().unwrap();
        let mut media = std::fs::File::open(sample).unwrap();
        let output = folder.path().join("poster.jpg");
        let timebase = qnc_frame_timebase::FrameTimebase::new(50, 1).unwrap();
        poster_from_bytes(&mut media, timebase, &output, &AtomicBool::new(false)).unwrap();
        assert!(std::fs::metadata(&output).unwrap().len() > 1000, "a picture, not a placeholder");
        if let Some(keep) = std::env::var_os("QNC_STREAM_POSTER_KEEP") {
            std::fs::copy(&output, keep).unwrap();
        }
    }

    #[test]
    fn a_poster_is_written_only_inside_the_project() {
        let project = Path::new("/p/project");
        assert!(inside_project(project, &project.join("products/thumbnails/c/poster.jpg")).is_ok());
        assert!(inside_project(project, &project.join("../other/poster.jpg")).is_err());
        assert!(inside_project(project, Path::new("/card/poster.jpg")).is_err());
    }
}
