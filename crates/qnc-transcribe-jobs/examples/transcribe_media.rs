//! Acceptance of the transcription contract on a medium that is only read: its sound through the
//! decoder catalog's executable, the engine the AI catalog selects, the time each step takes and
//! the words with their times. Writes only a WAV into an empty temporary folder it removes.
//! Usage: transcribe_media MEDIA CONTAINER STREAM RATE LANGUAGE [SECONDS]
//! (e.g. `card/Clip/A.MXF mxf 1 48000 hr 120`: the second stream, the first 120 s)

use qnc_transcribe_jobs::{to_whisper_rate, write_wav, Backend, Transcribe, Transcriber};
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 5 {
        return Err("usage: transcribe_media MEDIA CONTAINER STREAM RATE LANGUAGE [SECONDS]".into());
    }
    let media = PathBuf::from(&args[0]);
    let stream: u32 = args[2].parse().map_err(|_| "STREAM is a number")?;
    let rate: u32 = args[3].parse().map_err(|_| "RATE is a number")?;
    let seconds: Option<f64> = args.get(5).map(|s| s.parse().map_err(|_| "SECONDS is a number")).transpose()?;
    let engine = qnc_ai_catalog::installed_transcriber()?;
    let decoder = qnc_decoder_catalog::installed_deployment().map_err(|e| e.to_string())?;
    println!("engine {} ({}), decoder {}", engine.id, engine.backend, decoder.executable.display());

    let started = Instant::now();
    let never = AtomicBool::new(false);
    let mut sound = Vec::new();
    let most = seconds.map(|s| (s * f64::from(rate)) as usize);
    qnc_ffmpeg_decode::audio_streams_with_cancel(&decoder.executable, &media, &args[1], &[stream], 1, &never, &mut |samples| {
        sound.extend_from_slice(samples);
        if most.is_some_and(|most| sound.len() >= most) {
            return Err("enough".into());
        }
        Ok(())
    })
    .or_else(|e| if e.contains("enough") { Ok(()) } else { Err(e) })?;
    if let Some(most) = most {
        sound.truncate(most);
    }
    let duration = sound.len() as f64 / f64::from(rate);
    let read = started.elapsed();
    let sound = to_whisper_rate(&sound, rate)?;
    let folder = std::env::temp_dir().join(format!("qnc-transcribe-{}", std::process::id()));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let wav = folder.join("sound.wav");
    write_wav(&wav, &sound)?;
    if let Some(keep) = std::env::var_os("QNC_KEEP_WAV") {
        std::fs::copy(&wav, keep).map_err(|e| e.to_string())?;
    }
    println!("sound {duration:.1} s read in {:.1} s, 16 kHz in {:.1} s", read.as_secs_f64(), started.elapsed().as_secs_f64());

    let transcriber = Transcriber::beside_application(&engine.program)?;
    let job = Transcribe {
        model: engine.model,
        vad_model: engine.vad_model,
        backend: if engine.backend == "vulkan" { Backend::Vulkan } else { Backend::Cpu },
        wav,
        language: args[4].clone(),
        threads: engine.threads,
    };
    let began = Instant::now();
    let mut first = None;
    let transcript = transcriber.transcribe(&job, &never, &mut |segment| {
        first.get_or_insert(began.elapsed());
        println!("[{:7.2} - {:7.2}] {}", segment.start, segment.end, segment.text);
        let words: Vec<String> = segment.words.iter().map(|w| format!("{}@{:.2}-{:.2}", w.text, w.start, w.end)).collect();
        println!("            {}", words.join(" "));
    });
    let _ = std::fs::remove_dir_all(&folder);
    let transcript = transcript?;
    let took = began.elapsed().as_secs_f64();
    println!(
        "{} segments, language {}, engine {}; transcribed in {took:.1} s ({:.2}x real time), first segment after {:.1} s",
        transcript.segments.len(),
        transcript.language,
        transcript.engine,
        duration / took,
        first.map(|f| f.as_secs_f64()).unwrap_or(took)
    );
    Ok(())
}
