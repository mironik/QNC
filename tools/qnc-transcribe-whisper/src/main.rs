//! Speech to text as a plug-in program (contract `qnc.transcribe` 1) with whisper.cpp: the
//! model is loaded once and kept for the next job; words get their times from the engine's
//! alignment of the decoder's attention to the sound (DTW). Built with the feature `vulkan`
//! it runs on the graphics card (Intel, AMD, NVIDIA) and the processor; with `whisper` only on
//! the processor; without either it answers that it has no engine.

use qnc_transcribe_jobs::Backend;

#[cfg(feature = "whisper")]
mod engine;

fn main() {
    #[cfg(feature = "whisper")]
    let result = {
        let mut engine = engine::Whisper::default();
        qnc_transcribe_jobs::serve(&mut engine, "qnc.transcribe.whisper", env!("CARGO_PKG_VERSION"), "whisper.cpp")
    };
    #[cfg(not(feature = "whisper"))]
    let result = qnc_transcribe_jobs::serve(&mut NoEngine, "qnc.transcribe.whisper", env!("CARGO_PKG_VERSION"), "none");
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

/// A build without whisper.cpp: it runs no backend and says so.
#[cfg(not(feature = "whisper"))]
struct NoEngine;

#[cfg(not(feature = "whisper"))]
impl qnc_transcribe_jobs::Engine for NoEngine {
    fn backends(&self) -> Vec<Backend> {
        Vec::new()
    }

    fn transcribe(
        &mut self,
        _: &qnc_transcribe_jobs::Transcribe,
        _: &std::sync::atomic::AtomicBool,
        _: &mut dyn FnMut(qnc_transcribe_jobs::Segment) -> Result<(), String>,
    ) -> Result<String, String> {
        Err("this program was built without whisper.cpp (feature whisper or vulkan)".into())
    }
}

#[allow(dead_code)]
fn backends() -> Vec<Backend> {
    if cfg!(feature = "vulkan") {
        vec![Backend::Vulkan, Backend::Cpu]
    } else if cfg!(feature = "whisper") {
        vec![Backend::Cpu]
    } else {
        Vec::new()
    }
}
