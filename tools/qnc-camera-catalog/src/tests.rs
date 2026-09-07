use super::*;

fn seed() -> Seed {
    serde_json::from_str(include_str!("../../../catalogs/camera-patterns/seed.json")).unwrap()
}

fn revision(base: &str, next: &str, changes: Vec<Change>) -> Revision {
    Revision {
        base_version: base.into(),
        dataset_version: next.into(),
        changed_on: "2026-09-06".into(),
        sources: vec![],
        changes,
    }
}

#[test]
fn seed_has_factory_sources_directories_and_named_relationships() {
    let seed = seed();
    let db = build(&seed).unwrap();
    assert!(seed.patterns.len() >= 20);
    let enabled: i64 = db
        .query_row("SELECT count(*) FROM public_analysis_patterns", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(enabled >= 10);
    for p in seed.patterns.iter().filter(|p| p.status == "enabled") {
        assert!(!p.roots.is_empty());
        assert!(!p.naming_rule.is_empty());
        assert!(!p.grouping_notes.is_empty());
        assert!(p.evidence.iter().any(|e| seed
            .sources
            .iter()
            .any(|s| s.id == e.source && s.kind == "manufacturer_document")));
    }
}

#[test]
fn factory_documentation_does_not_require_owning_a_card() {
    let mut seed = seed();
    seed.patterns.retain(|p| p.id == "red-r3d-prores");
    seed.sources.retain(|s| s.kind == "manufacturer_document");
    seed.gaps.clear();
    assert!(build(&seed).is_ok());
}

#[test]
fn directory_and_recording_mode_are_kept_separate_from_extension() {
    let db = build(&seed()).unwrap();
    let red_raw = pattern(&db, "red-r3d-prores").unwrap();
    let red_mov = pattern(&db, "red-prores-original").unwrap();
    assert!(red_raw
        .files
        .iter()
        .any(|f| f.path == "*.RDC/*.mov" && f.role == "proxy_candidate"));
    assert!(red_mov
        .files
        .iter()
        .any(|f| f.path == "*.RDC/*.mov" && f.role == "original_candidate"));
    assert_ne!(red_raw.applicability, red_mov.applicability);
    let p2 = pattern(&db, "panasonic-p2-separate").unwrap();
    assert!(p2
        .files
        .iter()
        .any(|f| f.path == "AUDIO/*.MXF" && f.role == "audio_component"));
    assert!(p2
        .files
        .iter()
        .any(|f| f.path == "VIDEO/*.MXF" && f.role == "original_candidate"));
}

#[test]
fn sony_mixed_card_has_two_patterns_and_explicit_proxy_links() {
    let db = build(&seed()).unwrap();
    let xd = pattern(&db, "sony-xdroot-sd").unwrap();
    let m4 = pattern(&db, "sony-m4root-sd").unwrap();
    assert_eq!(xd.roots, ["PRIVATE/XDROOT"]);
    assert_eq!(m4.roots, ["PRIVATE/M4ROOT"]);
    assert_eq!(xd.grouping_method, "manifest_references");
    assert!(xd
        .metadata
        .iter()
        .any(|m| m.selector == "Contents/Material/Proxy/@uri"));
    assert!(xd
        .metadata
        .iter()
        .any(|m| m.selector == "CreationDate/@value"));
}

#[test]
fn unknown_or_extension_only_profiles_cannot_be_enabled() {
    let mut seed = seed();
    let p = seed
        .patterns
        .iter_mut()
        .find(|p| p.id == "dji-osmo-lrf")
        .unwrap();
    p.status = "enabled".into();
    assert!(build(&seed).is_err());
}

#[test]
fn path_expressions_are_os_neutral_and_cannot_escape_source() {
    for bad in [
        "C:/PRIVATE",
        "/PRIVATE",
        "\\\\server\\share",
        "../Clip",
        "Clip/../Sub",
        "Clip//file",
        "https://host/card",
        "Clip/\nfile",
    ] {
        assert!(relative(bad).is_err(), "{bad}");
    }
    for good in [
        "PRIVATE/XDROOT",
        "*.RDM",
        "CLPR/*/*.MP4",
        "Proxy/*_Proxy.MP4",
        ".",
        "Clip/My clip.MXF",
    ] {
        relative(good).unwrap();
    }
}

#[test]
fn missing_evidence_and_false_observation_claim_are_rejected() {
    let mut missing = seed();
    missing.patterns[0].evidence[0].source = "not-a-source".into();
    assert!(build(&missing).is_err());
    let mut false_claim = seed();
    false_claim.patterns[0]
        .evidence
        .retain(|e| e.source != "sony-sd-observation");
    assert!(build(&false_claim).is_err());
}

#[test]
fn duplicate_ids_and_unknown_executable_fields_are_rejected() {
    let mut duplicate = seed();
    duplicate.patterns.push(duplicate.patterns[0].clone());
    assert!(build(&duplicate).is_err());
    let mut value = serde_json::to_value(&duplicate.patterns[0]).unwrap();
    value["command"] = "ffprobe".into();
    assert!(serde_json::from_value::<Pattern>(value).is_err());
}

#[test]
fn mark_incorrect_removes_only_analysis_eligibility_and_logs_reason() {
    let seed = seed();
    let source = build(&seed).unwrap();
    let id = "sony-xdroot-sd";
    let edited = revise(
        &source,
        &revision(
            &seed.dataset_version,
            "test.2",
            vec![Change::MarkIncorrect {
                id: id.into(),
                reason: "Factory correction pending".into(),
            }],
        ),
    )
    .unwrap();
    assert_eq!(pattern(&edited, id).unwrap().status, "incorrect");
    assert_eq!(pattern(&source, id).unwrap().status, "enabled");
    assert!(!edited
        .prepare("SELECT 1 FROM public_analysis_patterns WHERE pattern_id=?1")
        .unwrap()
        .exists([id])
        .unwrap());
    let reason: String = edited
        .query_row(
            "SELECT reason FROM public_changes ORDER BY change_id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reason, "Factory correction pending");
    assert!(!pattern(&edited, id).unwrap().files.is_empty());
}

#[test]
fn add_replace_disable_enable_and_delete_are_versioned() {
    let seed = seed();
    let base = build(&seed).unwrap();
    let mut p = pattern(&base, "sony-xdroot-sd").unwrap();
    p.id = "test-only-pattern".into();
    let added = revise(
        &base,
        &revision(
            &seed.dataset_version,
            "test.2",
            vec![Change::Add {
                pattern: Box::new(p.clone()),
                reason: "Add test fixture".into(),
            }],
        ),
    )
    .unwrap();
    p.naming_rule = "Corrected fixture naming".into();
    let replaced = revise(
        &added,
        &revision(
            "test.2",
            "test.3",
            vec![Change::Replace {
                pattern: Box::new(p),
                reason: "Correct test fixture".into(),
            }],
        ),
    )
    .unwrap();
    let disabled = revise(
        &replaced,
        &revision(
            "test.3",
            "test.4",
            vec![Change::Disable {
                id: "test-only-pattern".into(),
                reason: "Suspend fixture".into(),
            }],
        ),
    )
    .unwrap();
    assert_eq!(
        pattern(&disabled, "test-only-pattern").unwrap().status,
        "disabled"
    );
    let enabled = revise(
        &disabled,
        &revision(
            "test.4",
            "test.5",
            vec![Change::Enable {
                id: "test-only-pattern".into(),
                reason: "Reviewed fixture".into(),
            }],
        ),
    )
    .unwrap();
    assert_eq!(
        pattern(&enabled, "test-only-pattern").unwrap().naming_rule,
        "Corrected fixture naming"
    );
    let deleted = revise(
        &enabled,
        &revision(
            "test.5",
            "test.6",
            vec![Change::Delete {
                id: "test-only-pattern".into(),
                reason: "Remove fixture".into(),
            }],
        ),
    )
    .unwrap();
    assert!(pattern(&deleted, "test-only-pattern").is_err());
    for table in ["pattern_root", "file_rule", "metadata_field", "evidence"] {
        assert!(!deleted
            .prepare(&format!(
                "SELECT 1 FROM {table} WHERE pattern_id='test-only-pattern'"
            ))
            .unwrap()
            .exists([])
            .unwrap());
    }
    let before: String = deleted
        .query_row(
            "SELECT before_json FROM change_log ORDER BY change_id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Pattern>(&before)
            .unwrap()
            .naming_rule,
        "Corrected fixture naming"
    );
}

#[test]
fn failed_revision_never_modifies_source() {
    let seed = seed();
    let db = build(&seed).unwrap();
    let before = db.serialize(DatabaseName::Main).unwrap().to_vec();
    let changes = vec![
        Change::Delete {
            id: "sony-xdroot-sd".into(),
            reason: "Test deletion".into(),
        },
        Change::Delete {
            id: "does-not-exist".into(),
            reason: "Must fail".into(),
        },
    ];
    assert!(revise(&db, &revision(&seed.dataset_version, "test.2", changes)).is_err());
    assert_eq!(&*db.serialize(DatabaseName::Main).unwrap(), before);
}

#[test]
fn stale_revision_and_empty_reason_fail() {
    let seed = seed();
    let db = build(&seed).unwrap();
    for (base, reason) in [
        ("wrong-version", "why"),
        (seed.dataset_version.as_str(), ""),
    ] {
        assert!(revise(
            &db,
            &revision(
                base,
                "test.2",
                vec![Change::Delete {
                    id: "sony-xdroot-sd".into(),
                    reason: reason.into()
                }]
            )
        )
        .is_err());
    }
}

#[test]
fn duplicate_changes_and_unknown_operations_fail() {
    let seed = seed();
    let db = build(&seed).unwrap();
    let changes = vec![
        Change::Disable {
            id: "sony-xdroot-sd".into(),
            reason: "Suspend".into(),
        },
        Change::Delete {
            id: "sony-xdroot-sd".into(),
            reason: "Delete".into(),
        },
    ];
    assert!(revise(&db, &revision(&seed.dataset_version, "test.2", changes)).is_err());
    assert!(serde_json::from_str::<Change>(r#"{"operation":"run","command":"anything"}"#).is_err());
}

#[test]
fn publisher_does_not_overwrite_and_reader_does_not_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("camera catalog.sqlite");
    let db = build(&seed()).unwrap();
    publish(&db, &path).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(publish(&db, &path).is_err());
    let reader = open_read_only(&path).unwrap();
    assert!(reader.execute("DELETE FROM pattern", []).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!path.with_extension("sqlite-wal").exists());
}

#[test]
fn foreign_database_and_modified_physical_schema_are_rejected() {
    let foreign = Connection::open_in_memory().unwrap();
    foreign
        .execute_batch("CREATE TABLE projects (id TEXT)")
        .unwrap();
    assert!(check(&foreign).is_err());
    let tampered = build(&seed()).unwrap();
    tampered.execute_batch("DROP VIEW public_analysis_patterns; CREATE VIEW public_analysis_patterns AS SELECT * FROM pattern;").unwrap();
    assert!(check(&tampered).is_err());
}

#[test]
fn packaged_database_matches_seed_and_public_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/camera-patterns");
    let path = root.join("camera-patterns-v1.sqlite");
    let db = open_read_only(&path).unwrap();
    let expected = build(&seed()).unwrap();
    assert_eq!(
        &*db.serialize(DatabaseName::Main).unwrap(),
        &*expected.serialize(DatabaseName::Main).unwrap()
    );
    let contract: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("contract.json")).unwrap()).unwrap();
    for view in contract["public_views"].as_array().unwrap() {
        assert!(db
            .prepare("SELECT 1 FROM sqlite_schema WHERE type='view' AND name=?1")
            .unwrap()
            .exists([view.as_str().unwrap()])
            .unwrap());
    }
}

#[test]
fn packaged_sony_metadata_revision_preserves_the_original_release() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/camera-patterns");
    let base_path = root.join("camera-patterns-v1.sqlite");
    let before = fs::read(&base_path).unwrap();
    let base = open_read_only(&base_path).unwrap();
    let revision: Revision = serde_json::from_str(include_str!(
        "../../../catalogs/camera-patterns/revisions/2026.09.07.1.json"
    ))
    .unwrap();
    let expected = revise(&base, &revision).unwrap();
    let actual = open_read_only(&root.join("camera-patterns-2026.09.07.1.sqlite")).unwrap();
    assert!(
        *actual.serialize(DatabaseName::Main).unwrap()
            == *expected.serialize(DatabaseName::Main).unwrap(),
        "published Sony revision differs from declared changes"
    );
    assert!(
        fs::read(&base_path).unwrap() == before,
        "base release was changed"
    );
    let added: i64 = actual
        .query_row(
            "SELECT COUNT(*) FROM public_metadata_fields WHERE pattern_id='sony-xdroot-sd'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(added, 28);
    let changes: i64 = actual.query_row("SELECT COUNT(*) FROM public_changes WHERE dataset_version=?1 AND operation='replace' AND before_json IS NOT NULL AND after_json IS NOT NULL", [&revision.dataset_version], |r| r.get(0)).unwrap();
    assert_eq!(changes, 1);
}
