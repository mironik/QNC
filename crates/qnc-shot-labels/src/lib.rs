//! The labels a usable part of a clip may get (user 2026-11-10: sorting the B-roll by what its
//! pictures show). The list is data (`catalogs/ai/shot-labels.json`), not code: a newsroom adds,
//! removes and renames labels there. Which labels a part gets is worked out from the part's kept
//! vector and the labels' text vectors, so a changed list needs no new look at the pictures.
//! Pure: it reads only its list and knows no model, database or form.

use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const MODULE_ID: &str = "qnc.module.shot-labels";
/// The list beside the application, below its folder or one of its parents.
pub const LIST: &str = "catalogs/ai/shot-labels.json";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    /// The label's id.
    pub code: String,
    /// The dictionary key of its name.
    pub text: String,
    /// What the model is asked.
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labels {
    pub version: u32,
    #[serde(default)]
    pub about: String,
    /// The least score a part's label has.
    pub least: f32,
    pub labels: Vec<Label>,
}

impl Labels {
    /// The list at `path`; an empty list, a label without a code or prompt, or one code twice
    /// is an error, never a guess.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let labels: Self = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut codes: Vec<&str> = labels.labels.iter().map(|label| label.code.as_str()).collect();
        codes.sort_unstable();
        let twice = codes.windows(2).any(|pair| pair[0] == pair[1]);
        if labels.labels.is_empty() || twice || labels.labels.iter().any(|l| l.code.is_empty() || l.prompt.trim().is_empty()) || !(0.0..1.0).contains(&labels.least) {
            return Err(format!("{}: not a usable list of labels", path.display()));
        }
        Ok(labels)
    }

    /// The prompts, in the order of the labels.
    pub fn prompts(&self) -> Vec<String> {
        self.labels.iter().map(|label| label.prompt.clone()).collect()
    }

    /// The labels a part gets from the model's logits of its vector against the prompts (in the
    /// order of the labels): each label's share among all labels (softmax: a part is compared
    /// with the whole list, as the labels' own sigmoid scores are low for any one caption), those
    /// reaching `least`, best first, as (code, share).
    pub fn best(&self, logits: &[f32]) -> Vec<(String, f32)> {
        let top = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if !top.is_finite() {
            return Vec::new();
        }
        let weights: Vec<f32> = logits.iter().map(|logit| (logit - top).exp()).collect();
        let sum: f32 = weights.iter().sum();
        let mut found: Vec<(String, f32)> = self
            .labels
            .iter()
            .zip(weights.iter().map(|weight| weight / sum))
            .filter(|(_, share)| *share >= self.least)
            .map(|(label, share)| (label.code.clone(), share))
            .collect();
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        found
    }

    /// The dictionary key of a label's name.
    pub fn text_of(&self, code: &str) -> Option<&str> {
        self.labels.iter().find(|label| label.code == code).map(|label| label.text.as_str())
    }
}

/// The list beside the application.
pub fn installed_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .map(|folder| folder.join(LIST))
        .find(|path| path.is_file())
        .ok_or_else(|| format!("{LIST} is not beside the application"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(text: &str) -> Result<Labels, String> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("labels.json");
        std::fs::write(&path, text).unwrap();
        Labels::read(&path)
    }

    #[test]
    fn a_part_gets_the_labels_reaching_least_best_first_and_a_bad_list_is_refused() {
        let labels = list(r#"{"version":1,"least":0.1,"labels":[
            {"code":"a","text":"shot.label.a","prompt":"a photo of a"},
            {"code":"b","text":"shot.label.b","prompt":"a photo of b"},
            {"code":"c","text":"shot.label.c","prompt":"a photo of c"}]}"#)
        .unwrap();
        // Shares e^2, e^0, e^4 of their sum: c 0.87, a 0.12, b 0.02 (under least).
        let best = labels.best(&[2.0, 0.0, 4.0]);
        assert_eq!(best.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["c", "a"]);
        assert!((best[0].1 - 0.867).abs() < 0.01);
        assert!(labels.best(&[f32::NEG_INFINITY; 3]).is_empty(), "no labels without logits");
        assert_eq!(labels.text_of("b"), Some("shot.label.b"));
        assert!(list(r#"{"version":1,"least":0.1,"labels":[]}"#).is_err());
        assert!(list(r#"{"version":1,"least":0.1,"labels":[{"code":"a","text":"t","prompt":"p"},{"code":"a","text":"t","prompt":"q"}]}"#).is_err());
    }

    #[test]
    fn the_installed_list_is_usable() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(LIST);
        let labels = Labels::read(&path).unwrap();
        for language in ["en", "hr"] {
            let texts = qnc_ui_text::Texts::for_language(language).unwrap();
            for label in &labels.labels {
                texts.get(&label.text).unwrap();
            }
        }
    }
}
