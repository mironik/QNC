use super::*;

const READER: &str = "camera.sony.index.read";
const FX3A_SIDECAR: &str = r#"<?xml version="1.0"?><NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.20"><Duration value="96"/><Device manufacturer="Sony" modelName="ILME-FX3A" serialNo="4294967295"/></NonRealTimeMeta>"#;
const FX6_INDEX: &str = r#"<?xml version="1.0"?><MediaProfile xmlns="http://xmlns.sony.net/pro/metadata/mediaprofile"><Properties><System systemId="5026EFFFFEB8895C" systemKind="ILME-FX6V ver.6.000"/></Properties></MediaProfile>"#;

/// The QNC root of this repository (the profiles shipped with QNC).
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_profiles(to: &Path) {
    let dir = to.join(PROFILE_DIR).join("sony");
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(repo().join(PROFILE_DIR).join("sony")).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
    }
}

#[test]
fn the_shipped_profiles_are_valid_and_one_is_the_default_of_the_sony_format() {
    let (found, refused) = read_dir(&repo().join(PROFILE_DIR));
    assert!(refused.is_empty(), "{refused:?}");
    let profiles: Vec<Profile> = found.into_iter().map(|(_, _, p)| p).collect();
    assert!(profiles.len() >= 5);
    assert_eq!(profiles.iter().filter(|p| p.reader == READER && p.default_for_reader).count(), 1);
}

#[test]
fn the_camera_is_recognised_from_the_sidecar_or_the_index_else_the_default_with_a_flag() {
    let (found, _) = read_dir(&repo().join(PROFILE_DIR));
    let profiles: Vec<Profile> = found.into_iter().map(|(_, _, p)| p).collect();
    let (fx3a, known) = recognise(&profiles, READER, &[FX3A_SIDECAR]).unwrap();
    assert!(known && fx3a.id == "sony-ilme-fx3a" && fx3a.stamp() == "sony-ilme-fx3a@1");
    assert_eq!(recognise(&profiles, READER, &[FX6_INDEX]).unwrap().0.id, "sony-ilme-fx6v");
    let unknown = FX3A_SIDECAR.replace("ILME-FX3A", "ILME-FX30");
    let (default, known) = recognise(&profiles, READER, &[&unknown]).unwrap();
    assert!(!known && default.default_for_reader);
    assert!(recognise(&profiles, "camera.other.read", &[FX3A_SIDECAR]).is_none(), "another format has no profile");
}

#[test]
fn sync_keeps_the_valid_profiles_writes_a_changed_one_once_and_keeps_the_last_valid_one_of_a_broken_file() {
    let root = tempfile::tempdir().unwrap();
    copy_profiles(root.path());
    let first = sync(root.path()).unwrap();
    assert!(first.refused.is_empty() && first.written.len() == first.profiles.len());
    let again = sync(root.path()).unwrap();
    assert!(again.written.is_empty(), "nothing changed: nothing written");
    let file = root.path().join(PROFILE_DIR).join("sony/ilme-fx3a.json");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("\"version\": 1", "\"version\": 2")).unwrap();
    let changed = sync(root.path()).unwrap();
    assert_eq!(changed.written, ["sony-ilme-fx3a@2"]);
    std::fs::write(&file, "{ not json").unwrap();
    let broken = sync(root.path()).unwrap();
    assert_eq!(broken.refused.len(), 1);
    assert!(broken.profiles.iter().any(|p| p.stamp() == "sony-ilme-fx3a@2"), "the last valid record stays");
    assert_eq!(stored(root.path()).unwrap(), broken.profiles);
}

#[test]
fn a_profile_without_a_rule_or_with_both_kinds_of_match_is_refused() {
    let (found, _) = read_dir(&repo().join(PROFILE_DIR));
    let mut profile = found.into_iter().map(|(_, _, p)| p).find(|p| p.id == "sony-ilme-fx3a").unwrap();
    profile.recognise[0].prefix = Some("ILME".into());
    assert!(profile.validate().is_err());
    profile.recognise.clear();
    assert!(profile.validate().is_err());
}
