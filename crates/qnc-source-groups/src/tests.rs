use super::*;

const SOURCE: &str = "qnc://local/source/card";
fn reference(path: &str) -> SourceReference {
    SourceReference::new(SOURCE, path).unwrap()
}
fn proposal() -> GroupProposal {
    GroupProposal {
        root: reference("PRIVATE/XDROOT"),
        recording_identity: "recording-a".into(),
        evidence: GroupEvidence {
            reader_id: "test.index.read".into(),
            document: reference("PRIVATE/XDROOT/MEDIAPRO.XML"),
            locator: "Contents/Material[1]".into(),
        },
        original: reference("PRIVATE/XDROOT/Clip/A.MXF"),
        proxies: vec![reference("PRIVATE/XDROOT/Sub/A.MP4")],
        related: vec![RelatedReference {
            reference: reference("PRIVATE/XDROOT/Thmbnl/A.JPG"),
            kind: "JPG".into(),
        }],
    }
}
fn facts(groups: &[GroupProposal]) -> Vec<FileFact> {
    groups
        .iter()
        .flat_map(GroupProposal::references)
        .map(|r| {
            (
                r.uri(),
                FileFact {
                    reference: r.clone(),
                    state: FileState::File,
                },
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}
fn grouped(groups: Vec<GroupProposal>) -> GroupReport {
    assemble(SOURCE, groups.clone(), &facts(&groups)).unwrap()
}

#[test]
fn explicit_original_proxy_support_and_evidence_stay_together() {
    let p = proposal();
    let result = grouped(vec![p.clone()]);
    assert_eq!(result.groups.len(), 1);
    assert_eq!(result.groups[0].proposal, p);
    assert!(result.blocked.is_empty());
    assert_eq!(
        serde_json::from_str::<GroupReport>(&serde_json::to_string(&result).unwrap()).unwrap(),
        result
    );
}

#[test]
fn missing_or_unchecked_original_never_promotes_proxy() {
    let p = proposal();
    for state in [None, Some(FileState::Missing), Some(FileState::Unavailable)] {
        let mut facts = facts(std::slice::from_ref(&p));
        facts.retain(|f| f.reference != p.original);
        if let Some(state) = state {
            facts.push(FileFact {
                reference: p.original.clone(),
                state,
            });
        }
        let result = assemble(SOURCE, vec![p.clone()], &facts).unwrap();
        assert!(result.groups.is_empty());
        assert!(result.blocked[0]
            .issues
            .contains(&GroupIssue::OriginalUnavailable));
        assert_eq!(result.blocked[0].proposal.proxies, p.proxies);
    }
}

#[test]
fn missing_indexed_proxy_blocks_group_without_silently_dropping_it() {
    let p = proposal();
    let mut facts = facts(std::slice::from_ref(&p));
    facts
        .iter_mut()
        .find(|f| f.reference == p.proxies[0])
        .unwrap()
        .state = FileState::Missing;
    let result = assemble(SOURCE, vec![p.clone()], &facts).unwrap();
    assert!(result.groups.is_empty());
    assert_eq!(result.blocked[0].issues, vec![GroupIssue::ProxyUnavailable]);
    assert_eq!(result.blocked[0].proposal.proxies.len(), 1);
}

#[test]
fn missing_optional_support_remains_explicit_without_losing_media_group() {
    let p = proposal();
    let mut facts = facts(std::slice::from_ref(&p));
    facts
        .iter_mut()
        .find(|f| f.reference == p.related[0].reference)
        .unwrap()
        .state = FileState::Missing;
    let result = assemble(SOURCE, vec![p], &facts).unwrap();
    assert_eq!(result.groups.len(), 1);
    assert_eq!(result.groups[0].related_states[0].state, FileState::Missing);
}

#[test]
fn no_proxy_and_multiple_proxies_are_explicitly_supported_without_selection() {
    let mut p = proposal();
    p.proxies.clear();
    assert_eq!(grouped(vec![p.clone()]).groups.len(), 1);
    p.proxies = vec![
        reference("PRIVATE/XDROOT/Sub/A.MP4"),
        reference("PRIVATE/XDROOT/Sub/A-low.MP4"),
    ];
    assert_eq!(grouped(vec![p]).groups[0].proposal.proxies.len(), 2);
}

#[test]
fn shared_media_blocks_both_groups() {
    let a = proposal();
    let mut b = a.clone();
    b.recording_identity = "recording-b".into();
    b.original = reference("PRIVATE/XDROOT/Clip/B.MXF");
    let result = grouped(vec![a, b]);
    assert!(result.groups.is_empty());
    assert_eq!(result.blocked.len(), 2);
    assert!(result
        .blocked
        .iter()
        .all(|g| g.issues.contains(&GroupIssue::SharedMedia)));
}

#[test]
fn duplicate_scoped_identity_is_not_first_match_wins() {
    let a = proposal();
    let mut b = a.clone();
    b.original = reference("PRIVATE/XDROOT/Clip/B.MXF");
    b.proxies.clear();
    let result = grouped(vec![a, b]);
    assert!(result.groups.is_empty());
    assert!(result
        .blocked
        .iter()
        .all(|g| g.issues.contains(&GroupIssue::DuplicateIdentity)));
}

#[test]
fn identical_recording_names_in_other_roots_do_not_collide() {
    let a = proposal();
    let mut b = a.clone();
    b.root = reference("OTHER/XDROOT");
    b.original = b.root.descendant("Clip/A.MXF").unwrap();
    b.proxies.clear();
    b.related.clear();
    b.evidence.document = b.root.descendant("MEDIAPRO.XML").unwrap();
    let result = grouped(vec![a, b]);
    assert_eq!(result.groups.len(), 2);
}

#[test]
fn media_role_aliases_and_cross_group_support_aliases_are_conflicts() {
    let mut a = proposal();
    a.proxies.push(a.original.clone());
    assert!(grouped(vec![a]).blocked[0]
        .issues
        .contains(&GroupIssue::ConflictingRoles));
    let a = proposal();
    let mut b = a.clone();
    b.recording_identity = "b".into();
    b.original = reference("PRIVATE/XDROOT/Clip/B.MXF");
    b.proxies.clear();
    b.related[0].reference = a.original.clone();
    let result = grouped(vec![a, b]);
    assert_eq!(result.blocked.len(), 2);
    assert!(result
        .blocked
        .iter()
        .all(|g| g.issues.contains(&GroupIssue::ConflictingRoles)));
}

#[test]
fn unsafe_cross_source_and_outside_recording_root_references_are_rejected() {
    for path in ["outside.MXF", "PRIVATE/XDROOT-sibling/A.MXF"] {
        let mut p = proposal();
        p.original = reference(path);
        assert!(assemble(SOURCE, vec![p], &[]).is_err());
    }
    let mut p = proposal();
    p.original =
        SourceReference::new("qnc://local/source/other", "PRIVATE/XDROOT/Clip/A.MXF").unwrap();
    assert!(assemble(SOURCE, vec![p], &[]).is_err());
    let mut encoded = serde_json::to_value(proposal()).unwrap();
    encoded["original"]["relative_path"] = serde_json::json!("PRIVATE/XDROOT/../outside.MXF");
    assert!(assemble(SOURCE, vec![serde_json::from_value(encoded).unwrap()], &[]).is_err());
}

#[test]
fn evidence_must_be_present_and_conflicting_file_facts_are_rejected() {
    let p = proposal();
    let mut facts = facts(std::slice::from_ref(&p));
    facts.retain(|f| f.reference != p.evidence.document);
    assert_eq!(
        assemble(SOURCE, vec![p.clone()], &facts).unwrap().blocked[0].issues,
        vec![GroupIssue::EvidenceUnavailable]
    );
    facts.push(facts[0].clone());
    assert!(assemble(SOURCE, vec![p], &facts).is_err());
}

#[test]
fn pure_grouping_manifest_has_no_io_app_or_camera_dependency() {
    assert!(qnc_contracts::validate_module_manifest_json(
        "source-groups",
        include_str!("../../../contracts/modules/source-groups.module.json")
    )
    .is_ok());
    let cargo = include_str!("../Cargo.toml")
        .split("[dev-dependencies]")
        .next()
        .unwrap();
    for forbidden in [
        "qnc-source-reader",
        "qnc-scanner",
        "qnc-sony",
        "qnc-ingest",
        "qnc-project",
        "rusqlite",
        "ureq",
    ] {
        assert!(!cargo.contains(forbidden));
    }
}
