//! The letter-align plug-in contract (`contracts/plugins/letter-align.v2.json`, user
//! 2026-11-05: cutting by letters in the script): the application asks the times of the letters
//! of a known text in a 16 kHz WAV, the program answers them. Both sides are here, so the
//! program can be replaced without building QNC.
//!
//! Version 2 (docs/102 step 5): a request carries the speech itself (WAV bytes) and names the
//! model; the program finds the model in the AI catalog of the computer it runs on.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.letter-align";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const CONTRACT: &str = "qnc.letter-align";
pub const CONTRACT_VERSION: u32 = 2;
const ALIGN: &str = "align";

/// One letter and the seconds it is said in (none for a sign the model has not).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Letter {
    pub letter: char,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordLetters {
    pub letters: Vec<Letter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignRequest {
    /// The model's name (`qnc_ai_catalog::model_name`).
    pub model: String,
    /// The speech: a 16 kHz mono 16-bit WAV.
    pub wav: qnc_plugin_process::Bytes,
    pub words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlignAnswer {
    words: Vec<WordLetters>,
}

/// The application's side: one program, started when first asked, kept for the next job.
pub struct Aligner {
    plugin: qnc_plugin_process::Plugin,
}

impl Aligner {
    pub fn new(executable: &Path) -> Self {
        let spec = qnc_plugin_process::PluginSpec {
            executable: executable.to_path_buf(),
            contract: CONTRACT.into(),
            version: CONTRACT_VERSION,
            timeout: Duration::from_secs(120),
            remote: None,
        };
        Self { plugin: qnc_plugin_process::Plugin::new(spec) }
    }

    /// The program `program` where this computer runs its AI (`qnc-ai-place`): beside the
    /// application, or on its AI server.
    pub fn for_program(program: &str) -> Result<Self, String> {
        Ok(Self { plugin: qnc_plugin_process::Plugin::new(qnc_ai_place::spec(program, CONTRACT, CONTRACT_VERSION, Duration::from_secs(120))?) })
    }

    pub fn align(&self, request: &AlignRequest) -> Result<Vec<WordLetters>, String> {
        let body = serde_json::to_value(request).map_err(|error| error.to_string())?;
        let answer: AlignAnswer = serde_json::from_value(self.plugin.call(ALIGN, body)?).map_err(|error| format!("an answer outside the contract: {error}"))?;
        if answer.words.len() != request.words.len() {
            return Err("the letters of another number of words".into());
        }
        Ok(answer.words)
    }
}

/// The program's side: answers every request with `align` until stdin ends.
pub fn serve(plugin_id: &str, plugin_version: &str, engine: &str, mut align: impl FnMut(&AlignRequest) -> Result<Vec<WordLetters>, String>) -> Result<(), String> {
    let ready = qnc_plugin_process::Ready {
        contract: CONTRACT.into(),
        version: CONTRACT_VERSION,
        plugin_id: plugin_id.into(),
        plugin_version: plugin_version.into(),
        about: serde_json::json!({ "engine": engine }),
    };
    qnc_plugin_process::serve(ready, |op, body| {
        if op != ALIGN {
            return Err(format!("unknown operation '{op}'"));
        }
        let request: AlignRequest = serde_json::from_value(body).map_err(|error| format!("a request outside the contract: {error}"))?;
        let words = align(&request)?;
        serde_json::to_value(AlignAnswer { words }).map_err(|error| error.to_string())
    })
}
