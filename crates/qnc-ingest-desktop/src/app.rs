use std::path::PathBuf;

use eframe::egui::{self, CentralPanel, Frame};
use qnc_ingest_components::{action_ids, IngestComponent, IngestIntent};
use qnc_keyboard_shortcut::ShortcutEvent;

use crate::{
    layout_contract::IngestContracts,
    theme::{self, Theme},
    widgets,
};

pub struct IngestApp {
    root: PathBuf,
    contracts: IngestContracts,
    component: IngestComponent,
}

impl IngestApp {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let component = IngestComponent::with_store_root(&root)?;
        Ok(Self {
            root,
            contracts: IngestContracts::load()?,
            component,
        })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.dispatch_keyboard_shortcuts(ctx);
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        ui.data_mut(|data| data.insert_temp(egui::Id::new("qnc_ingest_root"), self.root.clone()));
        let view = self.component.view().clone();
        if let Some(intent) = widgets::render_desktop(ui, &self.contracts, &theme, &view) {
            self.dispatch(ctx, intent);
        }
    }

    fn dispatch(&mut self, ctx: &egui::Context, intent: IngestIntent) {
        let result = self.component.dispatch(intent);
        if result.request_repaint {
            ctx.request_repaint();
        }
    }

    fn dispatch_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        for event in Self::shortcut_events(ctx) {
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

    fn shortcut_events(ctx: &egui::Context) -> Vec<ShortcutEvent> {
        let text_input_reserved = ctx.memory(|memory| memory.focused().is_some());
        ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        physical_key,
                        pressed,
                        repeat,
                        modifiers,
                        ..
                    } if *pressed && !*repeat => Some(ShortcutEvent {
                        code: physical_key.as_ref().and_then(Self::catalog_key_code),
                        key: Self::catalog_key_name(key),
                        shift: modifiers.shift,
                        ctrl: modifiers.ctrl || modifiers.command,
                        alt: modifiers.alt,
                        text_input_reserved,
                    }),
                    _ => None,
                })
                .collect()
        })
    }

    fn catalog_key_name(key: &egui::Key) -> Option<String> {
        use egui::Key;

        let name = match key {
            Key::ArrowLeft => "ArrowLeft",
            Key::ArrowRight => "ArrowRight",
            Key::Space => " ",
            _ => return None,
        };
        Some(name.to_string())
    }

    fn catalog_key_code(key: &egui::Key) -> Option<String> {
        use egui::Key;

        let code = match key {
            Key::ArrowLeft => "ArrowLeft",
            Key::ArrowRight => "ArrowRight",
            Key::Space => "Space",
            _ => return None,
        };
        Some(code.to_string())
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
