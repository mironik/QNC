//! Makes a voice-over clip of SOUND into DIR/voice.mxf. Usage: make DIR SOUND TEXT
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, sound, text] = args.as_slice() else { return Err("usage: make DIR SOUND TEXT".into()) };
    let dir = std::path::PathBuf::from(dir);
    let out = dir.join("voice.mxf");
    let clip = qnc_voice_clip::VoiceClip { sound: std::path::Path::new(sound), text, heading: "OFF 1", width: 1920, height: 1080, fps: 50.0, timecode: "19:14:48:00", out: &out, project_dir: &dir };
    let made = qnc_voice_clip::make(&clip)?;
    println!("{}", made.display());
    Ok(())
}
