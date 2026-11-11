//! The shot-content plug-in program (contract `qnc.shot-content` 2): a SigLIP model in pure
//! Rust, loaded once and kept for the next job, below the player and the desktop. It is given the
//! pictures themselves and the model's name, finds the model in the AI catalog of the computer it
//! runs on, and writes nothing.

use std::path::PathBuf;

use qnc_shot_content::{PicturesRequest, Texts, TextsRequest};
use qnc_siglip::Siglip;

#[derive(Default)]
struct Engine {
    model: Option<(PathBuf, Siglip)>,
}

impl Engine {
    fn model(&mut self, name: &str) -> Result<&Siglip, String> {
        let folder = qnc_ai_catalog::installed_model(name)?;
        let folder = folder.as_path();
        if self.model.as_ref().map(|(loaded, _)| loaded.as_path()) != Some(folder) {
            self.model = Some((folder.to_path_buf(), Siglip::load(folder)?));
        }
        Ok(&self.model.as_ref().expect("just loaded").1)
    }
}

impl qnc_shot_content::Engine for Engine {
    fn pictures(&mut self, request: &PicturesRequest) -> Result<Vec<Vec<f32>>, String> {
        let pictures = request
            .pictures
            .iter()
            .enumerate()
            .map(|(n, picture)| image::load_from_memory(&picture.0).map(|image| image.into_rgb8()).map_err(|e| format!("picture {n}: {e}")))
            .collect::<Result<Vec<_>, _>>()?;
        self.model(&request.model)?.pictures(&pictures)
    }

    fn texts(&mut self, request: &TextsRequest) -> Result<Texts, String> {
        let model = self.model(&request.model)?;
        let (scale, bias) = model.scoring();
        Ok(Texts { vectors: model.texts(&request.texts)?, scale, bias })
    }
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-shot-content-siglip");
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
    let mut engine = Engine::default();
    if let Err(error) = qnc_shot_content::serve("qnc.shot-content.siglip", env!("CARGO_PKG_VERSION"), "siglip (candle)", &mut engine) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
