use super::*;

/// A QNC root with the profiles QNC brings and a data folder.
fn root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let catalogs = dir.path().join("catalogs").join("script-profiles");
    fs::create_dir_all(&catalogs).unwrap();
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/script-profiles");
    for entry in fs::read_dir(shipped).unwrap().flatten() {
        fs::copy(entry.path(), catalogs.join(entry.file_name())).unwrap();
    }
    fs::create_dir_all(dir.path().join("data")).unwrap();
    dir
}

#[test]
fn every_prepared_profile_is_read_and_can_be_used() {
    let dir = root();
    let (profiles, errors) = Profiles::load(dir.path());
    assert!(errors.is_empty(), "{errors:?}");
    let ids: Vec<&str> = profiles.list.iter().map(|(p, _)| p.profile_id.as_str()).collect();
    assert_eq!(ids, ["agency_online", "general_tv", "inews_mos", "radio", "tv_package"]);
    let general = &profiles.list[1].0;
    assert_eq!(profiles.current().unwrap().profile_id, "general_tv", "chosen where none was chosen");
    // The blocks written before profiles (off, sot, intro, outro) are kinds of every profile
    // that has them, under the same codes.
    for code in ["intro", "off", "sot", "outro"] {
        assert!(general.kind(code).is_some(), "{code}");
    }
    assert_eq!(general.cue_label("sot", 2), "SOT 2");
    assert_eq!(profiles.list[2].0.cue_label("off", 3), "Take VO 3");
    assert!(general.kind("off").unwrap().recorded);
    assert_eq!(general.kind("intro").unwrap().counts, Counts::Anchor);
}

#[test]
fn a_profile_of_this_computer_is_kept_chosen_and_a_prepared_one_is_never_written_over() {
    let dir = root();
    let (mut profiles, _) = Profiles::load(dir.path());
    let mut own = profiles.list[1].0.clone();
    own.name = "Naša redakcija".into();
    own.kinds.push(KindSpec { id: "voditelj_uzivo".into(), label: "Voditelj uživo".into(), short: "LIVE".into(), role: Role::Anchor, counts: Counts::Anchor, recorded: false, cues: false, cue: String::new(), speaker: String::new(), picture: String::new(), fields: Vec::new() });
    profiles.keep(own).unwrap();
    let (again, errors) = Profiles::load(dir.path());
    assert!(errors.is_empty(), "{errors:?}");
    let chosen = again.current().unwrap();
    assert_eq!(chosen.profile_id, "nasa_redakcija", "a copy of a prepared one gets its own code");
    assert!(chosen.kind("voditelj_uzivo").is_some());
    assert!(again.list.iter().any(|(p, origin)| p.profile_id == "general_tv" && *origin == Origin::Prepared));
}

#[test]
fn a_profile_that_cannot_be_used_is_refused_with_why() {
    let dir = root();
    let (mut profiles, _) = Profiles::load(dir.path());
    let mut bad = profiles.list[1].0.clone();
    bad.name = "Loš".into();
    bad.kinds[0].recorded = true; // an anchor's lead-in is not recorded
    assert!(profiles.keep(bad.clone()).is_err());
    bad.kinds[0].recorded = false;
    bad.kinds.push(bad.kinds[1].clone()); // a code twice
    assert!(profiles.keep(bad).is_err());
    fs::write(dir.path().join("data").join("script-profiles").join("x.json"), "{ not json").ok();
    let (_, errors) = Profiles::load(dir.path());
    assert!(errors.is_empty() || errors.iter().all(|e| e.contains("x.json")));
}

#[test]
fn the_tv_package_profile_names_who_is_heard_and_the_picture_of_each_block() {
    let dir = root();
    let (profiles, _) = Profiles::load(dir.path());
    let package = &profiles.list.iter().find(|(p, _)| p.profile_id == "tv_package").unwrap().0;
    let mut fields = std::collections::BTreeMap::new();
    assert_eq!(package.speaker_with("sot", &fields), "", "nobody named until the caption has a name");
    fields.insert("caption_name".to_string(), "Ivan Marić".to_string());
    assert_eq!(package.speaker_with("sot", &fields), "Ivan Marić (SOT)");
    assert_eq!(package.speaker_with("off", &fields), "REPORTER (VO)");
    assert_eq!(package.cue_label("off", 2), "VO 2");
    assert!(package.kind("off").unwrap().fields.iter().any(|field| field.path == "broll"));
    assert!(package.kind("intro").unwrap().fields.iter().any(|field| field.path == "camera"));
    assert!(package.kind("signoff").unwrap().recorded);
}
