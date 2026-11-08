use super::*;
use qnc_content_store::{Access, ContentClient};
use std::path::PathBuf;

const SOURCE: &str = "qnc://local/source/project-p1";
const DB: &str = "qnc://local/db/ingest_content/p1";

fn record(takes: usize) -> ClipMetadata {
    let sounds = vec![PathBuf::from("t.wav"); takes];
    let clip = qnc_voice_clip::VoiceClip { sounds: &sounds, seconds: 7.11, text: "OFF 1", heading: "", sequence: None, width: 1920, height: 1080, fps: 50.0, fields: None, sample_rate: 48_000, timecode: "21:16:48:45", out: std::path::Path::new("x.mxf"), project_dir: std::path::Path::new(".") };
    qnc_voice_clip::describe(&clip, "voice-P_OFF1", &format!("{SOURCE}/file/products/voice/P_OFF1.mxf")).unwrap()
}

#[test]
fn a_voice_record_is_a_complete_clip_of_the_catalog_and_more_takes_replace_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    // The project database as Project made it (the content tables come with their owner).
    rusqlite::Connection::open(&file)
        .unwrap()
        .execute_batch(
            "CREATE TABLE project_settings(project_id TEXT,settings_json TEXT);
            INSERT INTO project_settings VALUES('p1','{\"storage\":{\"ingest_media\":\"link\"}}');
            CREATE VIEW public_project_settings AS SELECT * FROM project_settings;",
        )
        .unwrap();
    let first = entry(SOURCE, "P", "P_OFF1", record(2), 1).unwrap();
    // Every fact is what was written: picture and as many mono channels as takes, the same length.
    let media = &first.snapshot.metadata.original;
    assert_eq!(media.streams.len(), 3);
    assert_eq!(media.streams[1].duration_ts.as_ref().unwrap().value, 356 * 960, "7.11 s is 356 frames at 50, 960 samples each");
    publish(ContentTarget::from_owner_binding(&file, DB).unwrap(), first).unwrap();
    publish(ContentTarget::from_owner_binding(&file, DB).unwrap(), entry(SOURCE, "P", "P_OFF1", record(3), 2).unwrap()).unwrap();
    let mut reader = ContentClient::from_owner_binding(&file, DB, Access::ReadOnly).unwrap();
    let stored = reader.read("voice-P_OFF1").unwrap().unwrap();
    assert_eq!(stored.clip.snapshot.metadata.original.streams.len(), 4, "the record with one more take replaced the one before");
    assert_eq!(stored.imported_media_uri.as_deref(), Some(stored.clip.snapshot.binding.original_uri.as_str()));
    // A record published with another form of its address takes the one written now.
    let mut moved = entry(SOURCE, "P", "P_OFF1", record(3), 3).unwrap();
    let uri = format!("{SOURCE}/file/products/voice/P%5FOFF1%2Emxf");
    moved.snapshot.metadata.original.media_uri = uri.clone();
    moved.snapshot.metadata.evidence[0].media_uri = uri.clone();
    moved.snapshot.metadata.evidence[0].document_uri = uri.clone();
    moved.snapshot.binding.original_uri = uri.clone();
    moved.snapshot.report = qnc_media_records::inspect(&moved.snapshot.metadata, Phase::Final).unwrap();
    publish(ContentTarget::from_owner_binding(&file, DB).unwrap(), moved).unwrap();
    assert_eq!(reader.read("voice-P_OFF1").unwrap().unwrap().imported_media_uri, Some(uri));
    // A voice record is the script's, not a clip of a card in the pool.
    assert!(reader.list_summary(None).unwrap().is_empty());
}
