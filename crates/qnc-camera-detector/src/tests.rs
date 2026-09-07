use super::*;
use qnc_source_reader::LocalSource;
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

fn catalog() -> Catalog {
    let uri = "qnc://local/catalog/camera-patterns";
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite");
    let resolver = ResolverConfig::new(PathBuf::new()).with_local_binding(uri, path);
    qnc_camera_patterns::read_uri(&resolver, uri, None).unwrap()
}

fn fixture(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in files {
        let path = dir.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"placeholder; detector must not decode media").unwrap();
    }
    dir
}

fn run(root: &Path) -> DetectionReport {
    detect(
        &catalog(),
        &SourceReader::local("qnc://local/source/card", root).unwrap(),
        SourceScope::CardRelative,
        Limits::default(),
    )
    .unwrap()
}

fn root<'a>(report: &'a DetectionReport, id: &str) -> &'a RootFinding {
    report.roots.iter().find(|r| r.pattern_id == id).unwrap()
}

#[test]
fn sony_roles_follow_recording_structure_not_extension_alone() {
    let fixture = fixture(&[
        "PRIVATE/XDROOT/Clip/A.MXF",
        "PRIVATE/XDROOT/Sub/AS03.MP4",
        "PRIVATE/XDROOT/Clip/AM01.XML",
        "PRIVATE/XDROOT/Thmbnl/AT01.JPG",
        "PRIVATE/XDROOT/MEDIAPRO.XML",
        "unrelated/A.MXF",
        "PRIVATE/XDROOT/Sub/not-proxy.MP4",
    ]);
    let report = run(fixture.path());
    assert!(report.traversal_complete, "{:?}", report.issues);
    let sony = root(&report, "sony-xdroot-sd");
    assert_eq!(sony.root.relative_path(), "PRIVATE/XDROOT");
    assert!(sony.has_original_candidates());
    assert_eq!(sony.files.len(), 5);
    for (role, suffix) in [
        ("original_candidate", "Clip/A.MXF"),
        ("proxy_candidate", "Sub/AS03.MP4"),
        ("metadata", "Clip/AM01.XML"),
        ("thumbnail", "Thmbnl/AT01.JPG"),
        ("index", "MEDIAPRO.XML"),
    ] {
        assert!(sony
            .files
            .iter()
            .any(|f| f.role == role && f.reference.relative_path().ends_with(suffix)));
    }
    assert!(report
        .roots
        .iter()
        .flat_map(|r| &r.files)
        .all(|f| !f.reference.relative_path().starts_with("unrelated")));
    assert_eq!(report.coverage_gap_ids.len(), 7);
}

#[test]
fn proxy_is_optional_and_proxy_only_root_is_not_a_clip_candidate() {
    for (files, expected) in [
        (&["PRIVATE/XDROOT/Clip/A.MXF"][..], true),
        (&["PRIVATE/XDROOT/Sub/AS03.MP4"][..], false),
    ] {
        let fixture = fixture(files);
        let report = run(fixture.path());
        assert!(report.traversal_complete);
        let sony = root(&report, "sony-xdroot-sd");
        assert_eq!(sony.has_original_candidates(), expected);
        assert_eq!(
            sony.files
                .iter()
                .filter(|f| f.role == "original_candidate")
                .count(),
            usize::from(expected)
        );
    }
}

#[test]
fn documented_p2_gopro_and_canon_structures_keep_distinct_roles() {
    let fixture = fixture(&[
        "CONTENTS/VIDEO/0001.MXF",
        "CONTENTS/AUDIO/000100.MXF",
        "CONTENTS/CLIP/0001.XML",
        "CONTENTS/PROXY/0001.MOV",
        "DCIM/100GOPRO/GH010001.MP4",
        "DCIM/100GOPRO/GL010001.LRV",
        "DCIM/100GOPRO/Proxies/GH010001_Proxy.MP4",
        "DCIM/100___01/MVI_0001.MOV",
        "DCIM/100___01/MVI_0001.THM",
        "DCIM/100___01/wrong.MOV",
    ]);
    let report = run(fixture.path());
    assert!(report.traversal_complete, "{:?}", report.issues);
    let p2 = root(&report, "panasonic-p2-separate");
    assert_eq!(p2.files.len(), 4);
    assert_eq!(
        p2.files
            .iter()
            .filter(|f| f.role == "original_candidate")
            .count(),
        1
    );
    assert!(p2.files.iter().any(|f| f.role == "audio_component"));
    let gopro = root(&report, "gopro-standard-lrv");
    assert_eq!(gopro.files.len(), 2);
    assert!(gopro.files.iter().any(|f| f.role == "preview"));
    let canon = root(&report, "canon-dcim-legacy");
    assert_eq!(
        canon
            .files
            .iter()
            .filter(|f| f.role == "original_candidate")
            .count(),
        1
    );
    assert!(canon.files.iter().any(|f| f.role == "thumbnail"));
}

#[test]
fn conflicting_original_proxy_patterns_are_reported_not_first_match_wins() {
    let fixture = fixture(&[
        "A001.RDM/A001_C001.RDC/A001_001.R3D",
        "A001.RDM/A001_C001.RDC/A001_001.mov",
    ]);
    let report = run(fixture.path());
    let conflict = report
        .ambiguities
        .iter()
        .find(|a| a.file_uri.ends_with("mov"))
        .unwrap();
    assert!(conflict
        .claims
        .iter()
        .any(|c| c.role == "original_candidate"));
    assert!(conflict.claims.iter().any(|c| c.role == "proxy_candidate"));
    assert!(root(&report, "red-r3d-prores").has_original_candidates());
    assert!(root(&report, "red-prores-original").has_original_candidates());
}

#[test]
fn all_alternative_roots_and_zero_or_multiple_recursive_segments_are_matched() {
    let fixture = fixture(&[
        "BPAV/CLPR/A/A.MP4",
        "PRIVATE/JVC/BPAV/CLPR/B/B.MP4",
        "PRIVATE/PANA_GRP/INDEX.DAT",
        "PRIVATE/PANA_GRP/001/INDEX.DAT",
        "PRIVATE/PANA_GRP/001/nested/INDEX.DAT",
        "PRIVATE/PANA_GRP/001/A.MOV",
    ]);
    let report = run(fixture.path());
    assert!(report.traversal_complete, "{:?}", report.issues);
    assert_eq!(
        report
            .roots
            .iter()
            .filter(|r| r.pattern_id == "jvc-prohd-bpav" && r.has_original_candidates())
            .count(),
        2
    );
    let pana = root(&report, "panasonic-pana-grp");
    assert_eq!(pana.files.iter().filter(|f| f.role == "index").count(), 3);
}

#[test]
fn transport_case_policy_controls_matching_and_preserves_actual_spelling() {
    let fixture = fixture(&["private/xdroot/clip/A.mxf"]);
    for policy in [MatchCase::Exact, MatchCase::Insensitive] {
        let source = SourceReader::from_local(
            LocalSource::new("qnc://local/source/card", fixture.path())
                .unwrap()
                .with_match_case(policy),
        );
        let report = detect(
            &catalog(),
            &source,
            SourceScope::CardRelative,
            Limits::default(),
        )
        .unwrap();
        let sony = report
            .roots
            .iter()
            .find(|r| r.pattern_id == "sony-xdroot-sd");
        if policy == MatchCase::Exact {
            assert!(sony.is_none());
        } else {
            assert_eq!(
                sony.unwrap().files[0].reference.relative_path(),
                "private/xdroot/clip/A.mxf"
            );
        }
    }
}

#[test]
fn source_scope_and_pattern_status_are_explicit_not_guessed() {
    let fixture = fixture(&["A.braw", "Proxy/A.mp4", "PRIVATE/XDROOT/Clip/A.MXF"]);
    let source = SourceReader::local("qnc://local/source/card", fixture.path()).unwrap();
    let report = detect(
        &catalog(),
        &source,
        SourceScope::RecordingRelative,
        Limits::default(),
    )
    .unwrap();
    assert!(root(&report, "blackmagic-ursa-cine").has_original_candidates());
    assert!(report
        .roots
        .iter()
        .all(|r| r.pattern_id != "sony-xdroot-sd"));
    for status in ["disabled", "incorrect"] {
        let mut catalog = catalog();
        catalog
            .patterns
            .iter_mut()
            .find(|p| p.id == "sony-xdroot-sd")
            .unwrap()
            .status = status.into();
        let report = detect(
            &catalog,
            &source,
            SourceScope::CardRelative,
            Limits::default(),
        )
        .unwrap();
        assert!(report
            .roots
            .iter()
            .all(|r| r.pattern_id != "sony-xdroot-sd"));
        assert!(report
            .exclusions
            .iter()
            .any(|e| e.pattern_id == "sony-xdroot-sd"));
    }
}

#[test]
fn exhausted_budgets_produce_incomplete_reports_not_empty_success() {
    let fixture = fixture(&["PRIVATE/XDROOT/Clip/A.MXF", "PRIVATE/XDROOT/Clip/B.MXF"]);
    let source = SourceReader::local("qnc://local/source/card", fixture.path()).unwrap();
    for limits in [
        Limits {
            max_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_directories: 1,
            ..Limits::default()
        },
        Limits {
            max_matches: 1,
            ..Limits::default()
        },
        Limits {
            max_steps: 1,
            ..Limits::default()
        },
        Limits {
            max_entries_per_directory: 1,
            ..Limits::default()
        },
    ] {
        let report = detect(&catalog(), &source, SourceScope::CardRelative, limits).unwrap();
        assert!(!report.traversal_complete);
        assert!(!report.issues.is_empty());
    }
}

#[test]
fn candidate_catalog_and_listing_are_used_over_real_http_without_app_processes() {
    let fixture = fixture(&["PRIVATE/XDROOT/Clip/A.MXF", "PRIVATE/XDROOT/Sub/AS03.MP4"]);
    let expected = run(fixture.path());
    for environment in ["lan", "intranet"] {
        let catalog_uri = format!("qnc://{environment}/storage/catalog/camera-patterns");
        let source_uri = format!("qnc://{environment}/storage/source/card");
        let mut served = catalog();
        served.catalog_uri = catalog_uri.clone();
        let source = LocalSource::new(&source_uri, fixture.path()).unwrap();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let handle = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    if request.url() == qnc_camera_patterns::ENDPOINT {
                        qnc_camera_patterns::respond(request, &served, "token");
                    } else {
                        qnc_source_reader::server::respond(request, &source, "token");
                    }
                }
            }
        });
        let resolver = ResolverConfig::new(PathBuf::new())
            .with_lan_authority("storage", &url)
            .with_intranet_authority("storage", &url);
        let result = qnc_camera_patterns::read_uri(&resolver, &catalog_uri, Some("token"))
            .and_then(|catalog| {
                let source =
                    SourceReader::remote(&source_uri, &url, "token").map_err(|e| e.to_string())?;
                detect(
                    &catalog,
                    &source,
                    SourceScope::CardRelative,
                    Limits::default(),
                )
            });
        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        let report = result.unwrap();
        assert!(report.traversal_complete);
        assert_eq!(report.directories_listed, expected.directories_listed);
        assert_eq!(report.roots.len(), expected.roots.len());
        let actual = root(&report, "sony-xdroot-sd");
        assert_eq!(
            actual.files.len(),
            root(&expected, "sony-xdroot-sd").files.len()
        );
        assert!(actual
            .files
            .iter()
            .all(|f| f.reference.uri().starts_with(&source_uri)));
    }
}

#[test]
fn listing_cache_does_not_repeat_reads_for_each_rule() {
    let fixture = fixture(&["PRIVATE/XDROOT/Clip/A.MXF", "PRIVATE/XDROOT/Clip/AM01.XML"]);
    let mut catalog = catalog();
    catalog.patterns.retain(|p| p.id == "sony-xdroot-sd");
    let report = detect(
        &catalog,
        &SourceReader::local("qnc://local/source/card", fixture.path()).unwrap(),
        SourceScope::CardRelative,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(report.directories_listed, 4); // Root, PRIVATE, XDROOT and Clip, once each.
    assert_eq!(root(&report, "sony-xdroot-sd").files.len(), 2);
}

#[test]
fn manifest_is_public_and_has_no_probe_or_application_dependency() {
    assert!(qnc_contracts::validate_module_manifest_json(
        "camera-detector",
        include_str!("../../../contracts/modules/camera-detector.module.json")
    )
    .is_ok());
    let cargo: &str = include_str!("../Cargo.toml");
    let production = cargo.split("[dev-dependencies]").next().unwrap();
    assert!(!production.contains("qnc-ingest"));
    assert!(!production.contains("qnc-project"));
    assert!(!production.contains("qnc-sony-metadata"));
}
