//! The editor of script profiles (user 2026-10-28: the form of the script is programmable; a
//! newsroom shapes its own kinds of blocks). It chooses a profile of this computer, changes it
//! (its name, the lock, each kind of block: name, mark, role, which length it counts in,
//! whether it is recorded, whether its first and last words are shown, its cue label and its
//! fields) and keeps it on this computer; a prepared profile is copied under its own code,
//! never written over. It can give the profile to the active project: the owner of the
//! program does that (`qnc-script-db`), the editor only asks.

use std::path::Path;

use eframe::egui::{self, RichText};
use qnc_desktop_theme::Theme;
use qnc_newsroom_sheet::{button, code_of, label, row_box, ROW_GAP, ROW_HEIGHT};
use qnc_script_profile::{Counts, KindSpec, Profiles, Role, ScriptProfile};

pub const MODULE_ID: &str = "qnc.module.script-profile-editor";

/// What the user asked of the program.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The active project's script is written with this profile from now on.
    UseForProject(ScriptProfile),
}

/// The profiles of this computer and the one being changed.
pub struct ProfileEditor {
    pub profiles: Profiles,
    editing: Option<ScriptProfile>,
    new_kind: String,
    /// A new field's name, per kind.
    new_fields: std::collections::BTreeMap<String, String>,
    pub message: String,
}

const ROLES: [(Role, &str); 4] = [(Role::Spoken, "sp.role_spoken"), (Role::Anchor, "sp.role_anchor"), (Role::Clip, "sp.role_clip"), (Role::Note, "sp.role_note")];
const COUNTS: [(Counts, &str); 3] = [(Counts::Story, "sp.counts_story"), (Counts::Anchor, "sp.counts_anchor"), (Counts::None, "sp.counts_none")];

impl ProfileEditor {
    pub fn new(root: &Path) -> Self {
        let (profiles, errors) = Profiles::load(root);
        Self { profiles, editing: None, new_kind: String::new(), new_fields: Default::default(), message: errors.join("\n") }
    }

    pub fn editing(&self) -> bool {
        self.editing.is_some()
    }
}

/// The editor over the whole of `ui`; `project` names the profile the active project is written
/// with (none without an active project).
pub fn show(ui: &mut egui::Ui, editor: &mut ProfileEditor, theme: &Theme, project: Option<&str>) -> Option<Answer> {
    ui.spacing_mut().interact_size.y = ROW_HEIGHT;
    ui.spacing_mut().item_spacing.y = ROW_GAP / 2.0;
    if editor.editing.is_some() {
        change(ui, editor, theme);
        return None;
    }
    let mut answer = None;
    let mut chosen = None;
    let width = ui.available_width();
    ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("sp.profile"));
        let shown = editor.profiles.current().map_or("—".to_string(), |profile| profile.name.clone());
        egui::ComboBox::from_id_salt("qnc_script_profile_choice").selected_text(shown).width(width * 0.4).show_ui(ui, |ui| {
            for (index, (profile, _)) in editor.profiles.list.iter().enumerate() {
                if ui.selectable_label(index == editor.profiles.chosen, &profile.name).clicked() {
                    chosen = Some(index);
                }
            }
        });
        if button(ui, qnc_ui_text::text("sp.edit"), editor.profiles.current().is_some(), theme) {
            editor.editing = editor.profiles.current().cloned();
        }
    });
    if let Some(index) = chosen {
        if let Err(error) = editor.profiles.choose(index) {
            editor.message = error;
        }
    }
    if let Some(profile) = editor.profiles.current() {
        for kind in &profile.kinds {
            let role = ROLES.iter().find(|(role, _)| *role == kind.role).map_or("", |(_, code)| qnc_ui_text::text(code));
            let fields = kind.fields.iter().map(|field| field.label.as_str()).collect::<Vec<_>>().join(", ");
            let line = format!("{}  {}  ({role}{}){}", kind.short, kind.label, if kind.recorded { ", REC" } else { "" }, if fields.is_empty() { String::new() } else { format!("  [{fields}]") });
            ui.label(RichText::new(line).color(theme.text_muted).size(theme.font_ui));
        }
    }
    ui.add_space(ROW_GAP);
    match project {
        Some(name) => {
            ui.label(RichText::new(qnc_ui_text::fill("sp.project_uses", &[("name", name)])).color(theme.text).size(theme.font_ui));
            if button(ui, qnc_ui_text::text("sp.use_for_project"), editor.profiles.current().is_some(), theme) {
                answer = editor.profiles.current().cloned().map(Answer::UseForProject);
            }
        }
        None => {
            ui.label(RichText::new(qnc_ui_text::text("sp.no_project")).color(theme.text_muted).size(theme.font_ui));
        }
    }
    if !editor.message.is_empty() {
        ui.label(RichText::new(&editor.message).color(theme.danger));
    }
    answer
}

/// The profile being changed: name, lock, kinds, a new kind; Save keeps it, Back leaves it.
fn change(ui: &mut egui::Ui, editor: &mut ProfileEditor, theme: &Theme) {
    let width = ui.available_width();
    let Some(profile) = editor.editing.as_mut() else { return };
    let locked = profile.locked;
    ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("sp.name"));
        ui.add_enabled(!locked, row_box(&mut profile.name, width * 0.5));
    });
    ui.checkbox(&mut profile.locked, qnc_ui_text::text("sp.locked"));
    ui.add_space(ROW_GAP);
    let (mut up, mut down, mut gone) = (None, None, None);
    let count = profile.kinds.len();
    for index in 0..count {
        let id = profile.kinds[index].id.clone();
        ui.add_enabled_ui(!locked, |ui| {
            egui::Frame::new().stroke(egui::Stroke::new(1.0, theme.border)).inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let kind = &mut profile.kinds[index];
                kind_rows(ui, kind, theme);
                let new_field = editor.new_fields.entry(id.clone()).or_default();
                qnc_newsroom_sheet::fields_editor(ui, &format!("qnc_script_profile_{id}"), &mut profile.kinds[index].fields, &mut profile.options, new_field, locked, theme);
                ui.horizontal(|ui| {
                    up = up.or((button(ui, "▲", index > 0, theme)).then_some(index));
                    down = down.or((button(ui, "▼", index + 1 < count, theme)).then_some(index));
                    gone = gone.or((button(ui, qnc_ui_text::text("sp.remove_kind"), true, theme)).then_some(index));
                });
            });
        });
    }
    if let Some(index) = up {
        profile.kinds.swap(index, index - 1);
    }
    if let Some(index) = down {
        profile.kinds.swap(index, index + 1);
    }
    if let Some(index) = gone {
        profile.kinds.remove(index);
    }
    ui.add_enabled_ui(!locked, |ui| {
        ui.horizontal(|ui| {
            ui.add(row_box(&mut editor.new_kind, width * 0.4).hint_text(qnc_ui_text::text("sp.new_kind_hint")));
            if button(ui, qnc_ui_text::text("sp.add_kind"), !editor.new_kind.trim().is_empty(), theme) {
                let kind = new_kind(profile, &editor.new_kind);
                profile.kinds.push(kind);
                editor.new_kind.clear();
            }
        });
    });
    ui.add_space(ROW_GAP);
    ui.horizontal(|ui| {
        if button(ui, qnc_ui_text::text("sp.save"), true, theme) {
            if let Some(profile) = editor.editing.take() {
                editor.message = match editor.profiles.keep(profile.clone()) {
                    Ok(()) => String::new(),
                    Err(error) => {
                        editor.editing = Some(profile);
                        error
                    }
                };
            }
        }
        if button(ui, qnc_ui_text::text("sheet.back"), true, theme) {
            editor.editing = None;
            editor.message.clear();
        }
    });
    if !editor.message.is_empty() {
        ui.label(RichText::new(&editor.message).color(theme.danger));
    }
}

/// The rows of one kind: name and mark; role, length, recorded and cues; cue label.
fn kind_rows(ui: &mut egui::Ui, kind: &mut KindSpec, theme: &Theme) {
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.add(row_box(&mut kind.label, width * 0.45));
        ui.add(row_box(&mut kind.short, width * 0.2).hint_text(qnc_ui_text::text("sp.short_hint")));
        ui.label(RichText::new(&kind.id).color(theme.text_muted).size(theme.font_ui));
    });
    ui.horizontal(|ui| {
        let role = ROLES.iter().find(|(role, _)| *role == kind.role).map_or("", |(_, code)| qnc_ui_text::text(code));
        egui::ComboBox::from_id_salt(("qnc_script_profile_role", &kind.id)).selected_text(role).width(150.0).show_ui(ui, |ui| {
            for (role, code) in ROLES {
                ui.selectable_value(&mut kind.role, role, qnc_ui_text::text(code));
            }
        });
        let counts = COUNTS.iter().find(|(counts, _)| *counts == kind.counts).map_or("", |(_, code)| qnc_ui_text::text(code));
        egui::ComboBox::from_id_salt(("qnc_script_profile_counts", &kind.id)).selected_text(counts).width(170.0).show_ui(ui, |ui| {
            for (counts, code) in COUNTS {
                ui.selectable_value(&mut kind.counts, counts, qnc_ui_text::text(code));
            }
        });
        // Only text the reporter reads is recorded; a note has no length.
        if kind.role != Role::Spoken {
            kind.recorded = false;
        }
        if kind.role == Role::Note {
            kind.counts = Counts::None;
        }
        ui.add_enabled(kind.role == Role::Spoken, egui::Checkbox::new(&mut kind.recorded, qnc_ui_text::text("sp.recorded")));
        ui.checkbox(&mut kind.cues, qnc_ui_text::text("sp.cues"));
    });
    ui.horizontal(|ui| {
        ui.add(row_box(&mut kind.cue, width * 0.45).hint_text(qnc_ui_text::text("sp.cue_hint")));
    });
}

/// A new kind named `label`: read by the reporter, in the length of the story; its code free.
pub fn new_kind(profile: &ScriptProfile, label: &str) -> KindSpec {
    let base = code_of(label);
    let taken = |code: &str| profile.kinds.iter().any(|kind| kind.id == code);
    let id = if taken(&base) { (2..).map(|n| format!("{base}_{n}")).find(|code| !taken(code)).unwrap_or_default() } else { base };
    let short = label.trim().to_uppercase();
    KindSpec { id, label: label.trim().to_string(), short, role: Role::Spoken, counts: Counts::Story, recorded: false, cues: false, cue: String::new(), fields: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_kind_gets_a_free_code_and_can_be_used() {
        let mut profile = ScriptProfile { profile_id: "test".into(), name: "Test".into(), locked: false, kinds: Vec::new(), options: Default::default() };
        profile.kinds.push(new_kind(&profile, "Voditelj uživo"));
        profile.kinds.push(new_kind(&profile, "Voditelj uživo"));
        assert_eq!(profile.kinds[0].id, "voditelj_uzivo");
        assert_eq!(profile.kinds[1].id, "voditelj_uzivo_2");
        assert!(profile.check().is_ok());
    }
}
