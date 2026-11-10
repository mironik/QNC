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

fn part(from: u64, to: u64, vector: Vec<f32>) -> PartContent {
    PartContent { from, to, kind: "steady".into(), frames: vec![from], vector, poster: String::new(), fine: false }
}

fn content(clip: &str, parts: Vec<PartContent>) -> ClipContent {
    ClipContent { clip_id: clip.into(), model: "siglip-base-patch16-224".into(), parts, error: String::new(), took_ms: 400 }
}

#[test]
fn kept_content_is_read_back_by_clip_and_by_model_and_a_new_one_replaces_it() {
    let (_dir, target) = project();
    assert_eq!(read_clip(&target, "c1").unwrap(), None, "a project without the table reads none");
    save(&target, content("c1", vec![part(0, 100, vec![1.0, 0.0]), part(150, 300, vec![0.0, 1.0])]), None).unwrap();
    let failed = ClipContent { error: "no index".into(), ..content("c2", Vec::new()) };
    save(&target, failed.clone(), None).unwrap();
    save(&target, ClipContent { model: "other".into(), ..content("c3", vec![part(0, 10, vec![1.0])]) }, None).unwrap();
    assert_eq!(read_clip(&target, "c2").unwrap(), Some(failed));
    let all = read_all(&target, "siglip-base-patch16-224").unwrap();
    assert_eq!(all.iter().map(|c| c.clip_id.as_str()).collect::<Vec<_>>(), ["c1", "c2"], "another model's vectors are not mixed in");
    save(&target, content("c1", vec![part(0, 300, vec![0.5, 0.5])]), None).unwrap();
    assert_eq!(read_clip(&target, "c1").unwrap().unwrap().parts, [part(0, 300, vec![0.5, 0.5])]);
}

#[test]
fn the_label_vectors_of_a_model_are_kept_and_replaced() {
    let (_dir, target) = project();
    assert_eq!(read_labels(&target, "m").unwrap(), None);
    let set = |prompt: &str| LabelSet { model: "m".into(), scale: 110.0, bias: -16.0, labels: vec![LabelVector { code: "sea".into(), prompt: prompt.into(), vector: vec![1.0, 0.0] }] };
    save_labels(&target, set("a photo of the sea"), None).unwrap();
    save_labels(&target, set("a photo of a harbour"), None).unwrap();
    assert_eq!(read_labels(&target, "m").unwrap(), Some(set("a photo of a harbour")));
    assert_eq!(read_labels(&target, "other").unwrap(), None);
    assert!(save_labels(&target, LabelSet { labels: Vec::new(), ..set("x") }, None).is_err(), "an empty list is refused");
}

#[test]
fn a_part_outside_itself_a_vector_of_another_width_or_an_error_beside_parts_is_refused() {
    let (_dir, target) = project();
    let outside = PartContent { frames: vec![500], ..part(0, 100, vec![1.0]) };
    assert!(save(&target, content("c1", vec![outside]), None).is_err());
    assert!(save(&target, content("c1", vec![part(0, 10, vec![1.0]), part(20, 30, vec![1.0, 0.0])]), None).is_err());
    assert!(save(&target, content("c1", vec![part(0, 10, vec![f32::NAN])]), None).is_err());
    let both = ClipContent { error: "x".into(), ..content("c1", vec![part(0, 10, vec![1.0])]) };
    assert!(save(&target, both, None).is_err());
    assert_eq!(read_clip(&target, "c1").unwrap(), None, "nothing written");
}
