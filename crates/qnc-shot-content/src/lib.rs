//! The shot-content plug-in contract (`contracts/plugins/shot-content.v2.json`, user 2026-11-10:
//! sorting the B-roll by what its pictures show, offline): the application gives pictures and
//! texts, the program answers their vectors in one space and how a picture and a text are
//! scored. Both sides are here, so the program and its model can be replaced without building
//! QNC.
//!
//! Version 2 (docs/102 step 5): a request carries the pictures themselves (JPEG bytes) and names
//! the model; the program finds the model in the AI catalog of the computer it runs on, so it may
//! run on another one.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.shot-content";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const CONTRACT: &str = "qnc.shot-content";
pub const CONTRACT_VERSION: u32 = 2;
const PICTURES: &str = "pictures";
const TEXTS: &str = "texts";

/// The most pictures or texts one request carries.
pub const MOST_AT_ONCE: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PicturesRequest {
    /// The model's name (`qnc_ai_catalog::model_name`).
    pub model: String,
    /// JPEG pictures.
    pub pictures: Vec<qnc_plugin_process::Bytes>,
}

impl PicturesRequest {
    /// The request of JPEG `files` of the application for `model` (a path of its catalog).
    pub fn of_files(model: &Path, files: &[PathBuf]) -> Result<Self, String> {
        let pictures = files.iter().map(|file| std::fs::read(file).map(qnc_plugin_process::Bytes).map_err(|error| format!("{}: {error}", file.display()))).collect::<Result<_, _>>()?;
        Ok(Self { model: qnc_ai_catalog::model_name(model), pictures })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextsRequest {
    /// The model's name (`qnc_ai_catalog::model_name`).
    pub model: String,
    pub texts: Vec<String>,
}

impl TextsRequest {
    pub fn new(model: &Path, texts: Vec<String>) -> Self {
        Self { model: qnc_ai_catalog::model_name(model), texts }
    }
}

/// The vectors of texts and how a picture and a text are scored: the sigmoid of
/// `cosine * scale + bias`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Texts {
    pub vectors: Vec<Vec<f32>>,
    pub scale: f32,
    pub bias: f32,
}

impl Texts {
    /// How well text `text` describes a picture whose vector is `picture` (0 to 1).
    pub fn score(&self, picture: &[f32], text: usize) -> f32 {
        1.0 / (1.0 + (-self.logit(picture, text)).exp())
    }

    /// The model's logit of text `text` for a picture: `cosine * scale + bias` (none for a text
    /// it was not given: minus infinity).
    pub fn logit(&self, picture: &[f32], text: usize) -> f32 {
        let Some(vector) = self.vectors.get(text) else { return f32::NEG_INFINITY };
        let cosine: f32 = picture.iter().zip(vector).map(|(a, b)| a * b).sum();
        cosine * self.scale + self.bias
    }

    /// The logits of every text for a picture, in the order of the texts.
    pub fn logits(&self, picture: &[f32]) -> Vec<f32> {
        (0..self.vectors.len()).map(|text| self.logit(picture, text)).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Vectors {
    vectors: Vec<Vec<f32>>,
}

/// The application's side: one program, started when first asked, kept for the next job.
pub struct ContentClient {
    plugin: qnc_plugin_process::Plugin,
}

impl ContentClient {
    pub fn new(executable: &Path) -> Self {
        let spec = qnc_plugin_process::PluginSpec {
            executable: executable.to_path_buf(),
            contract: CONTRACT.into(),
            version: CONTRACT_VERSION,
            timeout: Duration::from_secs(300),
        };
        Self { plugin: qnc_plugin_process::Plugin::new(spec) }
    }

    /// The vector of each picture (unit length), in the order given.
    pub fn pictures(&self, request: &PicturesRequest) -> Result<Vec<Vec<f32>>, String> {
        if request.pictures.len() > MOST_AT_ONCE {
            return Err(format!("more than {MOST_AT_ONCE} pictures at once"));
        }
        let body = serde_json::to_value(request).map_err(|error| error.to_string())?;
        let answer: Vectors = serde_json::from_value(self.plugin.call(PICTURES, body)?).map_err(|error| format!("an answer outside the contract: {error}"))?;
        if answer.vectors.len() != request.pictures.len() {
            return Err("the vectors of another number of pictures".into());
        }
        Ok(answer.vectors)
    }

    /// The vectors of texts and how they score against pictures.
    /// A longer list is asked in parts of `MOST_AT_ONCE` (live 2026-11-10: a list of 39 labels
    /// stopped the whole look at the shots).
    pub fn texts(&self, request: &TextsRequest) -> Result<Texts, String> {
        if request.texts.len() > MOST_AT_ONCE {
            let mut all = Texts { vectors: Vec::with_capacity(request.texts.len()), scale: 0.0, bias: 0.0 };
            for texts in request.texts.chunks(MOST_AT_ONCE) {
                let part = self.texts(&TextsRequest { model: request.model.clone(), texts: texts.to_vec() })?;
                (all.scale, all.bias) = (part.scale, part.bias);
                all.vectors.extend(part.vectors);
            }
            return Ok(all);
        }
        let body = serde_json::to_value(request).map_err(|error| error.to_string())?;
        let answer: Texts = serde_json::from_value(self.plugin.call(TEXTS, body)?).map_err(|error| format!("an answer outside the contract: {error}"))?;
        if answer.vectors.len() != request.texts.len() {
            return Err("the vectors of another number of texts".into());
        }
        Ok(answer)
    }
}

/// What the program does for the two requests.
pub trait Engine {
    fn pictures(&mut self, request: &PicturesRequest) -> Result<Vec<Vec<f32>>, String>;
    fn texts(&mut self, request: &TextsRequest) -> Result<Texts, String>;
}

/// The program's side: answers every request until stdin ends.
pub fn serve(plugin_id: &str, plugin_version: &str, engine_name: &str, engine: &mut dyn Engine) -> Result<(), String> {
    let ready = qnc_plugin_process::Ready {
        contract: CONTRACT.into(),
        version: CONTRACT_VERSION,
        plugin_id: plugin_id.into(),
        plugin_version: plugin_version.into(),
        about: serde_json::json!({ "engine": engine_name }),
    };
    qnc_plugin_process::serve(ready, |op, body| match op {
        PICTURES => {
            let request: PicturesRequest = serde_json::from_value(body).map_err(|error| format!("a request outside the contract: {error}"))?;
            serde_json::to_value(Vectors { vectors: engine.pictures(&request)? }).map_err(|error| error.to_string())
        }
        TEXTS => {
            let request: TextsRequest = serde_json::from_value(body).map_err(|error| format!("a request outside the contract: {error}"))?;
            serde_json::to_value(engine.texts(&request)?).map_err(|error| error.to_string())
        }
        other => Err(format!("unknown operation '{other}'")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_scores_a_picture_by_the_sigmoid_of_its_scaled_cosine() {
        let texts = Texts { vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]], scale: 10.0, bias: -5.0 };
        assert!((texts.score(&[1.0, 0.0], 0) - 1.0 / (1.0 + (-5.0f32).exp())).abs() < 1e-6);
        assert!(texts.score(&[1.0, 0.0], 1) < 0.01);
        assert_eq!(texts.score(&[1.0, 0.0], 9), 0.0, "no such text");
    }
}
