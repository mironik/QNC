//! Prerender of the active project's story (user 2026-10-12): the story is read as the
//! player reads it (project database -> program input -> the player's program plan), cut
//! into units between cuts, and each unit not yet rendered is rendered from the camera
//! originals into the project's delivery format (`export.*`) under the project's
//! `products.prerender`, its file named by its fingerprint (the same unit wherever it
//! stands in the story). Which units are complete goes to the project database (next step);
//! this tool renders the units it is asked for.
//! Media is only read (a card stays read only); the project database is only read.
//!
//! `qnc-prerender [--root <QNC root>] [--units <n>] [--list 1]`

use std::{
    io,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

use qnc_broadcast_engine::ProgramPlan;
use qnc_content_store::{Access, ContentTarget};
use qnc_media_metadata::MediaRepresentation;
use qnc_media_stream::{CodecEndpoint, LocalSource, MediaStream, SourceReference};
use qnc_player_input::{InputReader, PlayerClipRecord, PlayerContentRead};
use qnc_program_render::Delivery;
use qnc_work_settings::SettingsReader;

type Result<T> = std::result::Result<T, String>;

/// The clip records of the active project, read only (as the player's preview reads them).
struct PlayerContent {
    target: std::result::Result<ContentTarget, String>,
}

impl PlayerContentRead for PlayerContent {
    fn read_clip(&self, clip_id: &str) -> std::result::Result<Option<PlayerClipRecord>, String> {
        let target = self.target.as_ref().map_err(Clone::clone)?;
        let stored = target.open(Access::ReadOnly)?.read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
            imported_copy_of: stored.imported_copy_of.map(|copy| match copy {
                qnc_content_store::ImportedCopy::Original => qnc_player_input::Representation::Original,
                qnc_content_store::ImportedCopy::Proxy => qnc_player_input::Representation::Proxy,
            }),
            imported_optimized: stored.imported_optimized,
        }))
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("qnc-prerender: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut root, mut limit, mut list) = (std::env::var_os("QNC_ROOT").map(PathBuf::from), usize::MAX, false);
    while let Some(arg) = args.next() {
        let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--root" => root = Some(PathBuf::from(value)),
            "--units" => limit = value.parse().map_err(|_| "--units N")?,
            "--list" => list = value == "1",
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let root = root.ok_or("QNC root missing (--root or QNC_ROOT)")?;
    let reader = SettingsReader::from_root(&root).map_err(|e| e.to_string())?;
    let settings = reader.read().map_err(|e| e.to_string())?;
    let project_dir = reader
        .local_workspace_dir(&settings)
        .map_err(|e| e.to_string())?
        .ok_or("Projekt nema lokalni direktorij.")?;
    let (out_dir, _) = settings.prerender_location(&project_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Mapa prerendera: {e}"))?;

    let started = Instant::now();
    let target = qnc_db_broker::ProjectDbTarget::for_project(&reader, &settings)?;
    let content = Arc::new(PlayerContent {
        target: ContentTarget::for_project(&reader, &settings),
    });
    let inputs = InputReader::with_content_reader(reader.clone(), content);
    let program = qnc_program_input::loader(target, settings.project_id.clone())(&inputs, &settings.workspace_db_uri)?;
    let plan = ProgramPlan::for_render(&program).map_err(|e| e.to_string())?;
    let units = plan.render_units().map_err(|e| e.to_string())?;
    println!("story: {} units, {} frames, read in {} ms", units.len(), program.playlist.duration_frames, started.elapsed().as_millis());

    if list {
        for unit in &units {
            let picture = unit.video.as_ref().map_or("black".to_string(), |v| format!("{} from {}", v.media.media_uri, v.first_frame));
            println!("unit {}..{}: {picture}", unit.record_in, unit.record_out);
            for part in &unit.audio {
                println!(
                    "    out {} <- {} stream {} channel {} from sample {}",
                    part.output + 1, part.media.media_uri, part.stream_index, part.channel_index, part.first_sample
                );
            }
        }
        return Ok(());
    }
    let bindings = qnc_source_bindings::load(&root)?;
    let mut roots: Vec<(String, PathBuf)> = bindings
        .sources
        .iter()
        .filter_map(|binding| binding.file.clone().map(|file| (binding.uri.clone(), file)))
        .collect();
    if let Ok(uri) = settings.project_media_source_uri() {
        roots.push((uri, project_dir.clone()));
    }
    let access = move |media: &MediaRepresentation| media_access(&roots, media);
    let decoders = qnc_decoder_catalog::installed_config().map_err(|e| e.to_string())?;
    // The encoder is the ffmpeg of the installed decoder catalog (the player's toolchain).
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    let delivery = Delivery::from_export(&settings.export)?.on(&ffmpeg);
    println!("delivery: {} {}x{} {} fps, encoder {:?}", delivery.video_codec, delivery.width, delivery.height, delivery.fps, delivery.encoder);
    let cancel = AtomicBool::new(false);
    for unit in units.iter().take(limit) {
        let out = out_dir.join(delivery.file_name(unit));
        let started = Instant::now();
        qnc_program_render::render(unit, &delivery, &decoders, &access, &ffmpeg, &out, &cancel)
            .map_err(|e| format!("unit {}..{}: {e}", unit.record_in, unit.record_out))?;
        let ms = started.elapsed().as_millis().max(1);
        println!(
            "unit {}..{} ({} frames, picture {}) rendered in {ms} ms ({:.1}x real time) -> {}",
            unit.record_in,
            unit.record_out,
            unit.frames(),
            if unit.video.is_some() { "yes" } else { "black" },
            unit.frames() as f64 * 1000.0 / ms as f64 / (unit.program_rate.fps_num as f64 / unit.program_rate.fps_den as f64),
            out.display()
        );
    }
    Ok(())
}

/// The endpoint of a medium of the story: its QNC URI bound to a local root of this host
/// (a card source or the project folder), the same binding the player gets.
fn media_access(roots: &[(String, PathBuf)], media: &MediaRepresentation) -> io::Result<(CodecEndpoint, String)> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    let reference = SourceReference::from_uri(&media.media_uri).map_err(|e| invalid(e.to_string()))?;
    let (source_uri, root) = roots
        .iter()
        .find(|(uri, _)| uri == reference.source_uri())
        .ok_or_else(|| invalid(format!("no binding for {}", reference.source_uri())))?;
    let root = root.canonicalize()?;
    let path = root.join(reference.relative_path()).canonicalize()?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(invalid("media escapes its source binding".into()));
    }
    let source = LocalSource::new(source_uri, &root).map_err(|e| invalid(e.to_string()))?;
    let stamp = MediaStream::local(&source, &media.media_uri)?.info().storage_stamp.clone();
    Ok((CodecEndpoint::for_local_file(path, &media.media_uri)?, stamp))
}
