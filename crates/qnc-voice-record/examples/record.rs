//! Records SECONDS of the microphone into DIR/voice.wav. Usage: record DIR SECONDS
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(args.first().ok_or("usage: record DIR SECONDS")?);
    let seconds: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let recorder = qnc_voice_record::Recorder::start(dir.join("voice.wav"), &dir, 48_000, 50.0, &dir.join(qnc_voice_record::INPUT_SETTING))?;
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
    println!("peak {:.3}", recorder.take_peak());
    let recording = recorder.stop()?;
    println!("{:?}", recording);
    Ok(())
}
