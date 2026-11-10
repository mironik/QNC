//! The shot-caption plug-in program (contract `qnc.shot-caption` 1): Florence-2 through the ONNX
//! Runtime library beside this program, loaded once and kept for the next job, below the player
//! and the desktop. It reads the pictures and the model it is given and writes nothing.

use std::path::{Path, PathBuf};

use qnc_florence2::Florence;
use qnc_shot_caption::CaptionsRequest;

/// Threads of one caption (the shots wait for the player; the desktop keeps the rest).
const THREADS: usize = 4;

#[derive(Default)]
struct Engine {
    model: Option<(PathBuf, Florence)>,
}

impl Engine {
    fn model(&mut self, folder: &Path) -> Result<&mut Florence, String> {
        if self.model.as_ref().map(|(loaded, _)| loaded.as_path()) != Some(folder) {
            self.model = Some((folder.to_path_buf(), Florence::load(folder, &runtime()?, THREADS)?));
        }
        Ok(&mut self.model.as_mut().expect("just loaded").1)
    }
}

/// The ONNX Runtime library beside this program.
fn runtime() -> Result<PathBuf, String> {
    let name = format!("{}onnxruntime{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.parent().map(|dir| dir.join(&name)).filter(|path| path.is_file()).ok_or_else(|| format!("{name} is not beside the program"))
}

impl qnc_shot_caption::Engine for Engine {
    fn captions(&mut self, request: &CaptionsRequest) -> Result<Vec<String>, String> {
        let pictures = request
            .pictures
            .iter()
            .map(|path| image::open(path).map(|image| image.into_rgb8()).map_err(|e| format!("{}: {e}", path.display())))
            .collect::<Result<Vec<_>, _>>()?;
        let model = self.model(&request.model)?;
        pictures.iter().map(|picture| model.caption(picture)).collect()
    }
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-shot-caption-florence");
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
    let mut engine = Engine::default();
    if let Err(error) = qnc_shot_caption::serve("qnc.shot-caption.florence2", env!("CARGO_PKG_VERSION"), "florence-2 (onnx runtime)", &mut engine) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
