//! The AI features of a project (docs/102, user 2026-11-11): QNC works without any of them, a
//! project turns on the ones it wants. The list is data (`catalogs/ai/features.json`): each
//! feature names the features it needs. One rule keeps them: turning a feature on turns on what
//! it needs, a feature another chosen feature needs cannot be turned off, and a choice that breaks
//! this is an error, never mended. An AI template is a named choice: QNC brings some
//! (`catalogs/ai/templates.json`, locked), a newsroom keeps its own on this computer
//! (`data/ai-templates`). A project keeps its template and its features in its settings
//! (`ai.template`, `ai.features`), apart from its technical template.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.ai-features";

/// The codes the runners ask for.
pub const TRANSCRIPTION: &str = "transcription";
pub const LETTER_TIMES: &str = "letter_times";
pub const READ_CHECK: &str = "read_check";
pub const SHOT_ANALYSIS: &str = "shot_analysis";
pub const SHOT_CONTENT: &str = "shot_content";
pub const SHOT_CAPTION: &str = "shot_caption";
pub const BROLL_SUGGEST: &str = "broll_suggest";
pub const TRANSLATE: &str = "translate";
pub const SMOOTH_CUT: &str = "smooth_cut";

/// The AI template of a project that turns nothing on.
pub const NO_AI: &str = "none";

/// Where the choice is kept in the settings of a project.
pub const TEMPLATE_PATH: &str = "template";
pub const FEATURES_PATH: &str = "features";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    pub code: String,
    /// Text code of its name.
    pub name: String,
    pub needs: Vec<String>,
    /// The catalog actions of the forms that start it: grey in every form while the project has it
    /// off (docs/102 step 3).
    #[serde(default)]
    pub actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureList {
    #[serde(default, rename = "note")]
    _note: String,
    features: Vec<Feature>,
}

/// The AI features, in the order a form shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Features {
    pub list: Vec<Feature>,
}

impl Features {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("catalogs").join("ai").join("features.json");
        let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let list: FeatureList = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let features = Self { list: list.features };
        for (index, feature) in features.list.iter().enumerate() {
            if feature.code.is_empty() || features.list[..index].iter().any(|earlier| earlier.code == feature.code) {
                return Err(format!("feature code '{}' is empty or twice", feature.code));
            }
            if let Some(need) = feature.needs.iter().find(|need| features.get(need).is_none()) {
                return Err(format!("feature '{}' needs '{need}', which is not in the list", feature.code));
            }
            if features.all_needs(&feature.code).contains(&feature.code) {
                return Err(format!("feature '{}' needs itself", feature.code));
            }
            if let Some(action) = feature.actions.iter().find(|action| features.list[..index].iter().any(|earlier| earlier.actions.contains(action))) {
                return Err(format!("action '{action}' starts two features"));
            }
        }
        Ok(features)
    }

    pub fn get(&self, code: &str) -> Option<&Feature> {
        self.list.iter().find(|feature| feature.code == code)
    }

    /// Everything `code` needs, through the features it needs.
    pub fn all_needs(&self, code: &str) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        let mut open: Vec<String> = self.get(code).map(|feature| feature.needs.clone()).unwrap_or_default();
        while let Some(next) = open.pop() {
            if found.contains(&next) || found.len() > self.list.len() {
                continue;
            }
            open.extend(self.get(&next).map(|feature| feature.needs.clone()).unwrap_or_default());
            found.push(next);
        }
        found
    }

    /// The chosen features that need `code` (through others too).
    pub fn needed_by<'a>(&'a self, chosen: &[String], code: &str) -> Vec<&'a Feature> {
        self.list
            .iter()
            .filter(|feature| feature.code != code && chosen.contains(&feature.code) && self.all_needs(&feature.code).iter().any(|need| need == code))
            .collect()
    }

    /// `code` and what it needs turned on; the choice keeps the order of the list.
    pub fn turn_on(&self, chosen: &mut Vec<String>, code: &str) {
        let mut wanted: Vec<String> = chosen.clone();
        wanted.push(code.to_string());
        wanted.extend(self.all_needs(code));
        *chosen = self.list.iter().filter(|feature| wanted.contains(&feature.code)).map(|feature| feature.code.clone()).collect();
    }

    /// `code` turned off; refused, naming them, while chosen features need it.
    pub fn turn_off(&self, chosen: &mut Vec<String>, code: &str) -> Result<(), Vec<String>> {
        let by: Vec<String> = self.needed_by(chosen, code).iter().map(|feature| feature.name.clone()).collect();
        if !by.is_empty() {
            return Err(by);
        }
        chosen.retain(|chosen| chosen != code);
        Ok(())
    }

    /// Whether a choice keeps the rule: every code known, everything needed chosen.
    pub fn check(&self, chosen: &[String]) -> Result<(), String> {
        for code in chosen {
            if self.get(code).is_none() {
                return Err(qnc_ui_text::fill("ai.unknown_feature", &[("code", code)]));
            }
            if let Some(need) = self.all_needs(code).into_iter().find(|need| !chosen.contains(need)) {
                return Err(qnc_ui_text::fill("ai.need_missing", &[("code", code), ("need", &need)]));
            }
        }
        Ok(())
    }
}

/// A named choice of AI features.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiTemplate {
    pub id: String,
    /// A text code for a prepared template, the name as typed for an own one.
    pub name: String,
    pub features: Vec<String>,
    /// Kept on this computer; it may be deleted.
    #[serde(skip)]
    pub own: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateList {
    #[serde(default, rename = "note")]
    _note: String,
    templates: Vec<AiTemplate>,
}

/// The AI templates: prepared first, then the own ones. One that breaks the rule of the features
/// is left out and named in the errors.
pub fn templates(root: &Path, features: &Features) -> (Vec<AiTemplate>, Vec<String>) {
    let mut list = Vec::new();
    let mut errors = Vec::new();
    let prepared = root.join("catalogs").join("ai").join("templates.json");
    match fs::read_to_string(&prepared).map_err(|error| error.to_string()).and_then(|text| serde_json::from_str::<TemplateList>(&text).map_err(|error| error.to_string())) {
        Ok(read) => list.extend(read.templates),
        Err(error) => errors.push(format!("{}: {error}", prepared.display())),
    }
    let mut files: Vec<PathBuf> = fs::read_dir(own_dir(root)).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
    files.retain(|file| file.extension().is_some_and(|extension| extension == "json"));
    files.sort();
    for file in files {
        match fs::read_to_string(&file).map_err(|error| error.to_string()).and_then(|text| serde_json::from_str::<AiTemplate>(&text).map_err(|error| error.to_string())) {
            Ok(template) => list.push(AiTemplate { own: true, ..template }),
            Err(error) => errors.push(format!("{}: {error}", file.display())),
        }
    }
    list.retain(|template| match features.check(&template.features) {
        Ok(()) => true,
        Err(error) => {
            errors.push(format!("{}: {error}", template.id));
            false
        }
    });
    let mut seen: Vec<String> = Vec::new();
    list.retain(|template| {
        let first = !seen.contains(&template.id);
        seen.push(template.id.clone());
        if !first {
            errors.push(qnc_ui_text::fill("ai.template_twice", &[("id", &template.id)]));
        }
        first
    });
    (list, errors)
}

/// Keeps `features` as an own template named `name` and gives its id. A name of a template
/// already kept is written over; a prepared template is never written over.
pub fn keep_template(root: &Path, name: &str, features: &[String], known: &[AiTemplate]) -> Result<AiTemplate, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(qnc_ui_text::text("ai.template_needs_name").to_string());
    }
    let id = match known.iter().find(|template| template.own && template.name == name) {
        Some(template) => template.id.clone(),
        None => free_id(name, known),
    };
    let template = AiTemplate { id, name: name.to_string(), features: features.to_vec(), own: true };
    let dir = own_dir(root);
    fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let file = dir.join(format!("{}.json", template.id));
    let text = serde_json::to_string_pretty(&template).map_err(|error| error.to_string())?;
    fs::write(&file, text).map_err(|error| format!("{}: {error}", file.display()))?;
    Ok(template)
}

/// Deletes an own template; a prepared one is refused.
pub fn delete_template(root: &Path, template: &AiTemplate) -> Result<(), String> {
    if !template.own {
        return Err(qnc_ui_text::text("ai.template_prepared").to_string());
    }
    let file = own_dir(root).join(format!("{}.json", template.id));
    fs::remove_file(&file).map_err(|error| format!("{}: {error}", file.display()))
}

fn free_id(name: &str, known: &[AiTemplate]) -> String {
    let mut base: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    base = base.trim_matches('_').to_string();
    if base.is_empty() {
        base = "ai".to_string();
    }
    let base = format!("own_{base}");
    let mut id = base.clone();
    let mut n = 2;
    while known.iter().any(|template| template.id == id) {
        id = format!("{base}_{n}");
        n += 1;
    }
    id
}

fn own_dir(root: &Path) -> PathBuf {
    qnc_dev_diagnostics::config_path(root).with_file_name("ai-templates")
}

/// The AI template a project chose (`ai.template`), `none` while it chose none.
pub fn template_of(settings: &Value) -> String {
    settings.get("ai").and_then(|ai| ai.get(TEMPLATE_PATH)).and_then(Value::as_str).unwrap_or(NO_AI).to_string()
}

/// The AI features a project turned on (`ai.features`).
pub fn chosen_of(settings: &Value) -> Vec<String> {
    settings
        .get("ai")
        .and_then(|ai| ai.get(FEATURES_PATH))
        .and_then(Value::as_array)
        .map(|codes| codes.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Writes the choice into the settings of a project.
pub fn set_choice(settings: &mut Value, template: &str, chosen: &[String]) {
    if !settings.is_object() {
        *settings = Value::Object(Default::default());
    }
    let root = settings.as_object_mut().expect("an object");
    let ai = root.entry("ai").or_insert_with(|| Value::Object(Default::default()));
    if !ai.is_object() {
        *ai = Value::Object(Default::default());
    }
    let ai = ai.as_object_mut().expect("an object");
    ai.insert(TEMPLATE_PATH.to_string(), Value::String(template.to_string()));
    ai.insert(FEATURES_PATH.to_string(), Value::Array(chosen.iter().cloned().map(Value::String).collect()));
}

/// The AI choice a form edits: the features, the templates and the name of a template to keep.
#[derive(Debug, Clone)]
pub struct Choice {
    root: PathBuf,
    pub features: Result<Features, String>,
    pub templates: Vec<AiTemplate>,
    pub errors: Vec<String>,
    /// The name typed for "Keep as AI template".
    pub name: String,
    pub message: String,
}

impl Choice {
    pub fn load(root: &Path) -> Self {
        let features = Features::load(root);
        let (templates, errors) = match &features {
            Ok(features) => templates(root, features),
            Err(_) => (Vec::new(), Vec::new()),
        };
        Self { root: root.to_path_buf(), features, templates, errors, name: String::new(), message: String::new() }
    }

    /// Takes template `id` into the settings.
    pub fn choose_template(&mut self, settings: &mut Value, id: &str) {
        if let Some(template) = self.templates.iter().find(|template| template.id == id) {
            set_choice(settings, &template.id, &template.features);
            self.message.clear();
        }
    }

    /// Turns feature `code` on or off in the settings; turning off what a chosen feature needs
    /// is refused and said.
    pub fn switch(&mut self, settings: &mut Value, code: &str, on: bool) {
        let Ok(features) = &self.features else { return };
        let mut chosen = chosen_of(settings);
        if on {
            features.turn_on(&mut chosen, code);
        } else if let Err(by) = features.turn_off(&mut chosen, code) {
            let names: Vec<String> = by.iter().map(|name| words_of(name)).collect();
            self.message = qnc_ui_text::fill("ai.needed_by", &[("names", &names.join(", "))]);
            return;
        }
        let template = template_of(settings);
        set_choice(settings, &template, &chosen);
        self.message.clear();
    }

    /// Keeps the features of the settings as an own AI template named as typed and chooses it.
    pub fn keep_template(&mut self, settings: &mut Value) {
        let chosen = chosen_of(settings);
        match keep_template(&self.root, &self.name, &chosen, &self.templates) {
            Ok(template) => {
                set_choice(settings, &template.id, &chosen);
                self.message = qnc_ui_text::fill("ai.template_kept", &[("name", &template.name)]);
                self.name.clear();
                self.reload();
            }
            Err(error) => self.message = error,
        }
    }

    /// Deletes own template `id`; a project that chose it keeps its features.
    pub fn delete_template(&mut self, id: &str) {
        let Some(template) = self.templates.iter().find(|template| template.id == id).cloned() else { return };
        match delete_template(&self.root, &template) {
            Ok(()) => {
                self.message = qnc_ui_text::fill("ai.template_deleted", &[("name", &template.name)]);
                self.reload();
            }
            Err(error) => self.message = error,
        }
    }

    fn reload(&mut self) {
        if let Ok(features) = &self.features {
            (self.templates, self.errors) = templates(&self.root, features);
        }
    }

    /// The name of template `template` as shown.
    pub fn template_name(template: &AiTemplate) -> String {
        if template.own {
            template.name.clone()
        } else {
            words_of(&template.name)
        }
    }
}

#[cfg(test)]
mod tests;

/// The words of a text code that comes from data (a feature, a prepared template), in this
/// computer's language; a code the dictionary lacks shows as itself.
pub fn words_of(code: &str) -> String {
    qnc_ui_text::host().ok().and_then(|texts| texts.get(code).ok()).unwrap_or(code).to_string()
}

/// The actions of the forms an AI feature starts (`actions` of the list), grey while the
/// project has that feature off (docs/102 step 3: the AI template chosen when the project was
/// made says what it does). Without a project every action is allowed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gate {
    off: Vec<String>,
}

impl Gate {
    /// The gate of a project whose feature `code` is on when `on(code)`.
    pub fn new(features: &Features, on: impl Fn(&str) -> bool) -> Self {
        Self { off: features.list.iter().filter(|feature| !on(&feature.code)).flat_map(|feature| feature.actions.iter().cloned()).collect() }
    }

    pub fn allows(&self, action: &str) -> bool {
        !self.off.iter().any(|off| off == action)
    }
}
