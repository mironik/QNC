use super::*;
use rusqlite::Connection;

const PROJECT_DB: &str = "qnc://local/db/project_db/p1";

fn project() -> (tempfile::TempDir, ProjectDbTarget) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    Connection::open(&file)
        .unwrap()
        .execute_batch(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings (project_id) VALUES ('p1');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;",
        )
        .unwrap();
    (dir, ProjectDbTarget::from_owner_binding(&file, PROJECT_DB).unwrap())
}

fn transcript(from: u64, to: u64, said: &str) -> ClipTranscript {
    let word = Word { from, to: from + 10, text: said.into() };
    ClipTranscript {
        clip_id: "c1".into(),
        from_frame: from,
        to_frame: to,
        language: "hr".into(),
        engine: "qnc.transcribe.whisper@0.1.0 ggml-large-v3-turbo.bin".into(),
        lines: vec![Line { from, to: from + 10, text: said.into(), words: vec![word] }],
    }
}

#[test]
fn a_kept_transcript_is_read_back_and_a_range_said_again_replaces_it() {
    let (_dir, target) = project();
    assert!(read_clip(&target, "c1").unwrap().is_empty(), "a project without transcripts reads none");
    save(&target, transcript(100, 200, "Dobar")).unwrap();
    save(&target, transcript(0, 50, "Vidite")).unwrap();
    save(&target, transcript(100, 200, "Dobar dan")).unwrap();
    let kept = read_clip(&target, "c1").unwrap();
    assert_eq!(kept.iter().map(|t| t.from_frame).collect::<Vec<_>>(), [0, 100], "in the order of their ranges");
    assert_eq!(kept[1].lines[0].text, "Dobar dan");
    assert!(read_clip(&target, "c2").unwrap().is_empty());
}

#[test]
fn a_word_outside_its_range_is_refused_and_nothing_is_kept() {
    let (_dir, target) = project();
    let mut outside = transcript(100, 200, "Dobar");
    outside.lines[0].words[0].to = 250;
    assert!(save(&target, outside).is_err());
    assert!(read_clip(&target, "c1").unwrap().is_empty());
}

fn asked(clip_id: &str, quality: QueueQuality) -> Queued {
    Queued { clip_id: clip_id.into(), from_frame: 0, to_frame: 500, quality, state: QueueState::Waiting, error: String::new(), took_ms: 0 }
}

#[test]
fn queued_clips_are_read_in_order_and_asking_a_clip_again_waits_it_again() {
    let (_dir, target) = project();
    assert!(read_queue(&target).unwrap().is_empty(), "a project without a queue reads none");
    enqueue(&target, vec![asked("a", QueueQuality::Fast), asked("b", QueueQuality::Best)]).unwrap();
    set_queued(&target, "a", QueueState::Done, "", 4200).unwrap();
    set_queued(&target, "b", QueueState::Failed, "no sound", 0).unwrap();
    let queue = read_queue(&target).unwrap();
    assert_eq!(queue[0].took_ms, 4200, "how long a done clip took is kept");
    assert_eq!(queue.iter().map(|q| (q.clip_id.as_str(), q.state, q.quality)).collect::<Vec<_>>(), [("a", QueueState::Done, QueueQuality::Fast), ("b", QueueState::Failed, QueueQuality::Best)]);
    assert_eq!(queue[1].error, "no sound");
    enqueue(&target, vec![asked("b", QueueQuality::Fast)]).unwrap();
    let again = read_queue(&target).unwrap().into_iter().find(|q| q.clip_id == "b").unwrap();
    assert_eq!((again.state, again.quality, again.error.as_str()), (QueueState::Waiting, QueueQuality::Fast, ""));
}

#[test]
fn a_newer_transcript_is_what_was_said_over_its_range() {
    let (_dir, target) = project();
    let mut whole = transcript(0, 1000, "staro");
    whole.lines = vec![
        Line { from: 100, to: 200, text: "prije".into(), words: Vec::new() },
        Line { from: 400, to: 500, text: "unutra".into(), words: Vec::new() },
        Line { from: 800, to: 900, text: "poslije".into(), words: Vec::new() },
    ];
    save(&target, whole).unwrap();
    save(&target, transcript(300, 600, "novo")).unwrap();
    let kept = read_clip(&target, "c1").unwrap();
    let texts: Vec<&str> = kept.iter().flat_map(|t| t.lines.iter().map(|l| l.text.as_str())).collect();
    assert_eq!(texts, ["prije", "poslije", "novo"], "the older line inside the new range goes");
    save(&target, transcript(0, 1000, "sve")).unwrap();
    let kept = read_clip(&target, "c1").unwrap();
    assert_eq!(kept.len(), 1, "a transcript inside the newer one's range goes");
}

#[test]
fn statements_are_kept_in_the_order_added_and_one_can_go() {
    let (_dir, target) = project();
    assert!(read_quotes(&target).unwrap().is_empty(), "a project without statements reads none");
    let quote = |from: u64, text: &str| Quote {
        id: 0,
        clip_id: "c1".into(),
        clip_name: "Mironik 2680.MXF".into(),
        from_frame: from,
        to_frame: from + 100,
        start_tc: "00:21:56:17".into(),
        end_tc: "00:21:58:17".into(),
        length: "00:00:02:00".into(),
        text: text.into(),
    };
    add_quote(&target, quote(500, "Pa dosad njih")).unwrap();
    add_quote(&target, quote(100, "igrača je")).unwrap();
    assert!(add_quote(&target, quote(200, " ")).is_err(), "a statement without text");
    let kept = read_quotes(&target).unwrap();
    assert_eq!(kept.iter().map(|q| q.text.as_str()).collect::<Vec<_>>(), ["Pa dosad njih", "igrača je"]);
    delete_quote(&target, kept[0].id).unwrap();
    assert_eq!(read_quotes(&target).unwrap().iter().map(|q| q.from_frame).collect::<Vec<_>>(), [100]);
}
