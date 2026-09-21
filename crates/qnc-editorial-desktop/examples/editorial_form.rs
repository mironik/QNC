//! Shows the editorial form with fake clips and no application behind it.
//!
//!     cargo run -p qnc-editorial-desktop --example editorial_form -- e
//!
//! Groups: e, g, l, o. Layout only: clicking a clip marks it, the monitor and
//! the timeline stay empty. The real thing (Broadcast Player, project catalog)
//! is `EditorialApp`, used by the four applications.

use eframe::egui;
use qnc_editorial_desktop::{EditorialClip, EditorialForm, EditorialIntent, EditorialView};
use qnc_timeline::TimelineProjection;

struct Host {
    form: EditorialForm,
    view: EditorialView,
}

impl eframe::App for Host {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.form.apply_theme(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if let Some(intent) = self.form.show_desktop(ui, &self.view) {
                    eprintln!("intent: {intent:?}");
                    match intent {
                        EditorialIntent::PreviewClip(id) => self.view.preview.clip_id = Some(id),
                        EditorialIntent::Timeline(qnc_timeline::TimelineIntent::CueFrame(
                            frame,
                        )) => {
                            self.view.preview.timeline =
                                self.view.preview.timeline.with_playhead(frame);
                        }
                        _ => {}
                    }
                }
            });
    }
}

fn main() -> eframe::Result<()> {
    let group = std::env::args().nth(1).unwrap_or_else(|| "e".to_string());
    let form = EditorialForm::new(&group).unwrap_or_else(|error| {
        eprintln!("editorial form error: {error}");
        std::process::exit(1);
    });
    let mut view = EditorialView::default();
    view.clips = (1..=40)
        .map(|i| EditorialClip {
            clip_id: format!("clip-{i:03}"),
            name: format!("Izjava {i:03}.MXF"),
            duration_seconds: 20.0 + (i as f64 * 7.3) % 190.0,
            imported: i % 3 != 0,
            thumb_uri: None,
            thumb_image: None,
            import_status: String::new(),
            imported_media_uri: String::new(),
        })
        .collect();
    view.preview.timeline = TimelineProjection::new(0, 5000)
        .with_playhead(1200)
        .with_cue_enabled(true);
    let host = Host { form, view };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0])
            .with_title(format!("QNC editorial form (grupa {group})")),
        ..Default::default()
    };
    eframe::run_native(
        "QNC editorial form",
        options,
        Box::new(move |_cc| Ok(Box::new(host))),
    )
}
