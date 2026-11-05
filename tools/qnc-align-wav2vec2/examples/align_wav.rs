//! The letters of a known text in a 16 kHz WAV that is only read: `cargo run --release -p
//! qnc-align-wav2vec2 --example align_wav -- <model folder> <wav> <text>`; without a text, what
//! the model hears.

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let (folder, wav) = (std::path::Path::new(args.get(1).ok_or("model folder")?), std::path::Path::new(args.get(2).ok_or("wav")?));
    let samples = qnc_transcribe_jobs::read_wav(wav)?;
    let mut engine = qnc_align_wav2vec2::Engine::default();
    let started = std::time::Instant::now();
    let Some(text) = args.get(3) else {
        println!("{}", engine.hear(folder, &samples)?);
        println!("{:?}", started.elapsed());
        return Ok(());
    };
    let words: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    let letters = engine.align(folder, &samples, &words)?;
    println!("{:?}", started.elapsed());
    for (word, letters) in words.iter().zip(letters) {
        let shown: Vec<String> = letters.letters.iter().map(|l| match (l.start, l.end) {
            (Some(start), Some(end)) => format!("{}{:.2}-{:.2}", l.letter, start, end),
            _ => format!("{}-", l.letter),
        }).collect();
        println!("{word}: {}", shown.join(" "));
    }
    Ok(())
}
