//! The AI store (contract `qnc.ai-store` 1, docs/102 step 7b, user 2026-11-11: "common to all, we
//! are building a base to train on"): the captions a model wrote of the shots and the groups the
//! users gave shots are kept on the AI server, one store for every QNC that uses the server, so
//! what one editor corrects teaches all. Without a server they stay in the databases of the
//! computer (`qnc-shot-captions`, `qnc-shot-examples`), which use this client only while their
//! computer names a server. The store program (`qnc-ai-store`) runs beside the AI server, which
//! starts it like any plug-in, over TLS and with the token.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use qnc_plugin_process::{Plugin, PluginSpec, Ready};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.ai-store";
pub const CONTRACT: &str = "qnc.ai-store";
pub const CONTRACT_VERSION: u32 = 1;
/// The store program, beside the AI server.
pub const PROGRAM: &str = "qnc-ai-store";

const CAPTIONED: &str = "captioned";
const READ_CAPTIONS: &str = "read_captions";
const SAVE_CAPTION: &str = "save_caption";
const READ_EXAMPLES: &str = "read_examples";
const SAVE_EXAMPLE: &str = "save_example";

/// A shot's caption: its frames in its clip and the sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caption {
    pub from: u64,
    pub to: u64,
    pub text: String,
}

/// One corrected shot of a model: what it is, the group given and its vector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    pub key: String,
    pub label: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Captioned {
    model: String,
    project_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadCaptions {
    model: String,
    project_uri: String,
    clip_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveCaption {
    model: String,
    shot_key: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadExamples {
    model: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveExample {
    model: String,
    example: Example,
}

/// The store of the AI server `server`: one connection a server, kept for the next call.
pub fn on(server: &qnc_ai_place::AiServer) -> Arc<Plugin> {
    static KEPT: OnceLock<Mutex<HashMap<String, Arc<Plugin>>>> = OnceLock::new();
    let mut kept = KEPT.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    kept.entry(server.address.clone())
        .or_insert_with(|| {
            let remote: qnc_plugin_process::Remote = Arc::new(server.clone());
            Arc::new(Plugin::new(PluginSpec { executable: PROGRAM.into(), contract: CONTRACT.into(), version: CONTRACT_VERSION, timeout: Duration::from_secs(30), remote: Some(remote) }))
        })
        .clone()
}

fn call<T: serde::de::DeserializeOwned>(store: &Plugin, op: &str, body: impl Serialize) -> Result<T, String> {
    let answer = store.call(op, serde_json::to_value(body).map_err(|e| e.to_string())?)?;
    serde_json::from_value(answer).map_err(|e| format!("an answer outside the AI store contract: {e}"))
}

/// The keys of the shots of `project_uri` with a caption of `model`.
pub fn captioned(store: &Plugin, model: &str, project_uri: &str) -> Result<Vec<String>, String> {
    call(store, CAPTIONED, Captioned { model: model.into(), project_uri: project_uri.into() })
}

/// The captions of `model` of a clip of `project_uri`.
pub fn read_captions(store: &Plugin, model: &str, project_uri: &str, clip_id: &str) -> Result<Vec<Caption>, String> {
    call(store, READ_CAPTIONS, ReadCaptions { model: model.into(), project_uri: project_uri.into(), clip_id: clip_id.into() })
}

pub fn save_caption(store: &Plugin, model: &str, shot_key: &str, text: &str) -> Result<(), String> {
    call::<Value>(store, SAVE_CAPTION, SaveCaption { model: model.into(), shot_key: shot_key.into(), text: text.into() }).map(drop)
}

/// The corrections of `model`, the oldest first.
pub fn read_examples(store: &Plugin, model: &str) -> Result<Vec<Example>, String> {
    call(store, READ_EXAMPLES, ReadExamples { model: model.into() })
}

pub fn save_example(store: &Plugin, model: &str, example: Example) -> Result<(), String> {
    call::<Value>(store, SAVE_EXAMPLE, SaveExample { model: model.into(), example }).map(drop)
}

/// What the store program keeps, in the databases of its computer.
pub trait Store {
    fn captioned(&mut self, model: &str, project_uri: &str) -> Result<Vec<String>, String>;
    fn read_captions(&mut self, model: &str, project_uri: &str, clip_id: &str) -> Result<Vec<Caption>, String>;
    fn save_caption(&mut self, model: &str, shot_key: &str, text: &str) -> Result<(), String>;
    fn read_examples(&mut self, model: &str) -> Result<Vec<Example>, String>;
    fn save_example(&mut self, model: &str, example: Example) -> Result<(), String>;
}

/// The store program's side: answers every request until its input ends.
pub fn serve(store: &mut dyn Store) -> Result<(), String> {
    let ready = Ready { contract: CONTRACT.into(), version: CONTRACT_VERSION, plugin_id: "qnc.ai-store".into(), plugin_version: env!("CARGO_PKG_VERSION").into(), about: Value::Null };
    fn parse<T: serde::de::DeserializeOwned>(body: Value) -> Result<T, String> {
        serde_json::from_value(body).map_err(|e| format!("a request outside the AI store contract: {e}"))
    }
    fn to<T: Serialize>(value: T) -> Result<Value, String> {
        serde_json::to_value(value).map_err(|e| e.to_string())
    }
    qnc_plugin_process::serve(ready, |op, body| match op {
        CAPTIONED => parse::<Captioned>(body).and_then(|asked| to(store.captioned(&asked.model, &asked.project_uri)?)),
        READ_CAPTIONS => parse::<ReadCaptions>(body).and_then(|asked| to(store.read_captions(&asked.model, &asked.project_uri, &asked.clip_id)?)),
        SAVE_CAPTION => parse::<SaveCaption>(body).and_then(|asked| store.save_caption(&asked.model, &asked.shot_key, &asked.text).map(|()| Value::Null)),
        READ_EXAMPLES => parse::<ReadExamples>(body).and_then(|asked| to(store.read_examples(&asked.model)?)),
        SAVE_EXAMPLE => parse::<SaveExample>(body).and_then(|asked| store.save_example(&asked.model, asked.example).map(|()| Value::Null)),
        other => Err(format!("unknown AI store operation '{other}'")),
    })
}
