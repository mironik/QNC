//! QNC desktop host: the desktop block and the public surfaces of the registered
//! applications (`desktop_entry` -> public adapter).

use std::{collections::HashMap, env, process};

use eframe::egui;
use qnc_shell_desktop_api::EmbeddedAppFactory;

fn main() -> eframe::Result<()> {
    let layout = qnc_desktop::ShellLayoutContract::load_embedded().unwrap_or_else(|error| {
        eprintln!("qnc-app shell contract error: {error}");
        process::exit(1);
    });

    let qnc_root = qnc_desktop::resolve_qnc_root().unwrap_or_else(|error| {
        eprintln!("qnc-app root error: {error}");
        process::exit(1);
    });

    let app_registry = qnc_app_registry::AppRegistry::load(&qnc_root).unwrap_or_else(|error| {
        eprintln!("qnc-app registry error: {error}");
        process::exit(1);
    });

    if env::args().any(|arg| arg == "--check-contracts") {
        println!(
            "qnc-app contracts OK: shell={} layout_tabs={} registered_apps={}",
            layout.layout_id,
            layout.application_tabs.join(","),
            app_registry.summary()
        );
        return Ok(());
    }

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([1100.0, 700.0])
            .with_maximized(true)
            .with_title("QNC"),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "QNC",
        options,
        Box::new(move |cc| {
            qnc_desktop::apply_app_fonts(&cc.egui_ctx, &layout);
            qnc_desktop::apply_visuals(&cc.egui_ctx, &layout);
            let shell = qnc_desktop::QncShell::new(layout, qnc_root, app_registry, embedded_app_factories());
            Ok(Box::new(shell))
        }),
    )
}

fn embedded_app_factories() -> HashMap<String, EmbeddedAppFactory> {
    let project = qnc_project_desktop_adapter::factory();
    let ingest = qnc_ingest_desktop_adapter::factory();
    let media_assist_audio_ai = qnc_media_assist_audio_ai_desktop_adapter::factory();
    let media_assist_audio = qnc_media_assist_audio_desktop_adapter::factory();
    let media_assist_video = qnc_media_assist_video_desktop_adapter::factory();
    let story = qnc_story_desktop_adapter::factory();
    HashMap::from([
        (project.desktop_entry.to_string(), project),
        (ingest.desktop_entry.to_string(), ingest),
        (
            media_assist_audio_ai.desktop_entry.to_string(),
            media_assist_audio_ai,
        ),
        (
            media_assist_audio.desktop_entry.to_string(),
            media_assist_audio,
        ),
        (
            media_assist_video.desktop_entry.to_string(),
            media_assist_video,
        ),
        (story.desktop_entry.to_string(), story),
    ])
}
