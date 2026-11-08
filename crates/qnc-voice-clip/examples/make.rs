//! Makes a voice-over clip of the SOUNDs (one channel each) into DIR/voice.mxf.
//! Usage: make DIR SECONDS TEXT SOUND... (QNC_VOICE_50I=1: 1080i50, upper field first)
fn main() -> Result<(), String> {
    let interlaced = std::env::var_os("QNC_VOICE_50I").is_some();
    // QNC_VOICE_SEQUENCE: the picture is that numbered sequence, not a title card.
    let sequence = std::env::var_os("QNC_VOICE_SEQUENCE").map(std::path::PathBuf::from);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, seconds, text, sounds @ ..] = args.as_slice() else { return Err("usage: make DIR SECONDS TEXT SOUND...".into()) };
    let dir = std::path::PathBuf::from(dir);
    let out = dir.join("voice.mxf");
    let sounds: Vec<std::path::PathBuf> = sounds.iter().map(std::path::PathBuf::from).collect();
    let seconds: f64 = seconds.parse().map_err(|_| "SECONDS is a number".to_string())?;
    let clip = qnc_voice_clip::VoiceClip { sounds: &sounds, seconds, text, heading: "OFF 1", sequence: sequence.as_deref(), width: 1920, height: 1080, fps: if interlaced { 25.0 } else { 50.0 }, fields: interlaced.then_some(true), sample_rate: 48_000, timecode: "19:14:48:00", out: &out, project_dir: &dir };
    let made = qnc_voice_clip::make(&clip)?;
    println!("{}", made.display());
    Ok(())
}
