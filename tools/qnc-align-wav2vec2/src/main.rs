//! The letter-align plug-in program (contract `qnc.letter-align` 1): a wav2vec2 CTC model in
//! pure Rust, loaded once and kept for the next job, below the player and the desktop.

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-align-wav2vec2");
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
    let mut engine = qnc_align_wav2vec2::Engine::default();
    let served = qnc_letter_align::serve("qnc.letter-align.wav2vec2", env!("CARGO_PKG_VERSION"), "wav2vec2 ctc (candle)", |request| {
        let samples = qnc_transcribe_jobs::read_wav(&request.wav)?;
        engine.align(&request.model, &samples, &request.words)
    });
    if let Err(error) = served {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
