//! Reads (only) the words of one clip's transcripts between two frames.
//! Usage: words PROJECT_DB PROJECT_ID CLIP_NAME_PART FROM TO
use qnc_db_broker::ProjectDbTarget;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [file, id, clip, from, to] = args.as_slice() else { return Err("usage: words PROJECT_DB PROJECT_ID CLIP FROM TO".into()) };
    let (from, to): (u64, u64) = (from.parse().map_err(|_| "FROM")?, to.parse().map_err(|_| "TO")?);
    let target = ProjectDbTarget::from_owner_binding(std::path::Path::new(file), &format!("qnc://local/db/project_db/{id}")).map_err(|e| e.to_string())?;
    for queued in qnc_transcript_db::read_queue(&target)? {
        if !queued.clip_id.contains(clip.as_str()) { continue; }
        for transcript in qnc_transcript_db::read_clip(&target, &queued.clip_id)? {
            for word in transcript.lines.iter().flat_map(|l| l.words.iter()).filter(|w| w.to >= from && w.from <= to) {
                println!("{:6}-{:6} {}", word.from, word.to, word.text);
            }
        }
    }
    Ok(())
}
