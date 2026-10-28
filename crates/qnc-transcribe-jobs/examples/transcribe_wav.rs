//! Measures one engine setting on a 16 kHz mono WAV already made (nothing is read from a card,
//! nothing is written): the program beside this example, a model, a backend, a thread count and
//! the first SECONDS of the WAV. Usage:
//! transcribe_wav WAV MODEL VAD BACKEND THREADS LANGUAGE [SECONDS]  (BACKEND cpu or vulkan)

use qnc_transcribe_jobs::{read_wav, write_wav, Backend, Transcribe, Transcriber};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 6 {
        return Err("usage: transcribe_wav WAV MODEL VAD BACKEND THREADS LANGUAGE [SECONDS]".into());
    }
    let mut samples = read_wav(&PathBuf::from(&args[0]))?;
    if let Some(seconds) = args.get(6) {
        let seconds: f64 = seconds.parse().map_err(|_| "SECONDS is a number")?;
        samples.truncate((seconds * 16_000.0) as usize);
    }
    let duration = samples.len() as f64 / 16_000.0;
    let folder = std::env::temp_dir().join(format!("qnc-transcribe-wav-{}", std::process::id()));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let wav = folder.join("sound.wav");
    write_wav(&wav, &samples)?;
    let job = Transcribe {
        model: PathBuf::from(&args[1]),
        vad_model: Some(PathBuf::from(&args[2])),
        backend: if args[3] == "vulkan" { Backend::Vulkan } else { Backend::Cpu },
        wav,
        language: args[5].clone(),
        threads: args[4].parse().map_err(|_| "THREADS is a number")?,
    };
    let program = std::env::var("QNC_TRANSCRIBE_PROGRAM").unwrap_or_else(|_| "qnc-transcribe-whisper".into());
    let transcriber = Transcriber::beside_application(&program)?;
    let never = AtomicBool::new(false);
    let began = Instant::now();
    let mut first = None;
    let transcript = transcriber.transcribe(&job, &never, &mut |segment| {
        first.get_or_insert(began.elapsed());
        println!("[{:7.2} - {:7.2}] {}", segment.start, segment.end, segment.text);
    });
    let _ = std::fs::remove_dir_all(&folder);
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
