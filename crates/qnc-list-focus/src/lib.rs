//! The keyboard on a drawn list (docs/99 step 3): which item of the list the keyboard is on
//! and which one Enter just opened, kept in the drawing's memory between frames. The walk of
//! the panels and the steps are the core piece `qnc-focus-walk`; this reads them where the
//! list is drawn.

use eframe::egui;
use qnc_focus_walk::{GridStep, WalkView};

pub const MODULE_ID: &str = "qnc.module.list-focus";

/// The focused item of a drawn list in focus (`step` from the walk view; each number is
/// applied once): it starts on `selected` and moves by `step` in a list of `count` items
/// in `columns` columns with `page_rows` rows on screen. Returns the focused item and the
/// step just applied (Enter, the tick key) so the list can act on it as on a click.
pub fn list_focus(
    ui: &egui::Ui,
    id: egui::Id,
    (count, columns, page_rows): (usize, usize, usize),
    selected: Option<usize>,
    step: Option<(GridStep, u64)>,
) -> (usize, Option<GridStep>) {
    let (key, applied) = (id.with("qnc-list-focus"), id.with("qnc-list-step"));
    if count == 0 {
        return (0, None);
    }
    let fresh = step.filter(|(_, number)| ui.data(|data| data.get_temp::<u64>(applied)) != Some(*number));
    let from = ui.data(|data| data.get_temp::<usize>(key)).or(selected).unwrap_or(0).min(count - 1);
    let focused = fresh.map_or(from, |(step, _)| qnc_key_list::step_focus(from, step, columns, count, page_rows));
    ui.data_mut(|data| {
        data.insert_temp(key, focused);
        if let Some((_, number)) = fresh {
            data.insert_temp(applied, number);
        }
    });
    (focused, fresh.map(|(step, _)| step))
}

/// The item of a drawn list the keyboard is on (when `panel` has the focus) and the item
/// Enter just opened, which the list opens as a click.
pub fn list_keys(ui: &egui::Ui, id: &str, count: usize, view: &WalkView, panel: &str) -> (Option<usize>, Option<usize>) {
    let (index, applied) = list_focus(ui, egui::Id::new(id), (count, 1, 12), None, view.step_of(panel));
    let opened = (applied == Some(GridStep::Open) && index < count).then_some(index);
    (view.is(panel).then_some(index), opened)
}

