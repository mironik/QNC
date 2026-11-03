//! Prints the pieces the transcription queue gives the engine for a mono WAV (only read): blocks
//! of 35 s cut where a piece of 30 s would end, the sound over the clip's floor (the lowest noise
//! met so far plus 6 dB, as the queue does, or a fixed FLOOR_DBFS), its pieces of 30 s.
//! The WAV is 16 kHz (the queue cuts the source rate; the places are the same). Usage: pieces WAV [FLOOR_DBFS]

use qnc_transcribe_jobs::{clip_floor_dbfs, loud_spans, noise_dbfs, quiet_pieces, read_wav, FLOOR_CEILING_DBFS, WHISPER_RATE};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [wav, rest @ ..] = args.as_slice() else { return Err("usage: pieces WAV [FLOOR_DBFS]".into()) };
    let fixed: Option<f32> = rest.first().map(|f| f.parse().map_err(|_| "FLOOR_DBFS is a number")).transpose()?;
    let (samples, rate) = (read_wav(std::path::Path::new(wav))?, WHISPER_RATE as usize);
    let window = 35 * rate;
    let (mut processed, mut noise, mut heard) = (0, f32::INFINITY, 0usize);
    while processed < samples.len() {
        let last = samples.len() - processed <= window;
        let end = if last { samples.len() } else { processed + window };
        let block = &samples[processed..end];
        let block = if last { block } else { &block[..quiet_pieces(block, rate, 30)[0].end] };
        noise = noise_dbfs(block, rate).map_or(noise, |n| noise.min(n));
        let floor = fixed.unwrap_or(if noise.is_finite() { clip_floor_dbfs(noise) } else { FLOOR_CEILING_DBFS });
        for span in loud_spans(block, rate, floor) {
            heard += span.len();
            for piece in quiet_pieces(&block[span.clone()], rate, 30) {
                let at = |s: usize| (processed + span.start + s) as f64 / rate as f64;
                println!("{:7.2} - {:7.2}  floor {floor:.1}", at(piece.start), at(piece.end));
            }
        }
        processed += block.len();
    }
    println!("{:.1} s of {:.1} s heard", heard as f64 / rate as f64, samples.len() as f64 / rate as f64);
    Ok(())
}
