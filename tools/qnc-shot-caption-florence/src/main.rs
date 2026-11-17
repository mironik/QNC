//! The shot-caption plug-in program (contract `qnc.shot-caption` 2): Florence-2 through the ONNX
//! Runtime library beside this program, loaded once and kept for the next job, below the player
//! and the desktop. It is given the pictures themselves and the model's name, finds
//! the model in the AI catalog of the computer it runs on, and writes nothing.

use std::path::PathBuf;

use qnc_florence2::Florence;
use qnc_shot_caption::CaptionsRequest;

/// Threads of one caption (the shots wait for the player; the desktop keeps the rest).
const THREADS: usize = 4;

#[derive(Default)]
struct Engine {
    model: Option<(PathBuf, Florence)>,
}

impl Engine {
    fn model(&mut self, name: &str) -> Result<&mut Florence, String> {
        let folder = qnc_ai_catalog::installed_model(name)?;
        let folder = folder.as_path();
        if self.model.as_ref().map(|(loaded, _)| loaded.as_path()) != Some(folder) {
            let directml = qnc_ai_catalog::installed_shot_captioner()?.backend == "directml";
            self.model = Some((folder.to_path_buf(), Florence::load(folder, &runtime(directml)?, THREADS, directml)?));
        }
        Ok(&mut self.model.as_mut().expect("just loaded").1)
    }
}

/// The ONNX Runtime library beside this program; for DirectML its DirectML build in the folder
/// `directml` beside it (the one beside the program is sherpa-onnx's, for the processor), with
/// DirectML's own library loaded from there first (Windows keeps an older one of its own).
fn runtime(directml: bool) -> Result<PathBuf, String> {
    let name = format!("{}onnxruntime{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("the program has no folder")?;
    let dir = if directml { dir.join(DIRECTML_FOLDER) } else { dir.to_path_buf() };
    if directml {
        preload_directml(&dir)?;
    }
    Some(dir.join(&name)).filter(|path| path.is_file()).ok_or_else(|| format!("{name} is not in {}", dir.display()))
}

/// The folder of ONNX Runtime's DirectML build beside the program.
const DIRECTML_FOLDER: &str = "directml";

#[cfg(windows)]
fn preload_directml(dir: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let path = dir.join("DirectML.dll");
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let module = unsafe { windows_sys::Win32::System::LibraryLoader::LoadLibraryW(wide.as_ptr()) };
    if module.is_null() {
        return Err(format!("{} could not be loaded", path.display()));
    }
    Ok(())
}

#[cfg(not(windows))]
fn preload_directml(_dir: &std::path::Path) -> Result<(), String> {
    Err("DirectML is Windows' own".into())
}

impl qnc_shot_caption::Engine for Engine {
    fn captions(&mut self, request: &CaptionsRequest) -> Result<Vec<String>, String> {
        let pictures = request
            .pictures
            .iter()
            .enumerate()
            .map(|(n, picture)| image::load_from_memory(&picture.0).map(|image| image.into_rgb8()).map_err(|e| format!("picture {n}: {e}")))
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
