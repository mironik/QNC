//! Export of the active project's story (user 2026-10-24). `--xml`: the story as a Final Cut
//! Pro 7 XML sequence in the project's export folder (`export.directory`), the camera
//! originals as its files. The story is read as the player reads it (project database ->
//! program input); media and the project database are only read; the one file written is the
//! export, inside the export folder. Its last line is the path written, or the error.
//!
//! `--express`: the story's prerender put together without rendering again (`qnc-unit-join`),
//! in the project's delivery format; units the prerender has not made yet are rendered for it
//! into the system's temporary folder; A1 and A2 keep the user's levels within the project's
//! loudness rule (`audio.loudness`), else they are brought to it.
//!
//! `--render <id>`: the story rendered from the originals in a delivery (`original`: XDCAM
//! HD422 MXF at the story's own picture, `project`, or an export preset), put together as
//! Express is, always from the originals (user 2026-10-24: never the prerender); an
//! interlaced delivery (50i) is encoded once from lossless progressive units.
//!
//! `qnc-export (--xml | --express | --render <id>) [--root <QNC root>]`

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use qnc_content_store::ContentTarget;
use qnc_media_stream::SourceReference;
use qnc_player_input::InputReader;
use qnc_player_clip_content::PlayerClipContent;
use qnc_work_settings::SettingsReader;

type Result<T> = std::result::Result<T, String>;

fn main() {
    match run() {
        Ok(path) => println!("{}", path.display()),
        Err(error) => {
            println!("qnc-export: {error}");
            if qnc_dev_diagnostics::player_diagnostics_enabled() {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("export failed: {error}"));
            }
            std::process::exit(1);
        }
    }
}

fn run() -> Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    let (mut root, mut what) = (std::env::var_os("QNC_ROOT").map(PathBuf::from), None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--xml" | "--express" => what = Some(arg),
            "--render" => what = Some(format!("--render={}", args.next().ok_or("--render needs an export setting")?)),
            "--root" => root = Some(PathBuf::from(args.next().ok_or("--root needs a value")?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let what = what.ok_or("--xml, --express or --render <id>")?;
    // Only the root it is given (audit 2026-10-25: found beside the executable, a test that
    // started it wrote into the real active project).
    let root = root.ok_or("QNC root missing (--root or QNC_ROOT)")?;
    let reader = SettingsReader::from_root(&root).map_err(|e| e.to_string())?;
    let settings = reader.read().map_err(|e| e.to_string())?;
    let project_dir = reader.local_workspace_dir(&settings).map_err(|e| e.to_string())?.ok_or("The project has no local folder.")?;
    let folder = export_folder(&settings.export, &project_dir)?;

    let target = qnc_db_broker::ProjectDbTarget::for_project(&reader, &settings)?;
    let inputs = InputReader::with_content_reader(reader.clone(), Arc::new(PlayerClipContent { target: ContentTarget::for_project(&reader, &settings) }));
    // Every cover's A2 on its own within the project's loudness rule (user 2026-10-24).
    let loudness = settings.audio_loudness().map_err(|e| e.to_string())?;
    let rule = qnc_program_input::CoverRule { target_lufs: loudness.a2_lufs, tolerance_lu: loudness.tolerance_lu, limit_dbtp: loudness.true_peak_dbtp };
    let program = qnc_program_input::loader(target.clone(), settings.project_id.clone(), Some(rule))(&inputs, &settings.workspace_db_uri)?;

    let bindings = qnc_source_bindings::load(&root)?;
    let mut roots: Vec<(String, PathBuf)> = bindings.sources.iter().filter_map(|b| b.file.clone().map(|file| (b.uri.clone(), file))).collect();
    roots.push((settings.project_media_source_uri().map_err(|e| e.to_string())?, project_dir.clone()));

    if what == "--express" {
        let delivery = qnc_program_render::Delivery::from_export(&settings.export)?;
        // The project's runtime record says whether its prerender is at work.
        let content = ContentTarget::for_project(&reader, &settings).ok();
        return deliver(&settings, &program, delivery, "express", content.as_ref(), &target, &project_dir, &folder, roots);
    }
    if let Some(id) = what.strip_prefix("--render=") {
        let values = qnc_export_choice::delivery_values(id, &settings.export, story_format(&program)?)?;
        let delivery = qnc_program_render::Delivery::from_export(&values)?;
        // From the camera originals, never the prerender (user 2026-10-24).
        return deliver(&settings, &program, delivery, &file_name(id), None, &target, &project_dir, &folder, roots);
    }
    let mut files = BTreeMap::new();
    for (clip_id, input) in &program.clips {
        let sources: Vec<_> = program.playlist.items.iter().flat_map(|item| &item.sources).filter(|source| &source.clip_id == clip_id).collect();
        // The original: the media the sound plays from (the import's copy of it when there
        // is one), else the saved original.
        let uri = sources
            .iter()
            .find(|source| source.video_layer.is_none())
            .map(|source| source.media.media_uri.clone())
            .unwrap_or_else(|| input.snapshot.binding.original_uri.clone());
        let path = local_path(&roots, &uri)?;
        let video = original_picture(clip_id, input)?;
        files.insert(
            clip_id.clone(),
            qnc_fcp7_xml::ClipFile {
                name: path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| clip_id.clone()),
                duration_frames: sources.iter().map(|source| source.source_duration_frames).max().unwrap_or(0),
                timecode_start: sources.iter().find_map(|source| source.source_timecode_start),
                width: video.map_or(0, |format| format.width),
                height: video.map_or(0, |format| format.height),
                audio_channels: sources.iter().map(|source| source.source_audio_channels).max().unwrap_or(0),
                audio_sample_rate: sources.iter().find_map(|source| source.source_audio_format).map_or(0, |format| format.sample_rate_hz),
                path,
            },
        );
    }
    let start = settings.export_start_timecode().map_err(|e| e.to_string())?;
    let rate = program.project_audio.sample_rate_hz;
    let base = format!("{}_{}", file_name(&settings.project_name), program.playlist.revision);
    let text = qnc_fcp7_xml::sequence(&settings.project_name, &program.playlist, &files, &start, rate)?;
    let path = folder.join(format!("{base}.xml"));
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// The picture and sound of the story itself: the frame size of its first picture, its rate,
/// its sound outputs.
fn story_format(program: &qnc_player_input::ProgramInput) -> Result<qnc_export_choice::StoryFormat> {
    // The first picture of the story, its size as the original has it (the playlist's is
    // the proxy's when the project plays proxies; audit 2026-10-25).
    let first = program.playlist.items.iter().flat_map(|item| &item.sources).find(|source| source.video_layer.is_some());
    let picture = match first {
        Some(source) => original_picture(&source.clip_id, program.clips.get(&source.clip_id).ok_or("no input of the first clip")?)?,
        None => None,
    };
    let rate = program.playlist.program_timebase;
    Ok(qnc_export_choice::StoryFormat {
        width: picture.map_or(0, |format| format.width),
        height: picture.map_or(0, |format| format.height),
        fps: rate.fps_num as f64 / rate.fps_den.max(1) as f64,
        audio_channels: program.playlist.audio_layout.channel_count,
        audio_sample_rate: program.project_audio.sample_rate_hz,
    })
}

/// The picture of a clip as its original has it (size and scan from the saved record), not
/// the representation the project plays.
fn original_picture(clip_id: &str, input: &qnc_player_input::PreparedInput) -> Result<Option<Size>> {
    let original = input.clone().with_original_picture().map_err(|e| e.to_string())?;
    let media = qnc_program_input::resolved_media(clip_id, &original.snapshot.metadata.original, original.audio_media(), &original.layout)?;
    Ok(media.video_format.map(|format| Size { width: format.width, height: format.height }))
}

/// A picture size.
#[derive(Clone, Copy)]
struct Size {
    width: u32,
    height: u32,
}

/// The story's units in `delivery` put together: with `from_prerender` (Express) from the
/// project's prerender when made there in the same format, else rendered now from the
/// originals into the system's temporary folder. An interlaced delivery is rendered as
/// lossless progressive units at the story's rate and encoded once when they are put
/// together (its fields from pairs of the story's pictures).
#[allow(clippy::too_many_arguments)]
fn deliver(
    settings: &qnc_work_settings::WorkSettings,
    program: &qnc_player_input::ProgramInput,
    delivery: qnc_program_render::Delivery,
    suffix: &str,
    from_prerender: Option<&ContentTarget>,
    target: &qnc_db_broker::ProjectDbTarget,
    project_dir: &std::path::Path,
    folder: &std::path::Path,
    roots: Vec<(String, PathBuf)>,
) -> Result<PathBuf> {
    let units = qnc_broadcast_engine::ProgramPlan::for_render(program).and_then(|plan| plan.render_units()).map_err(|e| e.to_string())?;
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    let delivery = delivery.on(&ffmpeg);
    let rate = program.playlist.program_timebase;
    let story_fps = rate.fps_num as f64 / rate.fps_den.max(1) as f64;
    let picture = match delivery.interlaced {
        true => delivery.interlaced_encode(&ffmpeg, story_fps)?,
        false => Vec::new(),
    };
    let delivery = if delivery.interlaced { delivery.intermediate(story_fps) } else { delivery };
    let start_timecode = settings.export_start_timecode().map_err(|e| e.to_string())?;
    let loudness = settings.audio_loudness().map_err(|e| e.to_string())?;
    let (prerender, _) = settings.prerender_location(project_dir).map_err(|e| e.to_string())?;
    let fingerprints: Vec<String> = units.iter().map(|unit| delivery.fingerprint(unit)).collect();
    let started = std::time::Instant::now();
    let made: std::collections::HashSet<String> = match from_prerender {
        Some(content) => prerendered_units(target, content, &fingerprints)?,
        None => Default::default(),
    };
    let from_prerender_count = made.len();
    let scratch = std::env::temp_dir().join(format!("qnc-export-{}", std::process::id()));
    let result = (|| {
        let mut files = Vec::with_capacity(units.len());
        let mut decoders = None;
        for (unit, fingerprint) in units.iter().zip(&fingerprints) {
            let file = prerender.join(delivery.file_name(unit));
            if made.contains(fingerprint) && file.is_file() {
                files.push(file);
                continue;
            }
            // Not prerendered yet: rendered for this export only, as the prerender would.
            std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
            let decoders = match &decoders {
                Some(decoders) => decoders,
                None => decoders.insert(qnc_decoder_catalog::installed_config().map_err(|e| e.to_string())?),
            };
            let out = scratch.join(delivery.file_name(unit));
            let access = |media: &qnc_media_metadata::MediaRepresentation| qnc_program_render::local_access(&roots, media);
            qnc_program_render::render(unit, &delivery, decoders, &access, &ffmpeg, &out, &std::sync::atomic::AtomicBool::new(false))?;
            files.push(out);
        }
        let sound = qnc_unit_join::Sound { sample_rate: delivery.audio_sample_rate, channels: delivery.audio_channels };
        let rule = qnc_unit_join::LoudnessRule {
            a1_lufs: loudness.a1_lufs,
            a2_lufs: loudness.a2_lufs,
            true_peak_dbtp: loudness.true_peak_dbtp,
            tolerance_lu: loudness.tolerance_lu,
        };
        let out = folder.join(format!("{}_{}_{suffix}.mxf", file_name(&settings.project_name), program.playlist.revision));
        let joined = qnc_unit_join::join(&files, sound, rule, &ffmpeg, &out, &picture, &start_timecode)?;
        let lane = |name: &str, lane: &qnc_unit_join::LaneOutcome| {
            let measured = lane.measured_lufs.map_or("silence".into(), |lufs| format!("{lufs:.1} LUFS"));
            format!("{name} {measured}, {:.1} dBTP, gain {:+.1} dB", lane.true_peak_dbtp, lane.gain_db)
        };
        let line = format!(
            "{suffix}: {} units ({} from the prerender), {:.1} s, {}; {}",
            files.len(),
            from_prerender_count,
            started.elapsed().as_secs_f64(),
            lane("A1", &joined.a1),
            lane("A2", &joined.a2)
        );
        eprintln!("{line}");
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("export {line}"));
        Ok(joined.path)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// The prerendered units among `fingerprints`. While the project's prerender is at work (its
/// lease is alive) the export waits for it, up to ten minutes, instead of rendering the same
/// units a second time beside it (user 2026-10-25: the Express export took long); what is
/// still missing then is rendered by the export.
fn prerendered_units(target: &qnc_db_broker::ProjectDbTarget, content: &ContentTarget, fingerprints: &[String]) -> Result<std::collections::HashSet<String>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        let made: std::collections::HashSet<String> =
            qnc_prerender_db::PrerenderReader::open(target)?.rendered(fingerprints.to_vec())?.into_iter().map(|unit| unit.fingerprint).collect();
        let working = qnc_playback_activity::is_fresh(content, qnc_prerender_launch::LEASE, qnc_playback_activity::WORKER_FRESH_SECONDS);
        if made.len() == fingerprints.len() || !working || std::time::Instant::now() >= deadline {
            return Ok(made);
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// The project's export folder (`export.directory`: inside the project when relative).
fn export_folder(export: &serde_json::Value, project_dir: &std::path::Path) -> Result<PathBuf> {
    let directory = export.get("directory").and_then(|d| d.as_str()).map(str::trim).unwrap_or_default();
    if directory.is_empty() {
        return Err("The project has no export folder (export.directory); open the project in Project.".into());
    }
    let folder = match PathBuf::from(directory) {
        absolute if absolute.is_absolute() => absolute,
        relative => project_dir.join(relative),
    };
    // One separator of this OS throughout (the setting is written with `/`).
    let folder: PathBuf = folder.components().collect();
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    Ok(folder)
}

/// The local file of a medium of the story: its QNC URI bound to a root of this computer (a
/// card source or the project folder), as the player is given it.
fn local_path(roots: &[(String, PathBuf)], uri: &str) -> Result<PathBuf> {
    let reference = SourceReference::from_uri(uri).map_err(|e| e.to_string())?;
    let (_, root) = roots.iter().find(|(source, _)| source == reference.source_uri()).ok_or_else(|| format!("no binding for {}", reference.source_uri()))?;
    let path = root.join(reference.relative_path());
    path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))
}

/// A name a file may have on every OS.
fn file_name(name: &str) -> String {
    let name: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if name.is_empty() { "story".into() } else { name }
}
