//! The AI store program (contract `qnc.ai-store` 1, docs/102 step 7b, user 2026-11-11: "common to
//! all, we are building a base to train on"). The AI server starts it like any plug-in, over TLS
//! and with the token; it keeps the captions of the shots and the users' corrections of their
//! groups in the databases of the server's computer, the same for every QNC that uses the server.

use std::path::PathBuf;

use qnc_ai_store::{Caption, Example, Store};

struct Here {
    root: PathBuf,
}

impl Store for Here {
    fn captioned(&mut self, model: &str, project_uri: &str) -> Result<Vec<String>, String> {
        qnc_shot_captions::captioned_here(&self.root, model, project_uri)
    }

    fn read_captions(&mut self, model: &str, project_uri: &str, clip_id: &str) -> Result<Vec<Caption>, String> {
        Ok(qnc_shot_captions::read_clip_here(&self.root, model, project_uri, clip_id)?
            .into_iter()
            .map(|caption| Caption { from: caption.from, to: caption.to, text: caption.text })
            .collect())
    }

    fn save_caption(&mut self, model: &str, shot_key: &str, text: &str) -> Result<(), String> {
        qnc_shot_captions::save_here(&self.root, model, shot_key, text)
    }

    fn read_examples(&mut self, model: &str) -> Result<Vec<Example>, String> {
        Ok(qnc_shot_examples::read_here(&self.root, model)?.into_iter().map(|example| Example { key: example.key, label: example.label, vector: example.vector }).collect())
    }

    fn save_example(&mut self, model: &str, example: Example) -> Result<(), String> {
        qnc_shot_examples::save_here(&self.root, &qnc_shot_examples::Example { model: model.into(), key: example.key, label: example.label, vector: example.vector })
    }
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-ai-store");
    let Some(root) = qnc_dev_diagnostics::resolve_qnc_root() else {
        eprintln!("qnc-ai-store: no QNC root");
        std::process::exit(1);
    };
    if let Err(error) = qnc_ai_store::serve(&mut Here { root }) {
        eprintln!("qnc-ai-store: {error}");
        std::process::exit(1);
    }
}
