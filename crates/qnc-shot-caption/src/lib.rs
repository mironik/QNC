//! The shot-caption plug-in contract (`contracts/plugins/shot-caption.v2.json`, user 2026-11-10:
//! Florence-2 as the second AI engine): the application gives pictures, the program answers a
//! sentence of what each shows. Both sides are here, so the program and its model can be
//! replaced without building QNC.
//!
//! Version 2 (docs/102 step 5): a request carries the pictures themselves (JPEG bytes) and names
//! the model; the program finds the model in the AI catalog of the computer it runs on.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.shot-caption";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const CONTRACT: &str = "qnc.shot-caption";
pub const CONTRACT_VERSION: u32 = 2;
const CAPTIONS: &str = "captions";

/// The most pictures one request carries (a caption takes seconds).
pub const MOST_AT_ONCE: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionsRequest {
    /// The model's name (`qnc_ai_catalog::model_name`).
    pub model: String,
    /// JPEG pictures.
    pub pictures: Vec<qnc_plugin_process::Bytes>,
}

impl CaptionsRequest {
    /// The request of JPEG `files` of the application for `model` (a path of its catalog).
    pub fn of_files(model: &Path, files: &[PathBuf]) -> Result<Self, String> {
        let pictures = files.iter().map(|file| std::fs::read(file).map(qnc_plugin_process::Bytes).map_err(|error| format!("{}: {error}", file.display()))).collect::<Result<_, _>>()?;
        Ok(Self { model: qnc_ai_catalog::model_name(model), pictures })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Captions {
    captions: Vec<String>,
}

/// The application's side: one program, started when first asked, kept for the next job.
pub struct CaptionClient {
    plugin: qnc_plugin_process::Plugin,
}

impl CaptionClient {
    pub fn new(executable: &Path) -> Self {
        let spec = qnc_plugin_process::PluginSpec {
            executable: executable.to_path_buf(),
            contract: CONTRACT.into(),
            version: CONTRACT_VERSION,
            timeout: Duration::from_secs(300),
        };
        Self { plugin: qnc_plugin_process::Plugin::new(spec) }
    }

    /// The caption of each picture, in the order given; a longer list is asked in parts.
    pub fn captions(&self, request: &CaptionsRequest) -> Result<Vec<String>, String> {
        let mut all = Vec::with_capacity(request.pictures.len());
        for pictures in request.pictures.chunks(MOST_AT_ONCE) {
            let body = serde_json::to_value(CaptionsRequest { model: request.model.clone(), pictures: pictures.to_vec() }).map_err(|error| error.to_string())?;
            let answer: Captions = serde_json::from_value(self.plugin.call(CAPTIONS, body)?).map_err(|error| format!("an answer outside the contract: {error}"))?;
            if answer.captions.len() != pictures.len() {
                return Err("the captions of another number of pictures".into());
            }
            all.extend(answer.captions);
        }
        Ok(all)
    }
}

/// What the program does for a request.
pub trait Engine {
    fn captions(&mut self, request: &CaptionsRequest) -> Result<Vec<String>, String>;
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
        CAPTIONS => {
            let request: CaptionsRequest = serde_json::from_value(body).map_err(|error| format!("a request outside the contract: {error}"))?;
            if request.pictures.len() > MOST_AT_ONCE {
                return Err(format!("more than {MOST_AT_ONCE} pictures at once"));
            }
            serde_json::to_value(Captions { captions: engine.captions(&request)? }).map_err(|error| error.to_string())
        }
        other => Err(format!("unknown operation '{other}'")),
    })
}
