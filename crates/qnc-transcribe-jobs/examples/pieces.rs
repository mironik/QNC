//! Prints the pieces the transcription queue gives the engine for a mono WAV (only read): blocks
//! of 35 s cut where a piece of 30 s would end, the sound over the floor, its pieces of 30 s.
//! The WAV is 16 kHz (the queue cuts the source rate; the places are the same). Usage: pieces WAV [FLOOR_DBFS]

use qnc_transcribe_jobs::{loud_spans, quiet_pieces, read_wav, WHISPER_RATE};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [wav, rest @ ..] = args.as_slice() else { return Err("usage: pieces WAV [FLOOR_DBFS]".into()) };
    let floor: f32 = rest.first().map(|f| f.parse().map_err(|_| "FLOOR_DBFS is a number")).transpose()?.unwrap_or(-42.0);
    let (samples, rate) = (read_wav(std::path::Path::new(wav))?, WHISPER_RATE as usize);
    let window = 35 * rate;
    let mut processed = 0;
    while processed < samples.len() {
        let last = samples.len() - processed <= window;
        let end = if last { samples.len() } else { processed + window };
        let block = &samples[processed..end];
        let block = if last { block } else { &block[..quiet_pieces(block, rate, 30)[0].end] };
        for span in loud_spans(block, rate, floor) {
            for piece in quiet_pieces(&block[span.clone()], rate, 30) {
                let at = |s: usize| (processed + span.start + s) as f64 / rate as f64;
                println!("{:7.2} - {:7.2}", at(piece.start), at(piece.end));
            }
        }
        processed += block.len();
    }
    Ok(())
}
