use std::path::PathBuf;

use eframe::egui::{self, CentralPanel, Frame};
use qnc_ingest_application::{action_ids, IngestApplication, IngestIntent};

use crate::{
    layout_contract::IngestContracts,
    theme::{self, Theme},
    widgets,
};

pub struct IngestApp {
    root: PathBuf,
    contracts: IngestContracts,
    application: IngestApplication,
    player_repaint_bound: bool,
}

impl IngestApp {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let application = IngestApplication::with_store_root(&root)?;
        Ok(Self {
            root,
            contracts: IngestContracts::load()?,
            application,
            player_repaint_bound: false,
        })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if !self.player_repaint_bound
            && (self.application.view().playback.preparing
                || self.application.view().playback.reply.is_some())
        {
            let repaint = ctx.clone();
            self.application.notify_on_player_change(move || {
                repaint.request_repaint();
            });
            self.player_repaint_bound = true;
        }
        let changed = self.application.poll();
        if changed {
            ctx.request_repaint();
        } else if let Some(delay) = self.application.next_repaint_delay() {
            // Cadence only when this paint had no new picture. Scheduling it
            // after a notify wake postpones the next source frame.
            ctx.request_repaint_after(delay);
        }
        self.dispatch_keyboard_shortcuts(ctx);
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        ui.data_mut(|data| data.insert_temp(egui::Id::new("qnc_ingest_root"), self.root.clone()));
        let view = self.application.view().clone();
        if let Some(intent) = widgets::render_desktop(ui, &self.contracts, &theme, &view) {
            self.dispatch(ctx, intent);
        }
    }

    pub fn footer_status(&self) -> &str {
        self.application.footer_status()
    }

    pub fn on_activated(&mut self) {
        self.application.refresh_active_project();
    }

    /// Uvezi has started the import: the shell may open the next application.
    pub fn take_navigation_request(&mut self) -> bool {
        self.application.take_navigation_request()
    }

    pub fn navigation_sequence(&self) -> Result<Vec<qnc_ingest_application::SequenceStep>, String> {
        self.application.navigation_sequence()
    }

    fn dispatch(&mut self, ctx: &egui::Context, intent: IngestIntent) {
        let result = self.application.dispatch(intent);
        if result.request_repaint {
            ctx.request_repaint();
        }
    }

    fn dispatch_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let play_presses = qnc_keyboard_shortcut::consume_egui_action_presses(
            ctx,
            &self.contracts.shortcuts,
            "ingest",
            action_ids::PLAY_PAUSE,
        );
        for _ in 0..play_presses {
            self.dispatch(ctx, IngestIntent::empty(action_ids::PLAY_PAUSE));
        }
        for event in qnc_keyboard_shortcut::egui_shortcut_events(ctx) {
            let actions = self
                .contracts
                .shortcuts
                .action_ids_for_event("ingest", &event)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            for action_id in actions {
                match action_id.as_str() {
                    action_ids::PLAY_PAUSE
                    | action_ids::STEP_BACK_FRAME
                    | action_ids::STEP_FORWARD_FRAME => {
                        self.dispatch(ctx, IngestIntent::empty(action_id));
                    }
                    _ => {}
                }
            }
        }
    }
}

impl eframe::App for IngestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        CentralPanel::default()
            .frame(Frame::NONE.fill(theme.bg))
            .show(ctx, |ui| self.show_desktop(ctx, ui));
    }
}
