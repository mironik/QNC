//! The text-translate plug-in contract (`contracts/plugins/text-translate.v1.json`, user
//! 2026-11-11: "the OFF into English", so that SigLIP 2 finds the pictures and the captions of
//! its words): the application gives texts, the program answers each in English. Both sides are
//! here, so the program and its model can be replaced without building QNC.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.text-translate";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const CONTRACT: &str = "qnc.text-translate";
pub const CONTRACT_VERSION: u32 = 1;
const TRANSLATE: &str = "translate";

/// The most texts one request carries (a sentence takes half a second).
pub const MOST_AT_ONCE: usize = 16;

/// The longest text the program takes (a few sentences of an OFF).
pub const LONGEST: usize = 4000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslateRequest {
    pub model: PathBuf,
    pub texts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Translated {
    texts: Vec<String>,
}

/// The application's side: one program, started when first asked, kept for the next job.
pub struct TranslateClient {
    plugin: qnc_plugin_process::Plugin,
}

impl TranslateClient {
    pub fn new(executable: &Path) -> Self {
        let spec = qnc_plugin_process::PluginSpec {
            executable: executable.to_path_buf(),
            contract: CONTRACT.into(),
            version: CONTRACT_VERSION,
            timeout: Duration::from_secs(120),
        };
        Self { plugin: qnc_plugin_process::Plugin::new(spec) }
    }

    /// Each text in English, in the order given; a longer list is asked in parts.
    pub fn translate(&self, request: &TranslateRequest) -> Result<Vec<String>, String> {
        let mut all = Vec::with_capacity(request.texts.len());
        for texts in request.texts.chunks(MOST_AT_ONCE) {
            let body = serde_json::to_value(TranslateRequest { model: request.model.clone(), texts: texts.to_vec() }).map_err(|error| error.to_string())?;
            let answer: Translated = serde_json::from_value(self.plugin.call(TRANSLATE, body)?).map_err(|error| format!("an answer outside the contract: {error}"))?;
            if answer.texts.len() != texts.len() {
                return Err("the translations of another number of texts".into());
            }
            all.extend(answer.texts);
        }
        Ok(all)
    }
}

/// What the program does for a request.
pub trait Engine {
    fn translate(&mut self, request: &TranslateRequest) -> Result<Vec<String>, String>;
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
        TRANSLATE => {
            let request: TranslateRequest = serde_json::from_value(body).map_err(|error| format!("a request outside the contract: {error}"))?;
            if request.texts.len() > MOST_AT_ONCE {
                return Err(format!("more than {MOST_AT_ONCE} texts at once"));
            }
            if request.texts.iter().any(|text| text.len() > LONGEST) {
                return Err(format!("a text longer than {LONGEST} bytes"));
            }
            serde_json::to_value(Translated { texts: engine.translate(&request)? }).map_err(|error| error.to_string())
        }
        other => Err(format!("unknown operation '{other}'")),
    })
}
