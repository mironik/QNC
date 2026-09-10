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
        let repaint = ctx.clone();
        self.component
            .notify_on_player_change(move || repaint.request_repaint());
        if self.component.poll() {
            ctx.request_repaint();
        }
        if let Some(delay) = self.component.next_repaint_delay() {
            ctx.request_repaint_after(delay);
        }
        self.dispatch_keyboard_shortcuts(ctx);
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        ui.data_mut(|data| data.insert_temp(egui::Id::new("qnc_ingest_root"), self.root.clone()));
        let view = self.component.view().clone();
        if let Some(intent) = widgets::render_desktop(ui, &self.contracts, &theme, &view) {
            self.dispatch(ctx, intent);
        }
    }

    pub fn footer_status(&self) -> &str {
        self.component.footer_status()
    }

    pub fn on_activated(&mut self) {
        self.component.refresh_active_project();
    }

    fn dispatch(&mut self, ctx: &egui::Context, intent: IngestIntent) {
        let result = self.component.dispatch(intent);
        if result.request_repaint {
            ctx.request_repaint();
        }
    }

    fn dispatch_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        self.dispatch_consumed_playback_space(ctx);
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

    fn dispatch_consumed_playback_space(&mut self, ctx: &egui::Context) {
        let event = ShortcutEvent {
            code: Some("Space".to_string()),
            key: Some(" ".to_string()),
            shift: false,
            ctrl: false,
            alt: false,
            text_input_reserved: false,
        };
        let actions = self
            .contracts
            .shortcuts
            .action_ids_for_event("ingest", &event)
            .into_iter()
            .filter(|action_id| *action_id == action_ids::PLAY_PAUSE)
            .map(str::to_string)
            .collect::<Vec<_>>();
        if actions.is_empty() {
            return;
        }
        let presses = ctx.input_mut(|input| {
            let mut presses = 0usize;
            input.events.retain(|event| {
                let consume = matches!(
                    event,
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } if modifiers.is_none()
                        && Self::catalog_key_code(key).as_deref() == Some("Space")
                );
                if consume {
                    presses += 1;
                }
                !consume
            });
            presses
        });
        for _ in 0..presses {
            for action_id in &actions {
                self.dispatch(ctx, IngestIntent::empty(action_id.clone()));
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
