//! What the model hears in a 16 kHz WAV (only read): `cargo run --release -p qnc-wav2vec2
//! --example hear -- <model folder> <wav>`.

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let (folder, wav) = (args.get(1).ok_or("model folder")?, args.get(2).ok_or("wav")?);
    let started = std::time::Instant::now();
    let model = qnc_wav2vec2::Wav2Vec2::load(std::path::Path::new(folder))?;
    let loaded = started.elapsed();
    let samples = qnc_transcribe_jobs::read_wav(std::path::Path::new(wav))?;
    let started = std::time::Instant::now();
    let emissions = model.emissions(&samples)?;
    println!("load {loaded:?}, {:.1} s of speech in {:?}, {} frames", samples.len() as f64 / 16000.0, started.elapsed(), emissions.frames.len());
    println!("{}", model.greedy(&emissions));
    Ok(())
}
