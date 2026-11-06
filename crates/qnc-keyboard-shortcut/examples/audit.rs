//! Reads the keyboard catalog and lists what does not fit (samo citanje): the same chord on two
//! actions of one scope, actions with a key that no help group shows, help actions without a
//! key in the preset, and actions of the catalog nobody binds.
//! `cargo run -p qnc-keyboard-shortcut --example audit -- [preset]`

use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/qnc-keyboard-shortcuts.json");
    let catalog = qnc_keyboard_shortcut::ShortcutCatalog::from_json_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let preset_id = std::env::args().nth(1).unwrap_or_else(|| "default".into());
    let preset = &catalog.presets[&preset_id];
    println!("PRESET {preset_id} ({})", preset.name);
    let mut scopes: Vec<_> = preset.scopes.keys().collect();
    scopes.sort();
    for scope in &scopes {
        let mut chords: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for (action, list) in &preset.scopes[*scope] {
            for chord in list {
                chords.entry(chord.display()).or_default().push(action);
            }
        }
        for (chord, mut actions) in chords.into_iter().filter(|(_, actions)| actions.len() > 1) {
            actions.sort_unstable();
            println!("SAME KEY  [{scope}] {chord}: {}", actions.join(", "));
        }
    }
    let helped: BTreeSet<&str> = catalog.help.groups.iter().flat_map(|group| group.actions.iter().map(String::as_str)).collect();
    let keyed: BTreeSet<&str> = ["storyboard", "source_timeline"]
        .iter()
        .filter_map(|scope| preset.scopes.get(*scope))
        .flat_map(|scope| scope.keys().map(String::as_str))
        .collect();
    for action in keyed.difference(&helped) {
        let chords = ["storyboard", "source_timeline"].iter().find_map(|scope| catalog.chord_help(scope, action)).unwrap_or_default();
        println!("NOT IN HELP  {action} [{chords}]: {}", catalog.actions.get(*action).map_or("", |a| a.label.as_str()));
    }
    let all_keyed: BTreeSet<&str> = preset.scopes.values().flat_map(|scope| scope.keys().map(String::as_str)).collect();
    for action in &helped {
        if !all_keyed.contains(action) {
            println!("HELP WITHOUT KEY  {action}");
        }
    }
    for (scope, group) in catalog.help.groups.iter().map(|group| ("help", group)) {
        let mut seen = BTreeSet::new();
        for action in &group.actions {
            if !seen.insert(action) {
                println!("TWICE IN GROUP [{scope}] {}: {action}", group.title);
            }
        }
    }
}
