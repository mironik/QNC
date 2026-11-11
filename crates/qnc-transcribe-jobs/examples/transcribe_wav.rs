//! Measures one engine of the AI catalog on a 16 kHz mono WAV already made (nothing is read from a
//! card, nothing is written): the program beside this example, the engine (its id in the AI
//! catalog: the program finds its model, backend and threads) and the first SECONDS of the WAV.
//! Usage: transcribe_wav WAV ENGINE LANGUAGE [SECONDS]

use qnc_transcribe_jobs::{read_wav, TranscribeRequest, Transcriber};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: transcribe_wav WAV ENGINE LANGUAGE [SECONDS]".into());
    }
    let mut samples = read_wav(&PathBuf::from(&args[0]))?;
    if let Some(seconds) = args.get(3) {
        let seconds: f64 = seconds.parse().map_err(|_| "SECONDS is a number")?;
        samples.truncate((seconds * 16_000.0) as usize);
    }
    let duration = samples.len() as f64 / 16_000.0;
    // As the application gives it: at the level of speech.
    let job = TranscribeRequest::new(&args[1], &qnc_transcribe_jobs::at_speech_level(&samples, 16_000), &args[2])?;
    let program = std::env::var("QNC_TRANSCRIBE_PROGRAM").unwrap_or_else(|_| "qnc-transcribe-whisper".into());
    let transcriber = Transcriber::beside_application(&program)?;
    let never = AtomicBool::new(false);
    let began = Instant::now();
    let mut first = None;
    let transcript = transcriber.transcribe(&job, &never, &mut |segment| {
        first.get_or_insert(began.elapsed());
        println!("[{:7.2} - {:7.2}] {}", segment.start, segment.end, segment.text);
    });
    let transcript = transcript?;
    let took = began.elapsed().as_secs_f64();
    println!(
        "{duration:.1} s of sound, {} segments in {took:.1} s ({:.2}x real time), first after {:.1} s",
        transcript.segments.len(),
        duration / took,
        first.map(|f| f.as_secs_f64()).unwrap_or(took)
    );
    Ok(())
}
