//! Prerender of the active project's story (user 2026-10-12): the story is read as the
//! player reads it (project database -> program input -> the player's program plan), cut
//! into units between cuts, and each unit not yet rendered is rendered from the camera
//! originals into the project's delivery format (`export.*`) under the project's
//! `products.prerender`, its file named by its fingerprint (the same unit wherever it
//! stands in the story); a complete unit is written into the project database
//! (`qnc-prerender-db`), which alone says what is rendered. One prerender runs per project
//! (its lease in the database); it waits while a player plays, leaves a unit unfinished
//! when a player starts, and ends when the active project changes or after a minute with
//! nothing to render.
//! Media is only read (a card stays read only); the project database is only read.
//!
//! `qnc-prerender [--root <QNC root>] [--units <n> (at most n, then end)] [--list 1]`

use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use qnc_broadcast_engine::ProgramPlan;
use qnc_content_store::{Access, ContentTarget};
use qnc_media_metadata::MediaRepresentation;
use qnc_media_stream::{CodecEndpoint, LocalSource, MediaStream, SourceReference};
use qnc_player_input::{InputReader, PlayerClipRecord, PlayerContentRead};
use qnc_prerender_db::{PrerenderReader, PrerenderWriter, RenderedUnit};
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
        // Woken in the background its output goes nowhere: the stop is in the diagnostics.
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("prerender stopped: {error}"));
        }
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
    let root = root.or_else(qnc_dev_diagnostics::locate_qnc_root).ok_or("QNC root missing (--root, QNC_ROOT or beside the executable)")?;
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
    let inputs = InputReader::with_content_reader(
        reader.clone(),
        Arc::new(PlayerContent { target: ContentTarget::for_project(&reader, &settings) }),
    );
    let units = story_units(&target, &settings, &inputs)?;
    say(format!("story: {} units, read in {} ms", units.len(), started.elapsed().as_millis()));

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
    let project_source = settings.project_media_source_uri().map_err(|e| e.to_string())?;
    roots.push((project_source.clone(), project_dir.clone()));
    let access = move |media: &MediaRepresentation| media_access(&roots, media);
    let decoders = qnc_decoder_catalog::installed_config().map_err(|e| e.to_string())?;
    // The encoder is the ffmpeg of the installed decoder catalog (the player's toolchain).
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    let delivery = Delivery::from_export(&settings.export)?.on(&ffmpeg);
    let relative = settings.products.prerender.clone().ok_or("Projekt nema products.prerender.")?;
    say(format!("delivery {} {}x{} {} fps, encoder {:?}", delivery.video_codec, delivery.width, delivery.height, delivery.fps, delivery.encoder));

    // The one prerender of the project: its lease in the project database; the player first.
    let content = ContentTarget::for_project(&reader, &settings)?;
    let Some(lease) = qnc_playback_activity::Lease::claim(content.clone(), LEASE, qnc_playback_activity::WORKER_FRESH_SECONDS)? else {
        say("another prerender of this project runs".into());
        return Ok(());
    };
    // Asked of the database each time (no cache): a player that starts is seen within
    // 100 ms (with a cached answer the prerender read the card 2.7 s into a Play and the
    // sound ran dry, 2026-10-12).
    let playing = || qnc_playback_activity::is_fresh(&content, qnc_playback_activity::PLAYBACK, qnc_playback_activity::PLAYBACK_FRESH_SECONDS);
    let mut quiet_since: Option<Instant> = None;
    let writer = PrerenderWriter::start(target.clone())?;
    let (mut idle_since, mut done_now, mut failed) = (Instant::now(), 0usize, std::collections::HashSet::new());
    let mut pruned: Option<Vec<String>> = None;
    loop {
        if !lease.held() {
            say("another prerender took the lease: prerender ends".into());
            return Ok(());
        }
        if reader.read().map(|now| now.project_id).ok() != Some(settings.project_id.clone()) {
            say("the active project changed: prerender ends".into());
            return Ok(());
        }
        // The player first; back to work only once it has been quiet a while (a story is
        // prepared and then played: no work between the two).
        if playing() {
            quiet_since = None;
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        if quiet_since.get_or_insert_with(Instant::now).elapsed() < SETTLE {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        let units = story_units(&target, &settings, &inputs)?;
        let fingerprints: Vec<String> = units.iter().map(|unit| delivery.fingerprint(unit)).collect();
        let rendered: std::collections::HashSet<String> = PrerenderReader::open(&target)?
            .rendered(fingerprints.clone())?
            .into_iter()
            .map(|unit| unit.fingerprint)
            .collect();
        let next = units
            .iter()
            .zip(&fingerprints)
            .find(|(_, fingerprint)| !rendered.contains(*fingerprint) && !failed.contains(*fingerprint));
        let Some((unit, fingerprint)) = next else {
            // Every unit of the story is there: the units it no longer has go (once per story).
            if pruned.as_ref() != Some(&fingerprints) {
                if lease.confirm() {
                    prune(&writer, &mut PrerenderReader::open(&target)?, &out_dir, &fingerprints);
                }
                pruned = Some(fingerprints.clone());
            }
            if idle_since.elapsed() > IDLE_END || done_now >= limit {
                say(format!("story prerendered ({} units): prerender ends", units.len()));
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        if done_now >= limit {
            return Ok(());
        }
        idle_since = Instant::now();
        let name = delivery.file_name(unit);
        let out = out_dir.join(&name);
        let started = Instant::now();
        let cancel = AtomicBool::new(false);
        let rendered = std::thread::scope(|scope| {
            // A player that starts takes the machine back: the unit is left unfinished.
            let watch = scope.spawn(|| {
                while !cancel.load(Ordering::Relaxed) {
                    if playing() {
                        cancel.store(true, Ordering::Relaxed);
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                false
            });
            let result = qnc_program_render::render(unit, &delivery, &decoders, &access, &ffmpeg, &out, &cancel);
            cancel.store(true, Ordering::Relaxed);
            (result, watch.join().unwrap_or(false))
        });
        match rendered {
            (Ok(()), _) => {
                let file_uri = SourceReference::new(&project_source, &format!("{}/{name}", relative.trim_matches('/')))
                    .map_err(|e| e.to_string())?
                    .uri();
                let media = qnc_program_render::unit_media(unit, &delivery, &file_uri)?;
                // Published only while this process still holds the lease (external audit
                // 2026-10-17): another prerender may have taken it meanwhile.
                if !lease.confirm() {
                    say("the lease is another's: the unit is not published".into());
                    return Ok(());
                }
                writer.publish(RenderedUnit {
                    fingerprint: fingerprint.clone(),
                    file_uri,
                    frames: unit.frames(),
                    samples: unit.samples,
                    encoder: delivery.encoder.clone().unwrap_or_default(),
                    key: delivery.key(),
                    media: serde_json::to_value(media).map_err(|e| e.to_string())?,
                })?;
                done_now += 1;
                let ms = started.elapsed().as_millis().max(1);
                let rate = unit.program_rate.fps_num as f64 / unit.program_rate.fps_den.max(1) as f64;
                say(format!(
                    "unit {}..{} rendered in {ms} ms ({:.1}x real time) {fingerprint}",
                    unit.record_in,
                    unit.record_out,
                    unit.frames() as f64 * 1000.0 / ms as f64 / rate
                ));
            }
            (Err(_), true) => {
                quiet_since = None;
                say(format!("unit {}..{} left for the player", unit.record_in, unit.record_out));
            }
            (Err(error), false) => {
                say(format!("unit {}..{} failed: {error}", unit.record_in, unit.record_out));
                failed.insert(fingerprint.clone());
            }
        }
    }
}

/// The units the story no longer has go (user 2026-10-13: "brisanje starih dijelova"):
/// their rows first, so no player takes them again, then their files and the leftovers
/// of unfinished renders in the prerender folder. A project folder is protected against
/// deleting; a file it keeps is left as it is (never emptied). Only files named by a
/// fingerprint are touched. UNDO needs no file: the story's edits are kept in its
/// database and a unit is rendered again from them.
fn prune(writer: &PrerenderWriter, reader: &mut PrerenderReader, out_dir: &std::path::Path, keep: &[String]) {
    let keep: std::collections::HashSet<&str> = keep.iter().map(String::as_str).collect();
    let old: Vec<String> = match reader.all() {
        Ok(units) => units.into_iter().map(|unit| unit.fingerprint).filter(|fp| !keep.contains(fp.as_str())).collect(),
        Err(error) => return say(format!("old units not read: {error}")),
    };
    if let Err(error) = writer.forget(old.clone()) {
        return say(format!("old units not forgotten: {error}"));
    }
    let Ok(entries) = std::fs::read_dir(out_dir) else { return };
    let (mut files, mut bytes, mut kept) = (0usize, 0u64, 0usize);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let fingerprint = name.split('.').next().unwrap_or("");
        let named = fingerprint.len() == 16 && fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit());
        let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        if !named || keep.contains(fingerprint) || size == 0 || !entry.path().is_file() {
            continue;
        }
        // Deleted with its name (user 2026-10-13: never only emptied); a file the folder's
        // protection keeps stays as it is and is told.
        match std::fs::remove_file(entry.path()) {
            Ok(()) => {
                files += 1;
                bytes += size;
            }
            Err(_) => kept += 1,
        }
    }
    say(format!(
        "old units: {} forgotten, {files} files deleted ({} MB), {kept} kept by the folder's protection",
        old.len(),
        bytes / 1_048_576
    ));
}

/// The lease of the one prerender of a project in its database (the waker reads it).
use qnc_prerender_launch::LEASE;
/// After a player stops, the prerender waits this long before it works again.
const SETTLE: Duration = Duration::from_secs(2);
/// With nothing to render this long the prerender ends (the story wakes it again).
const IDLE_END: Duration = Duration::from_secs(60);

/// The story of the active project as prerender units (read as the player reads it).
fn story_units(
    target: &qnc_db_broker::ProjectDbTarget,
    settings: &qnc_work_settings::WorkSettings,
    inputs: &InputReader,
) -> Result<Vec<qnc_render_unit::RenderUnit>> {
    let program = qnc_program_input::loader(target.clone(), settings.project_id.clone())(inputs, &settings.workspace_db_uri)?;
    ProgramPlan::for_render(&program)
        .and_then(|plan| plan.render_units())
        .map_err(|e| e.to_string())
}

/// A line of what the prerender does: on its output and in the player diagnostics.
fn say(line: String) {
    println!("{line}");
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("prerender {line}"));
    }
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
