//! Shows the editorial form base with an empty view.
//!
//!     cargo run -p qnc-editorial-desktop --example editorial_form -- e
//!
//! Groups: e, g, l, o. Only the layout is shown; no application, database or
//! player is attached.

use eframe::egui;
use qnc_editorial_desktop::{EditorialForm, EditorialView};
use qnc_timeline::TimelineProjection;

struct Host {
    form: EditorialForm,
    view: EditorialView,
    last_intent: String,
}

impl eframe::App for Host {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.form.apply_theme(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if let Some(intent) = self.form.show_desktop(ui, &self.view) {
                    self.last_intent = format!("{intent:?}");
                    if let qnc_editorial_desktop::EditorialIntent::Timeline(
                        qnc_timeline::TimelineIntent::CueFrame(frame),
                    ) = intent
                    {
                        self.view.timeline = self.view.timeline.with_playhead(frame);
                    }
                    eprintln!("intent: {}", self.last_intent);
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
    view.timeline = TimelineProjection::new(0, 5000)
        .with_playhead(1200)
        .with_cue_enabled(true);
    let host = Host {
        form,
        view,
        last_intent: String::new(),
    };
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
