//! The keyboard overview (catalog `toggle_cheatsheet`, F1 / Shift+/): every key of the
//! active preset for the scopes of a board, with its catalog label, so nobody has to
//! remember them (user rule 2026-10-01). Read from the external catalog only; it changes
//! nothing and knows no application. Whether it is open is window state of this frame's
//! context, not a record.

use eframe::egui;
use qnc_keyboard_shortcut::ShortcutCatalog;

/// The catalog action that opens and closes the overview.
pub const TOGGLE_ACTION: &str = "toggle_cheatsheet";

/// One line of the overview: the keys and what they do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub keys: String,
    pub label: String,
}

/// The lines of one scope of the active preset, by label.
pub fn lines(catalog: &ShortcutCatalog, scope: &str) -> Vec<Line> {
    let Some(bindings) = catalog
        .presets
        .get(&catalog.active_preset)
        .and_then(|preset| preset.scopes.get(scope))
    else {
        return Vec::new();
    };
    let mut lines: Vec<Line> = bindings
        .keys()
        .filter_map(|action_id| {
            let keys = catalog.chord_hint(scope, action_id)?;
            let label = catalog
                .actions
                .get(action_id)
                .map_or_else(|| action_id.clone(), |action| action.label.clone());
            Some(Line { keys, label })
        })
        .collect();
    lines.sort_by(|left, right| left.label.cmp(&right.label));
    lines
}

/// Opens or closes the overview when `toggled`, and draws it while it is open.
pub fn show(ctx: &egui::Context, catalog: &ShortcutCatalog, scopes: &[&str], toggled: bool) {
    let id = egui::Id::new("qnc_key_cheatsheet_open");
    let mut open = ctx.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    if toggled {
        open = !open;
    }
    if open {
        let preset = catalog
            .presets
            .get(&catalog.active_preset)
            .map_or(catalog.active_preset.as_str(), |preset| preset.name.as_str());
        egui::Window::new(format!("Tipke - {preset}"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .default_height(520.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for scope in scopes {
                        ui.heading(*scope);
                        egui::Grid::new(("qnc_key_cheatsheet", *scope))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                for line in lines(catalog, scope) {
                                    ui.monospace(line.keys);
                                    ui.label(line.label);
                                    ui.end_row();
                                }
                            });
                        ui.add_space(8.0);
                    }
                });
            });
    }
    ctx.data_mut(|data| data.insert_temp(id, open));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overview_lists_every_key_of_the_story_scope_with_its_label() {
        let catalog = ShortcutCatalog::from_json_str(include_str!(
            "../../../contracts/qnc-keyboard-shortcuts.json"
        ))
        .unwrap();
        let story = lines(&catalog, "storyboard");
        assert!(story.len() > 20);
        assert!(story.iter().any(|line| line.label == "Mark IN" && line.keys.contains('I')));
        assert!(story.iter().any(|line| line.label.contains("tipkovnički pregled")));
        assert!(lines(&catalog, "no-such-scope").is_empty());
    }
}
