//! The keys of one frame as catalog action ids (user rule 2026-09-30: everything is a
//! block; moved out of the Ingest and editorial forms, which had the same loop). Input
//! goes through the external keyboard catalog only: key -> catalog -> action id; the
//! caller turns each id into its own intent.

use eframe::egui;
use qnc_keyboard_shortcut::ShortcutCatalog;

/// The action ids of this frame's keys, in order: first every press of `play_action`
/// in the first scope (taken out of the input, so nothing else sees it), then for every
/// key the actions of `scopes`, each action once per key.
pub fn action_ids(
    ctx: &egui::Context,
    catalog: &ShortcutCatalog,
    scopes: &[&str],
    play_action: &str,
) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(first) = scopes.first() {
        let presses = qnc_keyboard_shortcut::consume_egui_action_presses(ctx, catalog, first, play_action);
        ids.extend(std::iter::repeat_n(play_action.to_string(), presses));
    }
    for event in qnc_keyboard_shortcut::egui_shortcut_events(ctx) {
        let mut this_key: Vec<String> = Vec::new();
        for scope in scopes {
            for action_id in catalog.action_ids_for_event(scope, &event) {
                if !this_key.iter().any(|seen| seen == action_id) {
                    this_key.push(action_id.to_string());
                }
            }
        }
        ids.extend(this_key);
    }
    ids
}
