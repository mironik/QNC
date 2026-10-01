//! The keyboard overview (catalog `toggle_cheatsheet`, F1 / Shift+/), so nobody has to
//! remember the keys (user rule 2026-10-01). First what a kind of key means everywhere
//! (plain key does it, Ctrl+ takes control of something that exists, Shift+ adds
//! something new...), then the keys in tables by group or by action. Everything comes from
//! the catalog's `help` block and its bindings; this block changes nothing and knows no
//! application. Whether it is open, and which view, is window state of the UI context only.

use eframe::egui;
use qnc_keyboard_shortcut::ShortcutCatalog;

/// The catalog action that opens and closes the overview.
pub const TOGGLE_ACTION: &str = "toggle_cheatsheet";

/// One line of a table: the keys and what they do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub keys: String,
    pub label: String,
}

/// One table: a title and its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub title: String,
    pub lines: Vec<Line>,
}

/// The keys of an action from the first of `scopes` that binds it.
fn line(catalog: &ShortcutCatalog, scopes: &[&str], action_id: &str) -> Option<Line> {
    let keys = scopes.iter().find_map(|scope| catalog.chord_help(scope, action_id))?;
    let label = catalog
        .actions
        .get(action_id)
        .map_or_else(|| action_id.to_string(), |action| action.label.clone());
    Some(Line { keys, label })
}

/// The catalog's help groups, each with the actions bound in `scopes`.
pub fn by_group(catalog: &ShortcutCatalog, scopes: &[&str]) -> Vec<Table> {
    catalog
        .help
        .groups
        .iter()
        .map(|group| Table {
            title: group.title.clone(),
            lines: group.actions.iter().filter_map(|id| line(catalog, scopes, id)).collect(),
        })
        .filter(|table| !table.lines.is_empty())
        .collect()
}

/// Every action of the help groups bound in `scopes`, by its label.
pub fn by_action(catalog: &ShortcutCatalog, scopes: &[&str]) -> Vec<Line> {
    let mut lines: Vec<Line> = by_group(catalog, scopes).into_iter().flat_map(|table| table.lines).collect();
    lines.sort_by_key(|line| line.label.to_lowercase());
    lines.dedup();
    lines
}

fn grid(ui: &mut egui::Ui, id: impl std::hash::Hash, rows: impl Iterator<Item = (String, String)>) {
    egui::Grid::new(id).num_columns(2).striped(true).spacing([16.0, 4.0]).show(ui, |ui| {
        for (keys, text) in rows {
            ui.strong(keys);
            ui.label(text);
            ui.end_row();
        }
    });
}

/// Opens or closes the overview when `toggled`, and draws it while it is open.
pub fn show(ctx: &egui::Context, catalog: &ShortcutCatalog, scopes: &[&str], toggled: bool) {
    let open_id = egui::Id::new("qnc_key_cheatsheet_open");
    let view_id = egui::Id::new("qnc_key_cheatsheet_by_action");
    let mut open = ctx.data(|data| data.get_temp::<bool>(open_id).unwrap_or(false));
    let mut by_action_view = ctx.data(|data| data.get_temp::<bool>(view_id).unwrap_or(false));
    if toggled {
        open = !open;
    }
    if open {
        egui::Window::new("Tipke")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .default_height(620.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Kako čitati tipke");
                    let rules = catalog.help.rules.iter().map(|rule| (rule.keys.clone(), rule.meaning.clone()));
                    grid(ui, "qnc_key_cheatsheet_rules", rules);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut by_action_view, false, "Po grupama");
                        ui.selectable_value(&mut by_action_view, true, "Po akciji");
                    });
                    ui.separator();
                    if by_action_view {
                        let rows = by_action(catalog, scopes).into_iter().map(|line| (line.keys, line.label));
                        grid(ui, "qnc_key_cheatsheet_actions", rows);
                    } else {
                        for table in by_group(catalog, scopes) {
                            ui.heading(&table.title);
                            let rows = table.lines.into_iter().map(|line| (line.keys, line.label));
                            grid(ui, ("qnc_key_cheatsheet_group", &table.title), rows);
                            ui.add_space(8.0);
                        }
                    }
                });
            });
    }
    ctx.data_mut(|data| {
        data.insert_temp(open_id, open);
        data.insert_temp(view_id, by_action_view);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> ShortcutCatalog {
        ShortcutCatalog::from_json_str(include_str!("../../../contracts/qnc-keyboard-shortcuts.json")).unwrap()
    }

    #[test]
    fn the_overview_explains_the_modifiers_first_then_tables_by_group() {
        let catalog = catalog();
        assert!(catalog.help.rules.len() >= 4, "what a kind of key means comes first");
        let tables = by_group(&catalog, &["storyboard", "off"]);
        assert!(tables.len() >= 5);
        let in_out = tables.iter().find(|table| table.title == "IN / OUT").unwrap();
        assert!(in_out.lines.iter().any(|line| line.label == "Mark IN"));
        assert!(by_group(&catalog, &["no-such-scope"]).is_empty());
    }

    #[test]
    fn by_action_lists_each_key_once_sorted_by_label() {
        let lines = by_action(&catalog(), &["storyboard", "off"]);
        let labels: Vec<_> = lines.iter().map(|line| line.label.to_lowercase()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        assert_eq!(labels, sorted);
        assert!(lines.iter().all(|line| !line.keys.contains(" / m") && !line.keys.contains("Digit")));
    }
}
