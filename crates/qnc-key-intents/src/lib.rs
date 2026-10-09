//! The keys of one frame as catalog action ids (user rule 2026-09-30: everything is a
//! block; moved out of the Ingest and editorial forms, which had the same loop). Input
//! goes through the external keyboard catalog only: key -> catalog -> action id; the
//! caller turns each id into its own intent.

use eframe::egui;
use qnc_keyboard_shortcut::ShortcutCatalog;

/// The preset the active project chose, read in the background once per process (the QNC
/// root of this process; none without one).
fn project_preset() -> Option<String> {
    static WATCHER: std::sync::OnceLock<Option<qnc_key_preset::KeyPresetWatcher>> =
        std::sync::OnceLock::new();
    WATCHER
        .get_or_init(|| qnc_dev_diagnostics::locate_qnc_root().map(qnc_key_preset::KeyPresetWatcher::start))
        .as_ref()?
        .preset()
}

/// The catalog with the preset the project chose (Project, Advanced), kept between frames;
/// `None` when that is the catalog's own preset or the catalog does not have it.
fn in_preset(ctx: &egui::Context, catalog: &ShortcutCatalog, preset: Option<&str>) -> Option<std::sync::Arc<ShortcutCatalog>> {
    let preset = preset.filter(|id| *id != catalog.active_preset && catalog.presets.contains_key(*id))?;
    let id = egui::Id::new(("qnc-key-intents-preset", preset));
    Some(ctx.data_mut(|data| {
        data.get_temp_mut_or_insert_with(id, || {
            let mut chosen = catalog.clone();
            chosen.active_preset = preset.to_string();
            std::sync::Arc::new(chosen)
        })
        .clone()
    }))
}

/// The action ids of this frame's keys, in order: first every press of `play_action`
/// in the first scope (taken out of the input, so nothing else sees it), then for every
/// key the actions of `scopes`, each action once per key. The keyboard overview
/// (`toggle_cheatsheet`) is opened and drawn here, so every board has it. The keys are
/// those of the preset the active project chose (Project, Advanced), for every form.
pub fn action_ids(
    ctx: &egui::Context,
    catalog: &ShortcutCatalog,
    scopes: &[&str],
    play_action: &str,
) -> Vec<String> {
    let preset = project_preset();
    let chosen = in_preset(ctx, catalog, preset.as_deref());
    let catalog = chosen.as_deref().unwrap_or(catalog);
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
    let toggled = ids.iter().any(|id| id == qnc_key_cheatsheet::TOGGLE_ACTION);
    ids.retain(|id| id != qnc_key_cheatsheet::TOGGLE_ACTION);
    qnc_key_cheatsheet::show(ctx, catalog, scopes, toggled);
    ids
}

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn the_keys_follow_the_preset_the_project_chose() {
        let catalog = ShortcutCatalog::from_json_str(include_str!("../../../contracts/qnc-keyboard-shortcuts.json")).unwrap();
        let ctx = egui::Context::default();
        let chosen = in_preset(&ctx, &catalog, Some("resolve")).unwrap();
        assert_eq!(chosen.active_preset, "resolve");
        assert!(in_preset(&ctx, &catalog, Some(&catalog.active_preset)).is_none(), "the catalog's own preset");
        assert!(in_preset(&ctx, &catalog, Some("no-such-preset")).is_none(), "an unknown preset keeps the catalog");
        assert!(in_preset(&ctx, &catalog, None).is_none());
    }
}
