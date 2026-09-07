use super::*;
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

const LOCAL: &str = "qnc://local/catalog/camera-patterns";
const TOKEN: &str = "test-catalog-token";

fn publication(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../catalogs/camera-patterns")
        .join(name)
}

fn read(path: &Path) -> Result<Catalog> {
    read_uri(
        &ResolverConfig::new(PathBuf::new()).with_local_binding(LOCAL, path),
        LOCAL,
        None,
    )
}

fn catalog() -> Catalog {
    read(&publication("camera-patterns-2026.09.07.1.sqlite")).unwrap()
}

struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(handler: impl Fn(tiny_http::Request) + Send + 'static) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let thread = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(20)).unwrap() {
                    handler(request);
                }
            }
        });
        Self {
            url,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

#[test]
fn published_catalogs_are_read_only_and_revisions_are_not_migrated() {
    for (file, version, selectors) in [
        ("camera-patterns-v1.sqlite", "2026.09.06.1", 7),
        ("camera-patterns-2026.09.07.1.sqlite", "2026.09.07.1", 28),
    ] {
        let path = publication(file);
        let before = fs::read(&path).unwrap();
        let snapshot = read(&path).unwrap();
        assert_eq!(snapshot.dataset_version, version);
        assert_eq!(snapshot.patterns.len(), 23);
        assert_eq!(
            snapshot
                .patterns
                .iter()
                .filter(|p| p.analysis_candidate())
                .count(),
            17
        );
        assert_eq!(snapshot.gaps.len(), 7);
        assert_eq!(
            snapshot
                .patterns
                .iter()
                .find(|p| p.id == "sony-xdroot-sd")
                .unwrap()
                .metadata
                .len(),
            selectors
        );
        assert_eq!(fs::read(path).unwrap(), before);
        let wire = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(serde_json::from_str::<Catalog>(&wire).unwrap(), snapshot);
    }
}

#[test]
fn malformed_database_is_rejected_before_executing_modified_views() {
    for sql in [
        "PRAGMA application_id=0;",
        "PRAGMA user_version=99;",
        "DROP VIEW public_patterns; CREATE VIEW public_patterns AS WITH RECURSIVE loop(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM loop) SELECT * FROM loop;",
        "UPDATE catalog SET dataset_version='';",
        "DELETE FROM evidence WHERE pattern_id='sony-xdroot-sd';",
        "UPDATE pattern_root SET relative_pattern='../escape' WHERE pattern_id='sony-xdroot-sd';",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.sqlite");
        fs::copy(publication("camera-patterns-2026.09.07.1.sqlite"), &path).unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        let before = fs::read(&path).unwrap();
        assert!(read(&path).is_err(), "accepted {sql}");
        assert_eq!(fs::read(path).unwrap(), before);
    }
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing.sqlite");
    assert!(read(&missing).is_err());
    assert!(!missing.exists());
}

#[test]
fn unsafe_or_unknown_pattern_semantics_are_not_silently_accepted() {
    for path in [
        "",
        "/Clip",
        "../Clip",
        "Clip/../a",
        "C:/Clip",
        "Clip\\a",
        "Clip/%2e",
        "Clip/{a,b}",
        "Clip/[ab]",
        "Clip/ab**",
        "Clip//a",
    ] {
        assert!(validate_expression(path).is_err(), "{path}");
    }
    for path in [
        ".",
        "PRIVATE/XDROOT",
        "*.RDM/*.RDC/*.R3D",
        "**/INDEX.DAT",
        "DCIM/???GOPRO",
    ] {
        assert!(validate_expression(path).is_ok(), "{path}");
    }
    let valid = catalog();
    for case in 0..8 {
        let mut bad = valid.clone();
        let pattern = bad
            .patterns
            .iter_mut()
            .find(|p| p.id == "sony-xdroot-sd")
            .unwrap();
        match case {
            0 => pattern.status = "published".into(),
            1 => pattern.evidence_level = "partial".into(),
            2 => pattern.evidence.clear(),
            3 => pattern.files[0].role = "execute".into(),
            4 => pattern.roots.push(pattern.roots[0].clone()),
            5 => pattern.files.retain(|f| f.role != "original_candidate"),
            6 => bad.contract_version = "99".into(),
            _ => bad.catalog_uri = "qnc://local/catalog/other".into(),
        }
        assert!(bad.validate().is_err(), "case {case}");
    }
}

#[test]
fn inactive_patterns_and_coverage_gaps_remain_visible_in_projection() {
    let mut snapshot = catalog();
    let p = snapshot
        .patterns
        .iter_mut()
        .find(|p| p.id == "sony-xdroot-sd")
        .unwrap();
    p.status = "incorrect".into();
    assert!(!p.analysis_candidate());
    assert!(snapshot.validate().is_ok());
    assert_eq!(snapshot.gaps.len(), 7);
    assert!(snapshot.patterns.iter().any(|p| p.status == "disabled"));
}

#[test]
fn lan_and_intranet_return_exact_same_validated_catalog() {
    for environment in ["lan", "intranet"] {
        let uri = format!("qnc://{environment}/storage/catalog/camera-patterns");
        let mut expected = catalog();
        expected.catalog_uri = uri.clone();
        let served = expected.clone();
        let server = Server::new(move |request| respond(request, &served, TOKEN));
        let resolver = ResolverConfig::new(PathBuf::new())
            .with_lan_authority("storage", &server.url)
            .with_intranet_authority("storage", &server.url);
        assert_eq!(read_uri(&resolver, &uri, Some(TOKEN)).unwrap(), expected);
        assert!(read_uri(&resolver, &uri, Some("wrong")).is_err());
        assert!(read_uri(&resolver, &uri, None).is_err());
    }
}

#[test]
fn remote_rejects_foreign_identity_unknown_schema_and_insecure_endpoint() {
    let uri = "qnc://lan/storage/catalog/camera-patterns";
    for case in 0..4 {
        let mut snapshot = catalog();
        snapshot.catalog_uri = uri.into();
        match case {
            0 => snapshot.catalog_uri = LOCAL.into(),
            1 => snapshot.schema_version = 99,
            2 => snapshot.patterns[0].roots.push("../outside".into()),
            _ => snapshot.catalog_id = "other".into(),
        }
        let bytes = serde_json::to_string(&snapshot).unwrap();
        let server = Server::new(move |request| {
            let _ = request.respond(tiny_http::Response::from_string(&bytes).with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
            ));
        });
        let resolver =
            ResolverConfig::new(PathBuf::new()).with_lan_authority("storage", &server.url);
        assert!(read_uri(&resolver, uri, Some(TOKEN)).is_err());
    }
    for url in [
        "http://192.0.2.1",
        "https://user:secret@example.invalid",
        "https://example.invalid?path=x",
    ] {
        let resolver = ResolverConfig::new(PathBuf::new()).with_lan_authority("storage", url);
        assert!(read_uri(&resolver, uri, Some(TOKEN)).is_err());
    }
}

#[test]
fn endpoint_rejects_extra_fields_versions_and_non_json_requests() {
    let mut snapshot = catalog();
    let uri = "qnc://lan/storage/catalog/camera-patterns";
    snapshot.catalog_uri = uri.into();
    let server = Server::new(move |request| respond(request, &snapshot, TOKEN));
    let request = || {
        ureq::post(&format!("{}{ENDPOINT}", server.url))
            .set("Authorization", &format!("Bearer {TOKEN}"))
    };
    for body in [
        serde_json::json!({"version": "99", "catalog_uri": uri}),
        serde_json::json!({"version": VERSION, "catalog_uri": uri, "sql": "DROP TABLE pattern"}),
    ] {
        assert!(matches!(
            request().send_json(body),
            Err(ureq::Error::Status(400, _))
        ));
    }
    assert!(matches!(
        request().send_string("{}"),
        Err(ureq::Error::Status(415, _))
    ));
    assert!(matches!(
        request()
            .set("Content-Type", "application/json")
            .send_string(&" ".repeat(4097)),
        Err(ureq::Error::Status(413, _))
    ));
}

#[test]
fn manifest_preserves_read_only_public_module_boundary() {
    assert!(qnc_contracts::validate_module_manifest_json(
        "camera-patterns",
        include_str!("../../../contracts/modules/camera-patterns.module.json")
    )
    .is_ok());
}
