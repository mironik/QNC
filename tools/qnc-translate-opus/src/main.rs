//! The text-translate plug-in program (contract `qnc.text-translate` 2): a Marian model of
//! OPUS-MT in pure Rust (candle), loaded once and kept for the next job, below the player and the
//! desktop. It finds the model it is asked by name in the AI catalog of the
//! computer it runs on, and writes nothing.

use std::path::PathBuf;

use qnc_text_translate::TranslateRequest;
use qnc_translate::Translator;

#[derive(Default)]
struct Engine {
    model: Option<(PathBuf, Translator)>,
}

impl Engine {
    fn model(&mut self, name: &str) -> Result<&mut Translator, String> {
        let folder = qnc_ai_catalog::installed_model(name)?;
        let folder = folder.as_path();
        if self.model.as_ref().map(|(loaded, _)| loaded.as_path()) != Some(folder) {
            self.model = Some((folder.to_path_buf(), Translator::load(folder)?));
        }
        Ok(&mut self.model.as_mut().expect("just loaded").1)
    }
}

impl qnc_text_translate::Engine for Engine {
    fn translate(&mut self, request: &TranslateRequest) -> Result<Vec<String>, String> {
        let model = self.model(&request.model)?;
        request.texts.iter().map(|text| model.translate(text)).collect()
    }
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-translate-opus");
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
    let mut engine = Engine::default();
    if let Err(error) = qnc_text_translate::serve("qnc.text-translate.opus-mt", env!("CARGO_PKG_VERSION"), "opus-mt (marian, candle)", &mut engine) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
