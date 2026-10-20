//! The pool tabs panel (user 2026-10-20, rule 11): the row of the media pool's tabs (All,
//! SubClip, B-roll, Segment, as a form's layout lists them) on its place, in the look of
//! the one theme. A tab is a catalog action; the form says which is chosen and which can be
//! used. It knows no form.

use eframe::egui::{Rect, Ui};
use qnc_media_pool_head::{RowClick, RowTab};

pub const MODULE_ID: &str = "qnc.module.pool-tabs-panel";

/// One tab as the layout lists it.
pub struct PoolTab<'a> {
    pub label: &'a str,
    pub action_id: Option<&'a str>,
}

/// The tabs in `rect`; the clicked tab's action. Without a chosen tab the first is shown
/// chosen; a tab without an action cannot be used.
pub fn show<'a>(
    ui: &mut Ui,
    rect: Rect,
    (theme, tab_gap): (&qnc_desktop_theme::Theme, f32),
    tabs: &[PoolTab<'a>],
    chosen: impl Fn(&str) -> bool,
    enabled: impl Fn(&str) -> bool,
) -> Option<&'a str> {
    let any_chosen = tabs.iter().any(|tab| tab.action_id.is_some_and(&chosen));
    let row: Vec<_> = tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| RowTab {
            label: tab.label,
            selected: tab.action_id.map_or(false, &chosen) || (!any_chosen && index == 0),
            enabled: tab.action_id.is_some_and(&enabled),
        })
        .collect();
    let style = qnc_theme_styles::row(theme, tab_gap);
    let mut clicked = None;
    qnc_source_dock::show_chrome_row(ui, rect, &qnc_theme_styles::dock(theme, 0.0), theme.surface, true, |ui| {
        if let Some(RowClick::Tab(index)) = qnc_media_pool_head::show_row(ui, &style, &row, &[]) {
            clicked = tabs[index].action_id;
        }
    });
    clicked
}
