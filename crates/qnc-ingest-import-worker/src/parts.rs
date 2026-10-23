//! The Ingest SubClip parts brought in as a card of their own (user 2026-10-23: "make a card
//! with XML files as on the original card, the original name and a number, so the parts are
//! imported the same way as the originals"; in a folder of its own, `products.subclips`, so it
//! never collides with originals copied into `original`). Each part not cut yet is cut from
//! the original in the original's container (MXF by `qnc-mxf-cut`, MP4 and QuickTime by
//! `qnc-mp4-cut`), one second of handles, from the intra picture at or before them, into
//! `<SubClip>/<card>/` at the place the original has on its card; a Sony card gets its
//! documents (`qnc-sony-card-write`). Then that card is selected like any card (the camera
//! reader; one probe only where its records lack facts) and its clips are queued; they are
//! linked where they are. The original card is only read.

use crate::{inside_project, ConfigMediaOpener, MediaOpener};
use qnc_ingest_parts::{Part, PartsWriter};
use qnc_ingest_store::content::{Access, ContentTarget, StoredClip};
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_media_metadata::{FrameCount, MediaRepresentation, StreamDetails};
use qnc_work_settings::SettingsReader;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// How long one Select of a parts card may take.
const SELECT_WAIT: Duration = Duration::from_secs(600);

/// Cuts the parts not cut yet, selects their cards and queues their clips; the file names
/// of the parts cut.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bring_in(
    root: &Path,
    reader: &SettingsReader,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &ConfigMediaOpener,
    content: &ContentTarget,
    queue: &mut crate::TransportQueue,
    lease: Option<qnc_db_broker::WriteLease>,
    cancel: &AtomicBool,
) -> Result<Vec<String>, String> {
    let parts_db = qnc_db_broker::ProjectDbTarget::for_project(reader, &plan.settings)?;
    let uncut = qnc_ingest_parts::read_uncut(&parts_db)?;
    if uncut.is_empty() {
        return Ok(Vec::new());
    }
    let (subclips_dir, subclips) = plan.settings.subclips_location(project_dir).map_err(|e| e.to_string())?;
    let mut writer = PartsWriter::start(parts_db)?;
    if let Some(lease) = lease {
        writer = writer.leased(lease);
    }
    let mut catalog = content.open(Access::ReadOnly).map_err(|e| e.to_string())?;
    let mut cards = BTreeSet::new();
    let mut names = Vec::new();
    for part in &uncut {
        if cancel.load(Ordering::Relaxed) {
            return Err(qnc_ui_text::text("cut.interrupted").into());
        }
        let Some(clip) = catalog.read(&part.clip_id).map_err(|e| e.to_string())? else { continue };
        match cut_part(&clip, part, project_dir, &subclips_dir, opener, cancel) {
            Ok((card, name)) => {
                writer.mark_cut(&part.part_id, &name)?;
                cards.insert(format!("{subclips}/{card}"));
                names.push(name);
            }
            Err(error) => log(&format!("ingest-part cut failed part={} error={error}", part.part_id)),
        }
    }
    let source_uri = plan.settings.project_media_source_uri().map_err(|e| e.to_string())?;
    for card in &cards {
        select_card(root, reader, plan, &source_uri, card, cancel)?;
    }
    // The clips of the parts as Select recorded them, queued to be linked where they are.
    // Through the one reader of the catalog (C11): the parts by their names, not imported yet.
    let ids = qnc_content_read::ContentReader::for_project(reader, &plan.settings)?.not_imported_named(&source_uri, &names)?;
    if !ids.is_empty() {
        queue.select(ids)?;
        queue.queue_selected(qnc_import_fit::Needs::of(&plan.settings)?)?;
    }
    Ok(names)
}

/// Cuts one part onto the parts card: (the card's folder in the SubClip folder, the file
/// name of the part).
fn cut_part(clip: &StoredClip, part: &Part, project_dir: &Path, subclips_dir: &Path, opener: &dyn MediaOpener, cancel: &AtomicBool) -> Result<(String, String), String> {
    let original = &clip.clip.snapshot.metadata.original;
    let (frames, rate) = picture(original).ok_or(qnc_ui_text::text("import.original_stored_picture_count_frame_rate"))?;
    let source = opener.local_path(&original.media_uri).ok_or(qnc_ui_text::text("cut.needs_local_original"))?;
    let relative = qnc_source_reader::SourceReference::from_uri(&original.media_uri).map_err(|e| e.to_string())?.relative_path().to_string();
    // The root of the original card: the file without its place on the card.
    let card_root = relative
        .split('/')
        .filter(|step| !step.is_empty())
        .fold(source.clone(), |dir, _| dir.parent().map(Path::to_path_buf).unwrap_or(dir));
    let card = card_name(&clip.clip);
    let dest_card = subclips_dir.join(&card);
    let number: u32 = part.part_id.rsplit("-part-").next().and_then(|n| n.parse().ok()).unwrap_or(1);
    let handle = (rate.0 as f64 / rate.1.max(1) as f64).round() as u64; // one second
    let (from, to) = (part.in_frame.saturating_sub(handle), (part.out_frame + handle).min(frames));
    let tags: Vec<(String, String)> = original.tags.iter().map(|(key, fact)| (key.clone(), fact.value.clone())).collect();
    let timecode = qnc_source_timecode::SourceTimecode::from_tags(tags.iter().map(|(k, v)| (k.as_str(), v.as_str())), rate);
    match qnc_sony_card_write::card_of(&card_root, &source) {
        Some(sony) => {
            let dest = dest_card.join(&sony.card_relative);
            let files = qnc_sony_card_write::part_files(&sony, &dest, number).ok_or(qnc_ui_text::text("card.clip_not_in_mediapro"))?;
            inside_project(project_dir, &files.media)?;
            let (first, cut_frames) = cut(&source, &files.media, (from, to), rate, timecode.as_ref(), cancel)?;
            let poster = poster(&files.media, part, first, rate);
            let written = qnc_sony_card_write::write_documents(
                &sony,
                &dest,
                &files,
                qnc_sony_card_write::PartFrames { number, first, frames: cut_frames },
                poster.as_deref(),
            );
            if let Some(poster) = poster {
                let _ = fs::remove_file(poster);
            }
            written?;
            Ok((card, file_name(&files.media)))
        }
        // A file of its own (no camera documents): the part where the original is on its card.
        None => {
            let stem = source.file_stem().and_then(|s| s.to_str()).ok_or("media name")?;
            let extension = source.extension().and_then(|s| s.to_str()).unwrap_or_default();
            let place = Path::new(&relative).parent().map(Path::to_path_buf).unwrap_or_default();
            let media = dest_card.join(place).join(format!("{}.{extension}", qnc_sony_card_write::part_stem(stem, number)));
            inside_project(project_dir, &media)?;
            cut(&source, &media, (from, to), rate, timecode.as_ref(), cancel)?;
            Ok((card, file_name(&media)))
        }
    }
}

/// Cuts frames `from..to` into `file`: (the frame of the original it starts on, its frames).
fn cut(source: &Path, file: &Path, (from, to): (u64, u64), rate: (u64, u64), timecode: Option<&qnc_source_timecode::SourceTimecode>, cancel: &AtomicBool) -> Result<(u64, u64), String> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if let Some(index) = qnc_mxf_index::MxfIndex::read(source)? {
        let plan = qnc_mxf_cut::plan(&index, from, to)?;
        qnc_mxf_cut::write(source, &index, &plan, file, cancel)?;
        return Ok((plan.first_frame, plan.frames));
    }
    let extension = source.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("mp4" | "mov")) {
        return Err(qnc_ui_text::text("cut.container_not_supported").into());
    }
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    let first = qnc_mp4_cut::intra_at_or_before(&ffmpeg, source, from, rate)?;
    let label = timecode.filter(|tc| tc.start_frame.is_some()).map(|tc| tc.label(first));
    let frames = qnc_mp4_cut::write(&ffmpeg, source, file, (first, to), rate, label.as_deref(), cancel)?;
    Ok((first, frames))
}

/// The poster of a part from its file at its IN, in the system's scratch folder.
fn poster(file: &Path, part: &Part, first: u64, rate: (u64, u64)) -> Option<PathBuf> {
    let output = std::env::temp_dir().join(format!("qnc-part-poster-{}-{}.jpg", std::process::id(), part.part_id));
    let timebase = qnc_frame_timebase::FrameTimebase::new(rate.0 as i64, rate.1 as i64).ok()?;
    let seek = part.in_frame.saturating_sub(first) as f64 * rate.1 as f64 / rate.0.max(1) as f64;
    qnc_poster_create::create_poster(file, seek, timebase, &output, &AtomicBool::new(false)).ok().filter(|made| *made)?;
    Some(output)
}

/// Select of the parts card `card` (its folder from the project folder), as of any card.
fn select_card(root: &Path, reader: &SettingsReader, plan: &IngestWorkPlan, source_uri: &str, card: &str, cancel: &AtomicBool) -> Result<(), String> {
    let config = qnc_ingest_select::SelectionConfig::for_project(root, reader, &plan.settings).map_err(|e| e.to_string())?;
    let target = qnc_ingest_select::SelectTarget::for_project(reader, &plan.settings)?;
    let registry = std::sync::Arc::new(qnc_ingest_cameras::registry(root)?);
    let selected = qnc_source_reader::SourceReference::new(source_uri, card).map_err(|e| e.to_string())?;
    let mut session = qnc_ingest_select::SelectSession::default();
    session.start(config, selected, target, registry)?;
    let deadline = Instant::now() + SELECT_WAIT;
    while Instant::now() < deadline && !cancel.load(Ordering::Relaxed) {
        for event in session.poll(64) {
            match event {
                qnc_ingest_select::Event::Finished(result) => return result.map(|_| ()),
                qnc_ingest_select::Event::Warning(warning) => log(&format!("ingest-part select warning={warning}")),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    session.cancel();
    Err(qnc_ui_text::text("cut.interrupted").into())
}

/// The folder of the parts card of a clip's card: the label and serial number of its medium,
/// made safe; never a drive letter or another name of one operating system (user 2026-10-23).
fn card_name(clip: &qnc_ingest_store::content::CatalogClip) -> String {
    let name = Some(clip.volume_name.as_str()).filter(|n| !n.trim().is_empty()).unwrap_or("card");
    let name = if clip.serial_number.trim().is_empty() { name.to_string() } else { format!("{name}_{}", clip.serial_number.trim()) };
    name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' }).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn log(line: &str) {
    eprintln!("{line}");
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
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
