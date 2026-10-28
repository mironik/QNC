//! Reads (only) the words of one clip's transcripts between two frames, in the active project
//! of a QNC root. Usage: words QNC_ROOT CLIP_ID FROM TO
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, clip, from, to] = args.as_slice() else { return Err("usage: words QNC_ROOT CLIP_ID FROM TO".into()) };
    let (from, to): (u64, u64) = (from.parse().map_err(|_| "FROM")?, to.parse().map_err(|_| "TO")?);
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    for transcript in qnc_transcript_db::read_clip(&target, clip)? {
        println!("[{}..{}] {} {}", transcript.from_frame, transcript.to_frame, transcript.engine, transcript.language);
        for word in transcript.lines.iter().flat_map(|l| l.words.iter()).filter(|w| w.to >= from && w.from <= to) {
            println!("{:6}-{:6} {}", word.from, word.to, word.text);
        }
    }
    Ok(())
}
