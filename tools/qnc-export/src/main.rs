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
//! `qnc-export (--xml | --express) [--root <QNC root>]`

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
            "--root" => root = Some(PathBuf::from(args.next().ok_or("--root needs a value")?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let what = what.ok_or("--xml or --express")?;
    let root = root.or_else(qnc_dev_diagnostics::locate_qnc_root).ok_or("QNC root missing (--root, QNC_ROOT or beside the executable)")?;
    let reader = SettingsReader::from_root(&root).map_err(|e| e.to_string())?;
    let settings = reader.read().map_err(|e| e.to_string())?;
    let project_dir = reader.local_workspace_dir(&settings).map_err(|e| e.to_string())?.ok_or("The project has no local folder.")?;
    let folder = export_folder(&settings.export, &project_dir)?;

    let target = qnc_db_broker::ProjectDbTarget::for_project(&reader, &settings)?;
    let inputs = InputReader::with_content_reader(reader.clone(), Arc::new(PlayerClipContent { target: ContentTarget::for_project(&reader, &settings) }));
    let program = qnc_program_input::loader(target.clone(), settings.project_id.clone())(&inputs, &settings.workspace_db_uri)?;

    let bindings = qnc_source_bindings::load(&root)?;
    let mut roots: Vec<(String, PathBuf)> = bindings.sources.iter().filter_map(|b| b.file.clone().map(|file| (b.uri.clone(), file))).collect();
    roots.push((settings.project_media_source_uri().map_err(|e| e.to_string())?, project_dir.clone()));

    if what == "--express" {
        return express(&settings, &program, &target, &project_dir, &folder, roots);
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
        let video = sources.iter().find_map(|source| source.source_video_format);
        files.insert(
            clip_id.clone(),
            qnc_fcp7_xml::ClipFile {
                name: path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| clip_id.clone()),
                duration_frames: sources.iter().map(|source| source.source_duration_frames).max().unwrap_or(0),
                timecode_start: sources.iter().find_map(|source| source.source_timecode_start),
                width: video.map_or(0, |format| format.width),
                height: video.map_or(0, |format| format.height),
                audio_channels: sources.iter().map(|source| source.source_audio_channels).max().unwrap_or(0),
                path,
            },
        );
    }
    let text = qnc_fcp7_xml::sequence(&settings.project_name, &program.playlist, &files)?;
    let path = folder.join(format!("{}_{}.xml", file_name(&settings.project_name), program.playlist.revision));
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// The prerender of the story put together (Express): its units in order, from the
/// project's prerender when made there, else rendered now into the system's temporary folder.
fn express(
    settings: &qnc_work_settings::WorkSettings,
    program: &qnc_player_input::ProgramInput,
    target: &qnc_db_broker::ProjectDbTarget,
    project_dir: &std::path::Path,
    folder: &std::path::Path,
    roots: Vec<(String, PathBuf)>,
) -> Result<PathBuf> {
    let units = qnc_broadcast_engine::ProgramPlan::for_render(program).and_then(|plan| plan.render_units()).map_err(|e| e.to_string())?;
    let ffmpeg = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?.executable;
    let delivery = qnc_program_render::Delivery::from_export(&settings.export)?.on(&ffmpeg);
    let loudness = settings.audio_loudness().map_err(|e| e.to_string())?;
    let (prerender, _) = settings.prerender_location(project_dir).map_err(|e| e.to_string())?;
    let fingerprints: Vec<String> = units.iter().map(|unit| delivery.fingerprint(unit)).collect();
    let made: std::collections::HashSet<String> =
        qnc_prerender_db::PrerenderReader::open(target)?.rendered(fingerprints.clone())?.into_iter().map(|unit| unit.fingerprint).collect();
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
        let out = folder.join(format!("{}_{}_express.mov", file_name(&settings.project_name), program.playlist.revision));
        let joined = qnc_unit_join::join(&files, sound, rule, &ffmpeg, &out)?;
        let lane = |name: &str, lane: &qnc_unit_join::LaneOutcome| {
            let measured = lane.measured_lufs.map_or("silence".into(), |lufs| format!("{lufs:.1} LUFS"));
            format!("{name} {measured}, {:.1} dBTP, gain {:+.1} dB", lane.true_peak_dbtp, lane.gain_db)
        };
        eprintln!("express: {} units, {}; {}", files.len(), lane("A1", &joined.a1), lane("A2", &joined.a2));
        Ok(joined.path)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
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
