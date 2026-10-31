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

fn element(kind: ElementKind, clip: &str, in_frame: u64, out_frame: u64) -> NewElement {
    NewElement {
        kind,
        clip_id: clip.into(),
        in_frame,
        out_frame,
        a1: Lane { channel: 0, level_db: 0, off: false },
        a2: Lane { channel: 1, level_db: 0, off: kind == ElementKind::Off },
        origin: format!("script:1:{in_frame}"),
    }
}

#[test]
fn every_send_is_a_new_version_numbered_one_two_and_read_back_in_order() {
    let (_dir, target) = project();
    assert!(versions(&target).unwrap().is_empty(), "no table yet: no versions");
    let first = append_version(&target, vec![element(ElementKind::Off, "voice-a", 0, 120), element(ElementKind::Ton, "clip-b", 50, 300)]).unwrap();
    let second = append_version(&target, vec![element(ElementKind::Ton, "clip-c", 0, 75)]).unwrap();
    assert_eq!((first.number, first.elements, second.number, second.elements), (1, 2, 2, 1));
    let listed = versions(&target).unwrap();
    assert_eq!(listed.iter().map(|version| (version.number, version.elements)).collect::<Vec<_>>(), [(1, 2), (2, 1)]);
    let elements = read(&target, first.version_id).unwrap();
    let clips: Vec<_> = elements.iter().map(|element| element.clip_id.as_str()).collect();
    assert_eq!(clips, ["voice-a", "clip-b"]);
    assert_eq!((elements[1].in_frame, elements[1].out_frame, elements[1].kind), (50, 300, ElementKind::Ton));
    assert_eq!(read(&target, second.version_id).unwrap()[0].clip_id, "clip-c");
}

#[test]
fn an_element_is_changed_moved_within_its_version_and_removed() {
    let (_dir, target) = project();
    let version = append_version(&target, vec![element(ElementKind::Off, "voice-a", 0, 120), element(ElementKind::Ton, "clip-b", 50, 300), element(ElementKind::Ton, "clip-c", 0, 75)]).unwrap();
    let other = append_version(&target, vec![element(ElementKind::Ton, "clip-z", 0, 10)]).unwrap();
    let ids: Vec<i64> = read(&target, version.version_id).unwrap().iter().map(|element| element.element_id).collect();
    let a1 = Lane { channel: 2, level_db: -3, off: false };
    change(&target, Operation::Update { element_id: ids[1], in_frame: 60, out_frame: 280, a1, gap_before: 5, cut: true }).unwrap();
    let changed = &read(&target, version.version_id).unwrap()[1];
    assert_eq!((changed.in_frame, changed.out_frame, changed.a1, changed.gap_before, changed.cut), (60, 280, a1, 5, true));
    change(&target, Operation::Move { element_id: ids[2], earlier: true }).unwrap();
    change(&target, Operation::Move { element_id: ids[0], earlier: true }).unwrap();
    let order: Vec<_> = read(&target, version.version_id).unwrap().iter().map(|element| element.clip_id.clone()).collect();
    assert_eq!(order, ["voice-a", "clip-c", "clip-b"], "the first stays first; the other version is not a neighbour");
    change(&target, Operation::Remove { element_id: ids[0] }).unwrap();
    assert_eq!(read(&target, version.version_id).unwrap().len(), 2);
    assert_eq!(read(&target, other.version_id).unwrap().len(), 1);
    assert!(change(&target, Operation::ReadVersions).is_err());
}

#[test]
fn a_bad_element_writes_no_version() {
    let (_dir, target) = project();
    let mut bad = element(ElementKind::Ton, "clip-b", 50, 50);
    assert!(append_version(&target, vec![element(ElementKind::Ton, "clip-a", 0, 10), bad.clone()]).is_err());
    bad.out_frame = 60;
    bad.a1.level_db = 40;
    assert!(append_version(&target, vec![bad]).is_err());
    assert!(append_version(&target, Vec::new()).is_err(), "an empty version is not made");
    assert!(versions(&target).unwrap().is_empty());
}
