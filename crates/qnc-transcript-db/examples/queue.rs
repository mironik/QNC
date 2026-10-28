//! Reads (only) a project database's transcription queue and the transcripts of its clips.
//! Usage: queue PROJECT_DB PROJECT_ID

use qnc_db_broker::ProjectDbTarget;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [file, id] = args.as_slice() else { return Err("usage: queue PROJECT_DB PROJECT_ID".into()) };
    let target = ProjectDbTarget::from_owner_binding(std::path::Path::new(file), &format!("qnc://local/db/project_db/{id}")).map_err(|e| e.to_string())?;
    for queued in qnc_transcript_db::read_queue(&target)? {
        println!("{} {}..{} {:?} {:?} {}", queued.clip_id, queued.from_frame, queued.to_frame, queued.quality, queued.state, queued.error);
        for transcript in qnc_transcript_db::read_clip(&target, &queued.clip_id)? {
            println!("  [{}..{}] {} {}", transcript.from_frame, transcript.to_frame, transcript.engine, transcript.language);
            for line in &transcript.lines {
                println!("    {:6}-{:6} {}", line.from, line.to, line.text);
            }
        }
    }
    Ok(())
}
