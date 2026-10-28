//! Speech to text as a plug-in program (contract `qnc.transcribe` 1) with NVIDIA Parakeet TDT
//! through sherpa-onnx, on the processor (user 2026-10-26: the trim zoom transcribes on the
//! processor and never takes the graphics card from the player). It is not autoregressive:
//! a few seconds of speech take a fraction of a second on two cores and it invents nothing
//! on silence. The sherpa-onnx C library is loaded from beside this program when the first
//! job comes; without it the program answers an error, nothing falls back. The model (a
//! folder of encoder, decoder, joiner and tokens) is loaded once and kept for the next job.

mod engine;
mod sherpa;

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-transcribe-parakeet");
    // Below the player and the desktop: when the processor is full the transcription waits,
    // so it may run while the user edits (user 2026-10-26: waiting for every touch of the
    // player made it not worth it).
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
    let mut engine = engine::Parakeet::default();
    if let Err(error) = qnc_transcribe_jobs::serve(&mut engine, "qnc.transcribe.parakeet", env!("CARGO_PKG_VERSION"), "sherpa-onnx parakeet-tdt") {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
