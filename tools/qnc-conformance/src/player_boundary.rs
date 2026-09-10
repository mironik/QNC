use qnc_contracts::ValidationReport;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};

pub fn check(root: &Path) -> ValidationReport {
    let mut report = ValidationReport::new();
    let output = match Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => {
            report.error("cannot read Cargo metadata for player boundary");
            return report;
        }
    };
    let metadata: Value = match serde_json::from_slice(&output.stdout) {
        Ok(value) => value,
        Err(error) => {
            report.error(format!("invalid Cargo metadata: {error}"));
            return report;
        }
    };
    let packages: BTreeMap<String, &Value> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["name"].as_str().map(|name| (name.to_owned(), p)))
        .collect();
    for (name, manifest) in [
        ("qnc-player-contract", "player-contract.module.json"),
        (
            "qnc-player-frame-transport",
            "player-frame-transport.module.json",
        ),
        ("qnc-broadcast-player", "broadcast-player-core.module.json"),
        ("qnc-player-input", "player-input.module.json"),
        ("qnc-player-client", "player-client.module.json"),
        ("qnc-broadcast-engine", "broadcast-engine.module.json"),
        ("qnc-player-runner", "broadcast-player.module.json"),
        ("qnc-media-stream", "media-stream.module.json"),
        ("qnc-media-decode", "media-decode.module.json"),
        ("qnc-ffmpeg-decode", "ffmpeg-decode.module.json"),
        ("qnc-decoder-catalog", "decoder-catalog.module.json"),
        ("qnc-audio-output", "audio-output.module.json"),
        ("qnc-video-output", "video-output.module.json"),
        ("qnc-pixel-convert", "pixel-convert.module.json"),
        ("qnc-gpu-raster", "gpu-raster.module.json"),
    ] {
        let Some(package) = packages.get(name) else {
            report.error(format!("missing {name}"));
            continue;
        };
        let path = root.join("contracts/modules").join(manifest);
        let value = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok());
        match value {
            Some(value) if value["module_version"] == package["version"] => (),
            _ => report.error(format!("{name}: module version does not match Cargo")),
        }
        for dependency in closure(name, &packages) {
            if forbidden_dependency(name, &dependency) {
                report.error(format!(
                    "{name}: forbidden executor/contract dependency {dependency}"
                ));
            }
        }
        let Some(manifest_path) = package["manifest_path"].as_str() else {
            continue;
        };
        let Some(parent) = Path::new(manifest_path).parent() else {
            continue;
        };
        let mut files = Vec::new();
        super::collect_rs_files(&parent.join("src"), &mut files);
        for file in files {
            if let Ok(source) = std::fs::read_to_string(&file) {
                let forbidden_source = if name == "qnc-player-frame-transport" {
                    [
                        "egui::",
                        "eframe::",
                        "rusqlite::",
                        "sqlx::",
                        "ffprobe",
                        "std::process",
                        "Command::new",
                        "std::net",
                    ]
                    .iter()
                    .any(|p| active_source(&source).contains(p))
                } else if matches!(name, "qnc-player-runner" | "qnc-player-client") {
                    [
                        "egui::",
                        "eframe::",
                        "rusqlite::",
                        "sqlx::",
                        "ffprobe",
                        "File::create",
                        "write(true)",
                    ]
                    .iter()
                    .any(|p| active_source(&source).contains(p))
                } else if name == "qnc-media-stream" {
                    forbidden_stream_source(&source)
                } else if name == "qnc-decoder-catalog" {
                    [
                        "egui::",
                        "eframe::",
                        "rusqlite::",
                        "sqlx::",
                        "ffprobe",
                        "File::create",
                        "write(true)",
                        ".spawn(",
                    ]
                    .iter()
                    .any(|p| active_source(&source).contains(p))
                } else if matches!(name, "qnc-media-decode" | "qnc-ffmpeg-decode") {
                    [
                        "egui::",
                        "eframe::",
                        "rusqlite::",
                        "sqlx::",
                        "ffprobe",
                        "File::open",
                        "File::create",
                        "std::fs",
                    ]
                    .iter()
                    .any(|p| active_source(&source).contains(p))
                } else {
                    forbidden_active_source(&source)
                };
                if forbidden_source
                    || (matches!(
                        name,
                        "qnc-media-decode" | "qnc-broadcast-engine" | "qnc-player-runner"
                    ) && [
                        "-nofind_stream_info",
                        "-stats_mux_pre",
                        "FfmpegAdapter",
                        "DecoderConfig::new(\"ffmpeg\")",
                    ]
                    .iter()
                    .any(|p| active_source(&source).contains(p)))
                    || (name == "qnc-player-input"
                        && active_source(&source).contains("Access::ReadWrite"))
                {
                    report.error(format!(
                        "{}: code violates player/input/stream module boundary",
                        file.display()
                    ));
                }
            }
        }
    }
    report
}

fn closure(name: &str, packages: &BTreeMap<String, &Value>) -> BTreeSet<String> {
    let mut visited = BTreeSet::new();
    let mut pending = vec![name.to_owned()];
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(package) = packages.get(&name) {
            for dependency in package["dependencies"].as_array().into_iter().flatten() {
                if dependency["kind"].as_str() == Some("dev") {
                    continue;
                }
                if let Some(name) = dependency["name"].as_str() {
                    pending.push(name.to_owned());
                }
            }
        }
    }
    visited
}

fn forbidden_dependency(owner: &str, dependency: &str) -> bool {
    if owner == "qnc-player-client" {
        return !matches!(
            dependency,
            "qnc-player-client" | "qnc-player-contract" | "qnc-player-frame-transport"
        ) && forbidden_dependency("qnc-player-input", dependency);
    }
    if owner == "qnc-player-frame-transport" {
        if dependency.starts_with("qnc-") {
            return !matches!(
                dependency,
                "qnc-player-frame-transport" | "qnc-player-contract" | "qnc-frame-timebase"
            );
        }
        return matches!(
            dependency,
            "egui"
                | "eframe"
                | "rfd"
                | "rusqlite"
                | "sqlx"
                | "reqwest"
                | "ureq"
                | "cpal"
                | "ffmpeg-next"
                | "ffmpeg-sys-next"
                | "winit"
                | "wgpu"
        );
    }
    // These are dependency boundaries, not a list of applications allowed to use a module.
    if owner == "qnc-player-runner" {
        return !matches!(
            dependency,
            "qnc-player-runner"
                | "qnc-player-frame-transport"
                | "winit"
                | "qnc-decoder-catalog"
                | "qnc-ffmpeg-decode"
        ) && forbidden_dependency("qnc-broadcast-engine", dependency);
    }
    if owner == "qnc-decoder-catalog" {
        return !matches!(dependency, "qnc-decoder-catalog" | "qnc-ffmpeg-decode")
            && forbidden_dependency("qnc-media-decode", dependency);
    }
    if owner == "qnc-ffmpeg-decode" {
        return dependency != owner && forbidden_dependency("qnc-media-decode", dependency);
    }
    if owner == "qnc-broadcast-engine" {
        return if dependency.starts_with("qnc-") {
            !matches!(
                dependency,
                "qnc-broadcast-engine" | "qnc-broadcast-player" | "qnc-player-contract"
            ) && forbidden_dependency("qnc-player-input", dependency)
                && forbidden_dependency("qnc-media-decode", dependency)
                && forbidden_dependency("qnc-pixel-convert", dependency)
                && forbidden_dependency("qnc-gpu-raster", dependency)
                && forbidden_dependency("qnc-video-output", dependency)
                && forbidden_dependency("qnc-audio-output", dependency)
        } else {
            matches!(dependency, "egui" | "eframe" | "rfd" | "winit")
        };
    }
    if owner == "qnc-gpu-raster" {
        return !matches!(dependency, "qnc-gpu-raster" | "wgpu")
            && forbidden_dependency("qnc-pixel-convert", dependency);
    }
    if owner == "qnc-pixel-convert" {
        if dependency.starts_with("qnc-") {
            return !matches!(
                dependency,
                "qnc-pixel-convert" | "qnc-media-metadata" | "qnc-contracts" | "qnc-frame-timebase"
            );
        }
        return matches!(
            dependency,
            "egui"
                | "eframe"
                | "rfd"
                | "rusqlite"
                | "sqlx"
                | "reqwest"
                | "ureq"
                | "wgpu"
                | "winit"
                | "cpal"
                | "ffmpeg-next"
                | "ffmpeg-sys-next"
        );
    }
    if matches!(owner, "qnc-audio-output" | "qnc-video-output") {
        return (dependency.starts_with("qnc-") && dependency != owner)
            || matches!(
                dependency,
                "egui" | "eframe" | "rfd" | "rusqlite" | "sqlx" | "reqwest" | "ureq" | "winit"
            );
    }
    if owner == "qnc-media-decode" {
        if matches!(
            dependency,
            "qnc-media-decode" | "qnc-media-metadata" | "qnc-frame-timebase"
        ) {
            return false;
        }
        return forbidden_dependency("qnc-media-stream", dependency);
    }
    if owner == "qnc-media-stream" {
        if dependency.starts_with("qnc-") {
            return !matches!(
                dependency,
                "qnc-media-stream"
                    | "qnc-source-reader"
                    | "qnc-source-contract"
                    | "qnc-contracts"
                    | "qnc-transport-resolver"
                    | "qnc-json-transport"
            );
        }
        return matches!(dependency, "egui" | "eframe" | "rfd" | "rusqlite" | "sqlx");
    }
    if owner == "qnc-player-input" {
        if dependency.starts_with("qnc-") {
            return !matches!(
                dependency,
                "qnc-player-input"
                    | "qnc-ingest-store"
                    | "qnc-work-settings"
                    | "qnc-media-records"
                    | "qnc-media-metadata"
                    | "qnc-contracts"
                    | "qnc-db-contract"
                    | "qnc-frame-timebase"
                    | "qnc-json-transport"
                    | "qnc-transport-resolver"
                    | "qnc-source-index-contract"
                    | "qnc-source-groups"
                    | "qnc-source-contract"
            );
        }
        return matches!(dependency, "egui" | "eframe" | "rfd");
    }
    if dependency.starts_with("qnc-") {
        return !matches!(dependency, "qnc-frame-timebase" | "qnc-player-contract")
            && !(owner == "qnc-broadcast-player" && dependency == owner);
    }
    matches!(
        dependency,
        "egui" | "eframe" | "rusqlite" | "sqlx" | "rfd" | "reqwest" | "ureq"
    )
}

fn active_source(source: &str) -> &str {
    if source.trim_start().starts_with("#![cfg(test)]") {
        return "";
    }
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

fn forbidden_active_source(source: &str) -> bool {
    let active = active_source(source);
    [
        "std::fs",
        "std::process",
        "std::net",
        "std::path",
        "Command::new",
        "egui::",
        "eframe::",
        "rusqlite::",
    ]
    .iter()
    .any(|pattern| active.contains(pattern))
}

fn forbidden_stream_source(source: &str) -> bool {
    let active = active_source(source);
    [
        "std::process",
        "process::Command",
        "Command::new",
        "egui::",
        "eframe::",
        "rusqlite::",
        "sqlx::",
        "File::create",
        "write_all(",
        "write(true)",
        "remove_file(",
    ]
    .iter()
    .any(|pattern| active.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_cannot_select_or_embed_a_decoder_technology() {
        for owner in [
            "qnc-broadcast-player",
            "qnc-broadcast-engine",
            "qnc-media-decode",
        ] {
            assert!(forbidden_dependency(owner, "qnc-ffmpeg-decode"));
            assert!(forbidden_dependency(owner, "qnc-decoder-catalog"));
        }
        assert!(!forbidden_dependency(
            "qnc-player-runner",
            "qnc-decoder-catalog"
        ));
        for owner in ["qnc-ffmpeg-decode", "qnc-decoder-catalog"] {
            assert!(forbidden_dependency(owner, "qnc-project-store"));
            assert!(forbidden_dependency(owner, "qnc-media-probe"));
            assert!(forbidden_dependency(owner, "qnc-ingest-components"));
        }
    }

    #[test]
    fn player_client_cannot_embed_executor_or_application_workflow() {
        for name in [
            "qnc-broadcast-player",
            "qnc-broadcast-engine",
            "qnc-media-decode",
            "qnc-ingest-components",
            "qnc-project-store",
            "qnc-media-probe",
            "qnc-scanner",
            "qnc-ui-kit",
            "eframe",
        ] {
            assert!(forbidden_dependency("qnc-player-client", name), "{name}");
        }
        for name in [
            "qnc-player-client",
            "qnc-player-contract",
            "qnc-player-frame-transport",
            "qnc-player-input",
            "qnc-json-transport",
            "qnc-transport-resolver",
        ] {
            assert!(!forbidden_dependency("qnc-player-client", name), "{name}");
        }
    }

    #[test]
    fn pixel_converter_only_depends_on_saved_data_and_color_math() {
        for dependency in [
            "qnc-media-decode",
            "qnc-player-input",
            "qnc-video-output",
            "qnc-ingest-store",
            "qnc-broadcast-player",
            "wgpu",
            "cpal",
            "reqwest",
            "eframe",
        ] {
            assert!(forbidden_dependency("qnc-pixel-convert", dependency));
        }
        for dependency in [
            "yuv",
            "serde",
            "qnc-media-metadata",
            "qnc-frame-timebase",
            "qnc-contracts",
            "qnc-pixel-convert",
        ] {
            assert!(!forbidden_dependency("qnc-pixel-convert", dependency));
        }
    }

    #[test]
    fn video_output_is_not_a_player_ui_decoder_or_network_client() {
        for dependency in [
            "qnc-broadcast-player",
            "qnc-media-decode",
            "qnc-player-input",
            "qnc-ui-kit",
            "qnc-project-store",
            "qnc-ingest-components",
            "reqwest",
            "winit",
        ] {
            assert!(forbidden_dependency("qnc-video-output", dependency));
        }
        for dependency in ["wgpu", "serde", "qnc-video-output"] {
            assert!(!forbidden_dependency("qnc-video-output", dependency));
        }
    }

    #[test]
    fn gpu_raster_is_separate_from_pixel_contract_and_has_no_workflow() {
        assert!(forbidden_dependency("qnc-pixel-convert", "qnc-gpu-raster"));
        for dependency in [
            "qnc-player-input",
            "qnc-broadcast-player",
            "qnc-media-decode",
            "qnc-ingest-components",
            "eframe",
            "rusqlite",
            "reqwest",
            "cpal",
        ] {
            assert!(forbidden_dependency("qnc-gpu-raster", dependency));
        }
        for dependency in ["wgpu", "qnc-pixel-convert", "qnc-media-metadata"] {
            assert!(!forbidden_dependency("qnc-gpu-raster", dependency));
        }
    }

    #[test]
    fn audio_output_is_a_device_edge_not_a_decoder_or_application() {
        for dependency in [
            "qnc-media-decode",
            "qnc-player-input",
            "qnc-broadcast-player",
            "qnc-media-probe",
            "qnc-ui-kit",
            "qnc-project-store",
            "qnc-ingest-components",
        ] {
            assert!(forbidden_dependency("qnc-audio-output", dependency));
        }
        for dependency in ["cpal", "rtrb", "serde", "qnc-audio-output"] {
            assert!(!forbidden_dependency("qnc-audio-output", dependency));
        }
    }

    #[test]
    fn media_stream_cannot_acquire_app_db_or_playback_responsibilities() {
        for dependency in [
            "qnc-project-store",
            "qnc-ingest-components",
            "qnc-player-input",
            "qnc-broadcast-player",
            "qnc-media-probe",
            "qnc-scanner",
            "qnc-ui-kit",
            "rusqlite",
        ] {
            assert!(forbidden_dependency("qnc-media-stream", dependency));
        }
        assert!(!forbidden_dependency(
            "qnc-media-stream",
            "qnc-source-reader"
        ));
        assert!(!forbidden_stream_source("use std::fs::File;"));
        assert!(forbidden_stream_source("Command::new(binary)"));
        assert!(forbidden_stream_source("options.write(true)"));
    }

    #[test]
    fn decoder_can_use_bytes_and_saved_facts_but_not_workflow_clock_or_probe() {
        for dependency in [
            "qnc-player-input",
            "qnc-broadcast-player",
            "qnc-media-probe",
            "qnc-ingest-store",
            "qnc-project-store",
            "qnc-work-settings",
            "qnc-ui-kit",
        ] {
            assert!(forbidden_dependency("qnc-media-decode", dependency));
        }
        for dependency in [
            "qnc-media-stream",
            "qnc-media-metadata",
            "qnc-frame-timebase",
        ] {
            assert!(!forbidden_dependency("qnc-media-decode", dependency));
        }
    }

    #[test]
    fn data_contract_cannot_depend_on_executor_or_applications() {
        for name in [
            "qnc-broadcast-player",
            "qnc-media-probe",
            "qnc-filmstrip",
            "qnc-ingest-store",
            "qnc-project-store",
            "qnc-ui-kit",
            "rusqlite",
            "eframe",
        ] {
            assert!(forbidden_dependency("qnc-player-contract", name), "{name}");
        }
        assert!(!forbidden_dependency(
            "qnc-player-contract",
            "qnc-frame-timebase"
        ));
        assert!(!forbidden_dependency(
            "qnc-broadcast-player",
            "qnc-player-contract"
        ));
    }

    #[test]
    fn frame_transport_is_a_local_picture_handoff_not_a_decoder_or_app() {
        for dependency in [
            "qnc-broadcast-engine",
            "qnc-media-decode",
            "qnc-player-input",
            "qnc-ingest-components",
            "qnc-project-store",
            "qnc-media-probe",
            "qnc-scanner",
            "qnc-ui-kit",
            "eframe",
            "rusqlite",
            "winit",
            "wgpu",
        ] {
            assert!(
                forbidden_dependency("qnc-player-frame-transport", dependency),
                "{dependency}"
            );
        }
        for dependency in [
            "qnc-player-frame-transport",
            "qnc-player-contract",
            "qnc-frame-timebase",
            "memmap2",
            "serde",
            "serde_json",
        ] {
            assert!(
                !forbidden_dependency("qnc-player-frame-transport", dependency),
                "{dependency}"
            );
        }
    }

    #[test]
    fn native_process_does_not_pull_application_workflow_or_probe() {
        for name in [
            "qnc-project-store",
            "qnc-ingest-components",
            "qnc-app",
            "qnc-media-probe",
            "qnc-scanner",
            "qnc-ui-kit",
            "egui",
            "eframe",
        ] {
            assert!(forbidden_dependency("qnc-player-runner", name), "{name}");
        }
        for name in [
            "qnc-broadcast-engine",
            "qnc-player-contract",
            "qnc-player-frame-transport",
            "qnc-media-stream",
            "qnc-json-transport",
            "winit",
        ] {
            assert!(!forbidden_dependency("qnc-player-runner", name), "{name}");
        }
    }

    #[test]
    fn source_guard_separates_tests_from_active_io() {
        assert!(forbidden_active_source("use std::process::Command;"));
        assert!(!forbidden_active_source(
            "pub struct Input;\n#[cfg(test)]\nmod tests { use std::fs; }"
        ));
    }

    #[test]
    fn input_may_read_public_db_but_cannot_acquire_media_or_drive_playback() {
        for dependency in [
            "qnc-media-probe",
            "qnc-scanner",
            "qnc-project-store",
            "qnc-ingest-components",
            "qnc-broadcast-player",
            "qnc-source-reader",
            "qnc-ui-kit",
        ] {
            assert!(forbidden_dependency("qnc-player-input", dependency));
        }
        for dependency in [
            "qnc-work-settings",
            "qnc-ingest-store",
            "qnc-media-metadata",
        ] {
            assert!(!forbidden_dependency("qnc-player-input", dependency));
        }
    }

    #[test]
    fn cargo_graph_uses_package_names_and_transitive_dependencies() {
        let first = serde_json::json!({"dependencies":[{"name":"adapter", "rename":"safe_name", "kind":null}]});
        let adapter = serde_json::json!({"dependencies":[{"name":"qnc-media-probe", "kind":null}]});
        let packages = BTreeMap::from([("core".into(), &first), ("adapter".into(), &adapter)]);
        assert!(closure("core", &packages).contains("qnc-media-probe"));
    }
}
