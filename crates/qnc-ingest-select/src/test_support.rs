use crate::*;
use qnc_ingest_store::content::{Access, ContentClient};
use qnc_media_probe::{ProbeBackend, Request as ProbeRequest};
use serde_json::json;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

const SOURCE: &str = "qnc://local/source/card-test";
const INDEX: &str = include_str!("../../qnc-sony-metadata/tests/fixtures/MEDIAPRO.XML");
const SIDE: &str = include_str!("../../qnc-sony-metadata/tests/fixtures/TEST-AM01.XML");

fn local(uri: &str, file: &Path) -> Binding {
    Binding {
        uri: uri.into(),
        file: Some(file.into()),
        endpoint: None,
        token_env: None,
    }
}

pub fn fixture() -> (tempfile::TempDir, SelectionConfig) {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("card");
    let recording = card.join("PRIVATE/XDROOT");
    for name in ["Clip", "Sub", "Thmbnl"] {
        std::fs::create_dir_all(recording.join(name)).unwrap();
    }
    std::fs::write(recording.join("MEDIAPRO.XML"), INDEX).unwrap();
    for name in [
        "Clip/TEST A.MXF",
        "Clip/TEST B.MXF",
        "Sub/TEST AS03.MP4",
        "Sub/TEST BS03.MP4",
        "Thmbnl/TEST AT01.JPG",
    ] {
        std::fs::write(recording.join(name), "not media; fixture backend only").unwrap();
    }
    std::fs::write(recording.join("Clip/TEST AM01.XML"), SIDE).unwrap();
    let mut jpg = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(32, 18, image::Rgb([200, 40, 80]))
        .write_to(&mut jpg, image::ImageFormat::Jpeg)
        .unwrap();
    std::fs::write(recording.join("Thmbnl/TEST AT01.JPG"), jpg.into_inner()).unwrap();
    std::fs::write(
        recording.join("Clip/TEST BM01.XML"),
        SIDE.replace("ORIGINAL-A", "ORIGINAL-B")
            .replace("500", "100"),
    )
    .unwrap();
    let config = SelectionConfig {
        version: "0.1.0".into(),
        parallelism: 2,
        catalog: local(
            "qnc://local/catalog/camera-patterns",
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../catalogs/camera-patterns/camera-patterns-2026.09.07.1.sqlite"),
        ),
        sources: vec![SourceConfig {
            location: local(SOURCE, &card),
            name: "Card".into(),
            serial_number: String::new(),
            volume_name: String::new(),
            scope: qnc_camera_detector::SourceScope::CardRelative,
            probe: ProbeConfig::Local {
                executable: dir.path().join("never-executed"),
                probe_size_bytes: 8388608,
                analyze_duration_us: 1000000,
            },
        }],
    };
    drop(
        ContentClient::from_owner_binding(
            &dir.path().join("content.db"),
            "qnc://local/db/ingest_content/p1",
            Access::ReadWrite,
        )
        .unwrap(),
    );
    (dir, config)
}

struct Backend {
    calls: Arc<AtomicUsize>,
    fail: bool,
    partial: bool,
}

impl ProbeBackend for Backend {
    fn execute(&self, request: &ProbeRequest) -> qnc_media_probe::Result<qnc_media_probe::Report> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(qnc_media_probe::Error::Timeout);
        }
        let a = request.media_uri.contains("TEST%20A");
        let frames = if a { 500 } else { 100 };
        let mut video = json!({"index":0,"codec_type":"video","codec_name":"h264","width":1920,"height":1080,
            "time_base":"1/50","start_pts":0,"duration_ts":frames,"avg_frame_rate":"50/1","r_frame_rate":"50/1","nb_frames":frames.to_string(),
            "field_order":"progressive","pix_fmt":"yuv422p","sample_aspect_ratio":"1:1",
            "color_primaries":"bt709","color_transfer":"bt709","color_space":"bt709","color_range":"tv"});
        if self.partial {
            video.as_object_mut().unwrap().remove("sample_aspect_ratio");
        }
        let json = json!({"streams":[video],"format":{"filename":request.media_uri,
            "format_name": if request.media_uri.contains("/Sub/") { "mov,mp4,m4a,3gp,3g2,mj2" } else {"mxf"},
            "duration":(frames/50).to_string()},"programs":[],"chapters":[]}).to_string();
        Ok(qnc_media_probe::Report {
            request_id: request.request_id.clone(),
            media_uri: request.media_uri.clone(),
            document_uri: request.document_uri.clone(),
            json,
            elapsed_ms: 1,
        })
    }
}

/// A camera adapter for tests: the Sony reader with a chosen metadata sufficiency.
pub struct TestCamera(pub MetadataSufficiency);

const TEST_PATTERNS: &[&str] = &["test-sony"];

impl qnc_camera_adapter::CameraAdapter for TestCamera {
    fn adapter_id(&self) -> &str {
        "camera.test.sony"
    }
    fn pattern_ids(&self) -> &[&'static str] {
        TEST_PATTERNS
    }
    fn index(&self) -> &dyn qnc_source_groups::IndexReader {
        &qnc_sony_metadata::SonyIndexReader
    }
    fn documents(&self, group: &qnc_source_groups::GroupProposal) -> Vec<SourceReference> {
        qnc_sony_metadata::metadata_references(group)
    }
    fn thumbnail(&self, group: &qnc_source_groups::GroupProposal) -> Option<SourceReference> {
        qnc_sony_metadata::thumbnail_reference(group)
    }
    fn metadata(
        &self,
        clip_id: &str,
        group: &qnc_source_groups::GroupProposal,
        documents: &[IndexDocument],
    ) -> std::result::Result<qnc_media_record_db::contract::ClipMetadata, String> {
        qnc_sony_metadata::read_group_metadata(clip_id, group, documents)
    }
    fn sufficiency(&self, _: &qnc_media_record_db::contract::ClipMetadata) -> MetadataSufficiency {
        self.0
    }
}

pub fn registry(sufficiency: MetadataSufficiency) -> CameraRegistry {
    let mut registry = CameraRegistry::new();
    registry
        .register(Arc::new(TestCamera(sufficiency)))
        .unwrap();
    registry
}

/// Runs Select with a camera that needs one probe (the behaviour before cameras declared metadata).
pub fn execute(
    config: &SelectionConfig,
    calls: &Arc<AtomicUsize>,
    fail: bool,
    partial: bool,
    path: &str,
) -> Vec<Event> {
    execute_with(
        config,
        calls,
        fail,
        partial,
        path,
        &registry(MetadataSufficiency::NeedsProbe),
    )
}

pub fn execute_with(
    config: &SelectionConfig,
    calls: &Arc<AtomicUsize>,
    fail: bool,
    partial: bool,
    path: &str,
    registry: &CameraRegistry,
) -> Vec<Event> {
    let (send, receive) = mpsc::sync_channel(128);
    run_inner(
        config,
        &SourceReference::new(SOURCE, path).unwrap(),
        &send,
        &AtomicBool::new(false),
        |_, _| {
            Ok(Box::new(Backend {
                calls: calls.clone(),
                fail,
                partial,
            }))
        },
        select_target(config),
        registry,
    )
    .unwrap();
    drop(send);
    receive.into_iter().collect()
}

/// The project database of the fixture: content, media records and source index.
pub fn content_file(config: &SelectionConfig) -> std::path::PathBuf {
    config.sources[0]
        .location
        .file
        .as_ref()
        .and_then(|card| card.parent())
        .expect("fixture card directory")
        .join("content.db")
}

/// The fixture project database: project settings of project p1, so the one
/// intermediary of the project database accepts it (it is never created there).
pub fn project_database(file: &Path) {
    let conn = rusqlite::Connection::open(file).unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = PERSIST;
         CREATE TABLE IF NOT EXISTS project_settings (project_id TEXT);
         INSERT INTO project_settings SELECT 'p1' WHERE NOT EXISTS (SELECT 1 FROM project_settings);
         CREATE VIEW IF NOT EXISTS public_project_settings AS SELECT project_id FROM project_settings;",
    )
    .unwrap();
}

/// Content and records of the project database at `file`.
pub fn select_target_at(file: &Path) -> crate::SelectTarget {
    project_database(file);
    crate::SelectTarget {
        content: ContentTarget::from_owner_binding(file, "qnc://local/db/ingest_content/p1").unwrap(),
        records: qnc_db_broker::ProjectDbTarget::from_owner_binding(file, "qnc://local/db/project_db/p1")
            .unwrap(),
    }
}

/// The project database of the fixture as a Select target.
pub fn select_target(config: &SelectionConfig) -> crate::SelectTarget {
    select_target_at(&content_file(config))
}
