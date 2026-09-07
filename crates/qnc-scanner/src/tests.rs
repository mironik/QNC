use super::*;
use qnc_sony_metadata::SonyIndexReader;
use qnc_source_groups::{GroupEvidence, GroupIssue};
use qnc_transport_resolver::ResolverConfig;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

const SOURCE: &str = "qnc://local/source/card";
const INDEX: &str = include_str!("../../qnc-sony-metadata/tests/fixtures/MEDIAPRO.XML");

fn catalog() -> Catalog {
    let uri = "qnc://local/catalog/camera-patterns";
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite");
    qnc_camera_patterns::read_uri(
        &ResolverConfig::new(PathBuf::new()).with_local_binding(uri, path),
        uri,
        None,
    )
    .unwrap()
}
fn write(dir: &Path, path: &str, content: &str) {
    let path = dir.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn fixture(excluded: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for relative in [
        "Clip/TEST A.MXF",
        "Clip/TEST B.MXF",
        "Sub/TEST AS03.MP4",
        "Sub/TEST BS03.MP4",
        "Clip/TEST AM01.XML",
        "Clip/TEST BM01.XML",
        "Thmbnl/TEST AT01.JPG",
        "MEDIAPRO.XML",
    ] {
        if !excluded.contains(&relative) {
            write(
                dir.path(),
                &format!("PRIVATE/XDROOT/{relative}"),
                if relative == "MEDIAPRO.XML" {
                    INDEX
                } else {
                    "test placeholder; no media probe or decode"
                },
            );
        }
    }
    dir
}
fn run(dir: &Path) -> ScanReport {
    scan_roles(
        &catalog(),
        &SourceReader::local(SOURCE, dir).unwrap(),
        SourceScope::CardRelative,
        &[&SonyIndexReader],
        ScanLimits::default(),
    )
    .unwrap()
}

#[test]
fn sony_index_forms_two_groups_not_four_standalone_media_clips() {
    let dir = fixture(&[]);
    let report = run(dir.path());
    assert!(
        report.relationships_resolved(),
        "{:?} {:?}",
        report.issues,
        report.unresolved_files
    );
    assert_eq!(report.grouping.groups.len(), 2);
    assert!(report
        .grouping
        .groups
        .iter()
        .all(|g| g.proposal.proxies.len() == 1));
    assert_eq!(report.indexes_read, 1);
    assert_eq!(report.file_facts.len(), 8);
    assert!(report.grouping.groups[0]
        .proposal
        .original
        .relative_path()
        .contains("/Clip/"));
    assert!(report.grouping.groups[0].proposal.proxies[0]
        .relative_path()
        .contains("/Sub/"));
    let wire = serde_json::to_string(&report).unwrap();
    let copy: ScanReport = serde_json::from_str(&wire).unwrap();
    assert_eq!(copy.grouping, report.grouping);
}

#[test]
fn missing_original_or_proxy_blocks_only_its_group() {
    for (missing, code) in [
        ("Clip/TEST A.MXF", GroupIssue::OriginalUnavailable),
        ("Sub/TEST AS03.MP4", GroupIssue::ProxyUnavailable),
    ] {
        let dir = fixture(&[missing]);
        let report = run(dir.path());
        assert!(!report.relationships_resolved());
        assert_eq!(report.grouping.groups.len(), 1);
        assert_eq!(report.grouping.blocked.len(), 1);
        assert!(report.grouping.blocked[0].issues.contains(&code));
        assert_eq!(
            report.grouping.blocked[0].proposal.recording_identity,
            "ORIGINAL-A"
        );
        assert_eq!(report.grouping.blocked[0].proposal.proxies.len(), 1);
    }
}

#[test]
fn missing_support_is_recorded_without_inventing_media_metadata() {
    let dir = fixture(&["Thmbnl/TEST AT01.JPG"]);
    let report = run(dir.path());
    assert_eq!(report.grouping.groups.len(), 2);
    assert!(!report.relationships_resolved());
    assert!(report.grouping.groups[0]
        .related_states
        .iter()
        .any(|f| f.state == FileState::Missing));
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == ScanIssueCode::FileUnavailable));
}

#[test]
fn unindexed_files_remain_unresolved_without_suffix_pairing_or_probe() {
    let dir = fixture(&[]);
    write(dir.path(), "PRIVATE/XDROOT/Clip/COPY.MXF", "copy");
    write(dir.path(), "PRIVATE/XDROOT/Sub/COPYS03.MP4", "copy");
    let report = run(dir.path());
    assert_eq!(report.grouping.groups.len(), 2);
    assert_eq!(report.unresolved_files.len(), 2);
    assert!(report
        .unresolved_files
        .iter()
        .any(|f| f.candidate_roles == ["proxy_candidate"]));
}

#[test]
fn explicit_index_reference_overrides_suffix_hint_but_remains_root_scoped() {
    let dir = fixture(&["Sub/TEST AS03.MP4"]);
    write(
        dir.path(),
        "PRIVATE/XDROOT/other/proxy-A.MP4",
        "referenced proxy",
    );
    write(
        dir.path(),
        "PRIVATE/XDROOT/MEDIAPRO.XML",
        &INDEX.replace("./Sub/TEST AS03.MP4", "./other/proxy-A.MP4"),
    );
    let report = run(dir.path());
    assert_eq!(report.grouping.groups.len(), 2);
    assert!(report.relationships_resolved());
    assert_eq!(
        report.grouping.groups[0].proposal.proxies[0].relative_path(),
        "PRIVATE/XDROOT/other/proxy-A.MP4"
    );
}

#[test]
fn malformed_missing_and_unsupported_indexes_never_trigger_original_only_fallback() {
    let dir = fixture(&["MEDIAPRO.XML"]);
    let report = run(dir.path());
    assert!(report.grouping.groups.is_empty());
    assert!(!report.unresolved_files.is_empty());
    write(dir.path(), "PRIVATE/XDROOT/MEDIAPRO.XML", "<unrelated/>");
    let report = run(dir.path());
    assert!(report.grouping.groups.is_empty());
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == ScanIssueCode::IndexInvalid));
    let report = scan_roles(
        &catalog(),
        &SourceReader::local(SOURCE, dir.path()).unwrap(),
        SourceScope::CardRelative,
        &[],
        ScanLimits::default(),
    )
    .unwrap();
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == ScanIssueCode::ReaderUnavailable));
    assert_eq!(report.indexes_read, 0);
}

struct AlternativeReader {
    namespace: &'static str,
    foreign: bool,
}
impl IndexReader for AlternativeReader {
    fn reader_id(&self) -> &str {
        "test.alternative.index"
    }
    fn namespace(&self) -> &str {
        self.namespace
    }
    fn read(
        &self,
        root: &SourceReference,
        document: &IndexDocument,
    ) -> Result<Vec<GroupProposal>, String> {
        let root = if self.foreign {
            SourceReference::new(root.source_uri(), "OTHER").unwrap()
        } else {
            root.clone()
        };
        Ok(vec![GroupProposal {
            recording_identity: "alt-a".into(),
            evidence: GroupEvidence {
                reader_id: self.reader_id().into(),
                document: document.reference.clone(),
                locator: "test:item[1]".into(),
            },
            original: root.descendant("Clip/ALT.MXF").unwrap(),
            root,
            proxies: vec![],
            related: vec![],
        }])
    }
}

#[test]
fn another_reader_is_pluggable_without_changing_the_scanner() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "PRIVATE/XDROOT/Clip/ALT.MXF", "alt media");
    write(dir.path(), "PRIVATE/XDROOT/MEDIAPRO.XML", "fixture index");
    let mut catalog = catalog();
    catalog.patterns.retain(|p| p.id == "sony-xdroot-sd");
    catalog.patterns[0].id = "test-fixture-layout".into();
    for m in &mut catalog.patterns[0].metadata {
        m.namespace = "urn:qnc:test:index".into();
    }
    let source = SourceReader::local(SOURCE, dir.path()).unwrap();
    let reader = AlternativeReader {
        namespace: "urn:qnc:test:index",
        foreign: false,
    };
    let report = scan_roles(
        &catalog,
        &source,
        SourceScope::CardRelative,
        &[&reader],
        ScanLimits::default(),
    )
    .unwrap();
    assert!(report.relationships_resolved());
    assert_eq!(
        report.grouping.groups[0].proposal.evidence.reader_id,
        reader.reader_id()
    );
    let bad = AlternativeReader {
        foreign: true,
        ..reader
    };
    let report = scan_roles(
        &catalog,
        &source,
        SourceScope::CardRelative,
        &[&bad],
        ScanLimits::default(),
    )
    .unwrap();
    assert!(report.grouping.groups.is_empty());
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == ScanIssueCode::IndexInvalid));
}

#[test]
fn competing_reader_capabilities_are_reported_not_selected_arbitrarily() {
    let dir = fixture(&[]);
    let alternative = AlternativeReader {
        namespace: qnc_sony_metadata::INDEX_NAMESPACE,
        foreign: false,
    };
    let report = scan_roles(
        &catalog(),
        &SourceReader::local(SOURCE, dir.path()).unwrap(),
        SourceScope::CardRelative,
        &[&SonyIndexReader, &alternative],
        ScanLimits::default(),
    )
    .unwrap();
    assert!(report.grouping.groups.is_empty());
    assert_eq!(report.indexes_read, 0);
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == ScanIssueCode::ReaderAmbiguous));
}

#[test]
fn empty_index_is_processed_without_fabricating_any_clip() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "PRIVATE/XDROOT/MEDIAPRO.XML",
        &format!(
            "<MediaProfile xmlns=\"{}\"><Contents/></MediaProfile>",
            qnc_sony_metadata::INDEX_NAMESPACE
        ),
    );
    let report = run(dir.path());
    assert!(report.relationships_resolved());
    assert!(report.grouping.groups.is_empty());
    assert_eq!(report.indexes_read, 1);
}

#[test]
fn group_and_file_limits_are_not_reported_as_success() {
    let dir = fixture(&[]);
    let source = SourceReader::local(SOURCE, dir.path()).unwrap();
    let report = scan_roles(
        &catalog(),
        &source,
        SourceScope::CardRelative,
        &[&SonyIndexReader],
        ScanLimits {
            max_groups: 1,
            ..ScanLimits::default()
        },
    )
    .unwrap();
    assert!(!report.relationships_resolved());
    assert!(report.grouping.groups.is_empty());
    assert!(scan_roles(
        &catalog(),
        &source,
        SourceScope::CardRelative,
        &[&SonyIndexReader],
        ScanLimits {
            max_file_checks: 1,
            ..ScanLimits::default()
        }
    )
    .is_err());
}

#[test]
fn scanner_uses_identical_contract_for_lan_and_intranet_sources() {
    let dir = fixture(&[]);
    for environment in ["lan", "intranet"] {
        let uri = format!("qnc://{environment}/storage/source/card");
        let source = qnc_source_reader::LocalSource::new(&uri, dir.path()).unwrap();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let handle = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    qnc_source_reader::server::respond(request, &source, "test-token");
                }
            }
        });
        let result = scan_roles(
            &catalog(),
            &SourceReader::remote(&uri, &endpoint, "test-token").unwrap(),
            SourceScope::CardRelative,
            &[&SonyIndexReader],
            ScanLimits::default(),
        );
        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        let report = result.unwrap();
        assert!(report.relationships_resolved());
        assert_eq!(report.grouping.groups.len(), 2);
        assert!(report
            .grouping
            .groups
            .iter()
            .flat_map(|g| g.proposal.references())
            .all(|r| r.source_uri() == uri));
        assert_eq!(report.file_facts.len(), 8);
    }
}

#[test]
fn public_modules_have_no_app_store_probe_or_built_in_camera_dependency() {
    assert!(qnc_contracts::validate_module_manifest_json(
        "scanner",
        include_str!("../../../contracts/modules/scanner.module.json")
    )
    .is_ok());
    let production = include_str!("../Cargo.toml")
        .split("[dev-dependencies]")
        .next()
        .unwrap();
    for forbidden in [
        "qnc-ingest",
        "qnc-project",
        "qnc-sony",
        "rusqlite",
        "qnc-media-probe",
        "eframe",
    ] {
        assert!(!production.contains(forbidden));
    }
}
