//! Script profiles (user 2026-10-28: "the application is for general use; this part is
//! programmable, not fixed, and can be changed"). A profile says what a newsroom writes in a
//! script: the kinds of blocks (anchor lead-in, OFF, SOT, stand-up, natural sound, graphics,
//! tag, or kinds QNC does not foresee), how each behaves, the label of its numbered cue (as
//! iNEWS numbers its production cues) and the fields that go with it (the caption of a
//! statement: name and role). Names are free; codes are what the database keeps.
//!
//! The profiles QNC brings are in `catalogs/script-profiles` (their names are text codes);
//! the ones a newsroom made on this computer are beside its other settings
//! (`data/script-profiles`), and the one chosen in `data/script-profile.json`. A prepared
//! profile is never written over: a copy gets its own code. It knows no form or database;
//! a project keeps the profile it was written with in its database (`qnc-script-db`).

use std::fs;
use std::path::{Path, PathBuf};

use qnc_field_spec::{code_of, valid_code};
use qnc_field_spec::{FieldSpec, OptionSets};
use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.script-profile";

/// The profile chosen on a computer where none was chosen (the general TV one QNC brings; any
/// prepared one when it is not there).
pub const DEFAULT_PROFILE: &str = "general_tv";

/// What a kind of block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Text the reporter reads (an OFF): its length comes from its words, or its recording.
    Spoken,
    /// Text the anchor reads in the studio (lead-in, tag): not in the length of the story.
    Anchor,
    /// A range of a clip (a statement, a stand-up, natural sound): its length is the range.
    Clip,
    /// An instruction (graphics, a note for the director): no length.
    Note,
}

/// Which length a block of a kind counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Counts {
    /// The length of the story (TRT).
    Story,
    /// The anchor's time in the studio.
    Anchor,
    None,
}

/// One kind of block of a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindSpec {
    /// The code the database keeps (lower-case letters, digits, `_`).
    pub id: String,
    pub label: String,
    /// The short mark on the block (for example `OFF`, `SOT`).
    pub short: String,
    pub role: Role,
    pub counts: Counts,
    /// Whether a block of this kind is recorded from the microphone.
    #[serde(default)]
    pub recorded: bool,
    /// Whether the first and last words of a block are shown (incue, outcue).
    #[serde(default)]
    pub cues: bool,
    /// The label of its numbered cue, `{n}` its number among the blocks of its kind (empty:
    /// not numbered), for example `OFF {n}` or `Take SOT {n}`.
    #[serde(default)]
    pub cue: String,
    /// The fields that go with a block of this kind (for example the caption of a statement).
    #[serde(default)]
    pub fields: Vec<FieldSpec>,
}

/// A script profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptProfile {
    pub profile_id: String,
    pub name: String,
    /// While locked the profile is used as it is; it is not changed.
    #[serde(default)]
    pub locked: bool,
    pub kinds: Vec<KindSpec>,
    #[serde(default)]
    pub options: OptionSets,
}

impl ScriptProfile {
    pub fn kind(&self, id: &str) -> Option<&KindSpec> {
        self.kinds.iter().find(|kind| kind.id == id)
    }

    /// The label of the cue of the `number`-th block of kind `id` (empty when not numbered).
    pub fn cue_label(&self, id: &str, number: usize) -> String {
        self.cue_label_with(id, number, &std::collections::BTreeMap::new())
    }

    /// The cue label with the block's field values in it: `{code}` of a field becomes the name
    /// of its choice or its value (for example `*CG {cg_template}` becomes `*CG 2line`).
    pub fn cue_label_with(&self, id: &str, number: usize, fields: &std::collections::BTreeMap<String, String>) -> String {
        let Some(kind) = self.kind(id) else { return String::new() };
        let mut label = kind.cue.replace("{n}", &number.to_string());
        for field in &kind.fields {
            let value = fields.get(&field.path).cloned().unwrap_or_else(|| field.default.clone());
            let shown = field
                .options
                .as_ref()
                .and_then(|set| self.options.get(set))
                .and_then(|choices| choices.iter().find(|(choice, _)| *choice == value))
                .map_or(value.clone(), |(_, name)| name.clone());
            label = label.replace(&format!("{{{}}}", field.path), &shown);
        }
        label
    }

    /// Why the profile cannot be used, if it cannot.
    pub fn check(&self) -> Result<(), String> {
        if !valid_code(&self.profile_id) {
            return Err(qnc_ui_text::fill("script_profile.bad_code", &[("code", &self.profile_id)]));
        }
        if self.kinds.is_empty() {
            return Err(qnc_ui_text::text("script_profile.no_kinds").into());
        }
        for (index, kind) in self.kinds.iter().enumerate() {
            if !valid_code(&kind.id) {
                return Err(qnc_ui_text::fill("script_profile.bad_code", &[("code", &kind.id)]));
            }
            if self.kinds[..index].iter().any(|other| other.id == kind.id) {
                return Err(qnc_ui_text::fill("script_profile.twice", &[("code", &kind.id)]));
            }
            if kind.recorded && kind.role != Role::Spoken {
                return Err(qnc_ui_text::fill("script_profile.recorded_spoken", &[("code", &kind.id)]));
            }
            if kind.role == Role::Note && kind.counts != Counts::None {
                return Err(qnc_ui_text::fill("script_profile.note_counts", &[("code", &kind.id)]));
            }
            if let Some(field) = kind.fields.iter().find(|field| !valid_code(&field.path)) {
                return Err(qnc_ui_text::fill("script_profile.bad_code", &[("code", &field.path)]));
            }
            if let Some(field) = kind.fields.iter().find(|field| field.options.as_ref().is_some_and(|set| !self.options.contains_key(set))) {
                return Err(qnc_ui_text::fill("script_profile.no_options", &[("code", &field.path)]));
            }
        }
        Ok(())
    }
}

/// Where the profile comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Brought by QNC: it is copied, never changed.
    Prepared,
    /// Made on this computer.
    Own,
}

/// The profiles of a QNC root and the one chosen.
#[derive(Debug, Clone)]
pub struct Profiles {
    root: PathBuf,
    pub list: Vec<(ScriptProfile, Origin)>,
    pub chosen: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct Chosen {
    selected: String,
}

impl Profiles {
    /// The profiles of the QNC root; one that cannot be read or used is left out and named in
    /// the errors.
    pub fn load(root: &Path) -> (Self, Vec<String>) {
        let mut list: Vec<(ScriptProfile, Origin)> = Vec::new();
        let mut errors = Vec::new();
        for (dir, origin) in [(prepared_dir(root), Origin::Prepared), (own_dir(root), Origin::Own)] {
            let mut files: Vec<PathBuf> = fs::read_dir(&dir).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
            files.retain(|file| file.extension().is_some_and(|extension| extension == "json"));
            files.sort();
            for file in files {
                match read_profile(&file) {
                    Ok(profile) if list.iter().any(|(other, _)| other.profile_id == profile.profile_id) => {
                        errors.push(format!("{}: {}", file.display(), qnc_ui_text::fill("script_profile.twice", &[("code", &profile.profile_id)])));
                    }
                    Ok(profile) => list.push((profile, origin)),
                    Err(error) => errors.push(format!("{}: {error}", file.display())),
                }
            }
        }
        let selected = fs::read_to_string(chosen_file(root)).ok().and_then(|text| serde_json::from_str::<Chosen>(&text).ok()).map(|chosen| chosen.selected);
        let at = |id: &str| list.iter().position(|(profile, _)| profile.profile_id == id);
        let chosen = selected.and_then(|id| at(&id)).or_else(|| at(DEFAULT_PROFILE)).unwrap_or(0);
        (Self { root: root.to_path_buf(), list, chosen }, errors)
    }

    pub fn current(&self) -> Option<&ScriptProfile> {
        self.list.get(self.chosen).map(|(profile, _)| profile)
    }

    /// Chooses profile `index` for the next projects on this computer.
    pub fn choose(&mut self, index: usize) -> Result<(), String> {
        let Some((profile, _)) = self.list.get(index) else { return Ok(()) };
        self.chosen = index;
        write_json(&chosen_file(&self.root), &Chosen { selected: profile.profile_id.clone() })
    }

    /// Keeps `profile` as one of this computer (a new one, or the own one of the same code) and
    /// chooses it. A prepared profile is never written over: its copy gets its own code.
    pub fn keep(&mut self, mut profile: ScriptProfile) -> Result<(), String> {
        let prepared = self.list.iter().any(|(other, origin)| *origin == Origin::Prepared && other.profile_id == profile.profile_id);
        if prepared || !valid_code(&profile.profile_id) {
            profile.profile_id = free_id(&self.list, &code_of(&profile.name));
        }
        profile.check()?;
        write_json(&own_dir(&self.root).join(format!("{}.json", profile.profile_id)), &profile)?;
        let index = match self.list.iter().position(|(other, _)| other.profile_id == profile.profile_id) {
            Some(index) => {
                self.list[index] = (profile, Origin::Own);
                index
            }
            None => {
                self.list.push((profile, Origin::Own));
                self.list.len() - 1
            }
        };
        self.choose(index)
    }
}

/// A profile file; its names may be text codes (`@...`) of the dictionary.
fn read_profile(file: &Path) -> Result<ScriptProfile, String> {
    let text = fs::read_to_string(file).map_err(|error| error.to_string())?;
    let value = qnc_ui_text::resolve_contract(&text)?;
    let profile: ScriptProfile = serde_json::from_value(value).map_err(|error| error.to_string())?;
    profile.check()?;
    Ok(profile)
}

fn write_json(file: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(file, text).map_err(|error| error.to_string())
}

fn prepared_dir(root: &Path) -> PathBuf {
    root.join("catalogs").join("script-profiles")
}

/// Beside the other settings of this computer.
fn own_dir(root: &Path) -> PathBuf {
    qnc_dev_diagnostics::config_path(root).with_file_name("script-profiles")
}

fn chosen_file(root: &Path) -> PathBuf {
    qnc_dev_diagnostics::config_path(root).with_file_name("script-profile.json")
}

fn free_id(list: &[(ScriptProfile, Origin)], base: &str) -> String {
    let taken = |id: &str| list.iter().any(|(profile, _)| profile.profile_id == id);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}_{n}")).find(|id| !taken(id)).unwrap_or_default()
}

#[cfg(test)]
mod tests;
