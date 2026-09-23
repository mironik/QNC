//! Public read-only port for the active project.
//!
//! This module keeps no active-project cache. Each `read` call goes back
//! through the public work-settings DB/transport contract.

use std::path::Path;

use qnc_content_read::{CatalogSignature, ContentReader};
use qnc_work_settings::{ReadError, SettingsReader, WorkSettings, WorkspaceBinding};

pub const MODULE_ID: &str = "qnc.module.active-project-read";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveProjectSnapshot {
    pub project_id: String,
    pub project_uri: String,
    pub workspace_db_uri: String,
    pub settings: WorkSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownProject {
    pub project_id: String,
    pub catalog_signature: CatalogSignature,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActiveProjectChange {
    Same(ActiveProjectSnapshot, CatalogSignature),
    ProjectChanged(ActiveProjectSnapshot, CatalogSignature),
    SignatureChanged(ActiveProjectSnapshot, CatalogSignature),
    NoActiveProject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveProjectError {
    pub code: String,
    pub message: String,
}

impl ActiveProjectError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ActiveProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ActiveProjectError {}

impl From<ReadError> for ActiveProjectError {
    fn from(error: ReadError) -> Self {
        Self {
            code: error.code,
            message: error.message,
        }
    }
}

#[derive(Clone)]
pub struct ActiveProjectReader {
    settings: SettingsReader,
}

impl std::fmt::Debug for ActiveProjectReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveProjectReader")
            .finish_non_exhaustive()
    }
}

impl ActiveProjectReader {
    pub fn from_root(root: &Path) -> Result<Self, ActiveProjectError> {
        Ok(Self {
            settings: SettingsReader::from_root(root)?,
        })
    }

    pub fn from_settings_reader(settings: SettingsReader) -> Self {
        Self { settings }
    }

    /// Reads the active project from the DB now. No result from a previous call
    /// is retained as truth.
    pub fn read(&self) -> Result<ActiveProjectSnapshot, ActiveProjectError> {
        let settings = self.settings.read()?;
        Ok(ActiveProjectSnapshot {
            project_id: settings.project_id.clone(),
            project_uri: settings.output_root_uri.clone(),
            workspace_db_uri: settings.workspace_db_uri.clone(),
            settings,
        })
    }

    pub fn workspace_binding(
        &self,
        snapshot: &ActiveProjectSnapshot,
    ) -> Result<WorkspaceBinding, ActiveProjectError> {
        self.settings
            .workspace_binding(&snapshot.settings)
            .map_err(Into::into)
    }

    pub fn catalog_signature(
        &self,
        snapshot: &ActiveProjectSnapshot,
    ) -> Result<CatalogSignature, ActiveProjectError> {
        ContentReader::for_project(&self.settings, &snapshot.settings)
            .and_then(|reader| reader.signature())
            .map_err(|message| ActiveProjectError::new("catalog_read", message))
    }

    pub fn compare(
        &self,
        shown: Option<&ShownProject>,
    ) -> Result<ActiveProjectChange, ActiveProjectError> {
        let snapshot = match self.read() {
            Ok(snapshot) => snapshot,
            Err(error) if error.code == "no_active_project" => {
                return Ok(ActiveProjectChange::NoActiveProject);
            }
            Err(error) => return Err(error),
        };
        let signature = self.catalog_signature(&snapshot)?;
        Ok(match shown {
            Some(shown) if shown.project_id == snapshot.project_id => {
                if shown.catalog_signature == signature {
                    ActiveProjectChange::Same(snapshot, signature)
                } else {
                    ActiveProjectChange::SignatureChanged(snapshot, signature)
                }
            }
            _ => ActiveProjectChange::ProjectChanged(snapshot, signature),
        })
    }

    pub fn settings_reader(&self) -> &SettingsReader {
        &self.settings
    }
}

pub fn read_from_root(root: &Path) -> Result<ActiveProjectSnapshot, ActiveProjectError> {
    ActiveProjectReader::from_root(root)?.read()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn project_settings_json() -> String {
        r#"{
                "storage": {
                    "ingest_profile": "field",
                    "ingest_media": "link",
                    "proxy_policy": "link_when_available",
                    "original_policy": "link_when_available"
                },
                "products": {
                    "root": "products",
                    "thumbnails": "products/thumbnails",
                    "filmstrip": "products/filmstrip",
                    "virtual_shorts": "products/virtual_shorts",
                    "virtual_segments": "products/virtual_segments",
                    "b_roll_virtual_clips": "products/b_roll_virtual_clips"
                },
                "input": {"mode": "auto"},
                "playback": {
                    "input": "proxy_if_available",
                    "decoder": "catalog"
                },
                "video": {"fps": 50.0},
                "audio": {
                    "channels": 2,
                    "sample_rate": 48000
                },
                "ai": {"enabled": false},
                "keyboard_shortcuts": {"active_preset": "default"}
            }"#
        .to_string()
    }

    fn workspace(root: &Path, id: &str, name: &str) {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("project.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE project_settings(project_id TEXT, settings_json TEXT);
             CREATE VIEW public_project_settings AS SELECT project_id, settings_json FROM project_settings;
             CREATE TABLE clips (clip_id TEXT PRIMARY KEY, name TEXT, created_at_utc TEXT, import_status TEXT DEFAULT 'detected',
                 duration_seconds REAL, duration_frames INTEGER, imported_media_uri TEXT, selected INTEGER DEFAULT 0);
             CREATE VIEW public_clips AS SELECT clip_id,name,created_at_utc,duration_seconds,
                 duration_frames,imported_media_uri,import_status,selected FROM clips;",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_settings VALUES(?1, ?2)",
            rusqlite::params![id, project_settings_json()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clips VALUES(?1, ?2, '2026-09-23T00:00:00Z', 'imported', 1.0, 50, NULL, 0)",
            rusqlite::params![format!("clip-{id}"), format!("Clip {id}")],
        )
        .unwrap();
        let registry = Connection::open(root.join("data").join("qnc-projects.db")).unwrap();
        registry
            .execute(
                "INSERT INTO projects(project_id, name, project_uri) VALUES(?1, ?2, ?3)",
                rusqlite::params![id, name, format!("qnc://local/project/{id}")],
            )
            .unwrap();
        registry
            .execute(
                "INSERT INTO project_storage_locations(project_id, local_path) VALUES(?1, ?2)",
                rusqlite::params![id, dir.to_string_lossy()],
            )
            .unwrap();
    }

    fn root() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let registry = Connection::open(data.join("qnc-projects.db")).unwrap();
        registry
            .execute_batch(
                "CREATE TABLE app_settings(key TEXT,value TEXT);
                 CREATE VIEW public_app_settings AS SELECT key, value FROM app_settings;
                 CREATE TABLE projects(project_id TEXT, name TEXT, project_uri TEXT);
                 CREATE VIEW public_projects AS SELECT project_id, name, project_uri FROM projects;
                 CREATE TABLE project_storage_locations(project_id TEXT, local_path TEXT);
                 INSERT INTO app_settings VALUES('active_project_id','p1');",
            )
            .unwrap();
        workspace(root.path(), "p1", "Project 1");
        workspace(root.path(), "p2", "Project 2");
        root
    }

    #[test]
    fn rereads_the_active_project_each_time() {
        let root = root();
        let reader = ActiveProjectReader::from_root(root.path()).unwrap();
        assert_eq!(reader.read().unwrap().project_id, "p1");

        let registry = Connection::open(root.path().join("data").join("qnc-projects.db")).unwrap();
        registry
            .execute(
                "UPDATE app_settings SET value='p2' WHERE key='active_project_id'",
                [],
            )
            .unwrap();

        let snapshot = reader.read().unwrap();
        assert_eq!(snapshot.project_id, "p2");
        assert_eq!(
            snapshot.workspace_db_uri,
            "qnc://local/db/project_workspace/p2"
        );
        assert_eq!(snapshot.project_uri, "qnc://local/project/p2");
    }

    #[test]
    fn reports_missing_active_project_without_fallback() {
        let root = root();
        let registry = Connection::open(root.path().join("data").join("qnc-projects.db")).unwrap();
        registry
            .execute(
                "UPDATE app_settings SET value='' WHERE key='active_project_id'",
                [],
            )
            .unwrap();

        let error = ActiveProjectReader::from_root(root.path())
            .unwrap()
            .read()
            .unwrap_err();
        assert_eq!(error.code, "no_active_project");
    }

    #[test]
    fn compare_reports_same_project_catalog_changes_and_project_changes() {
        let root = root();
        let reader = ActiveProjectReader::from_root(root.path()).unwrap();
        let ActiveProjectChange::ProjectChanged(first, signature) = reader.compare(None).unwrap()
        else {
            panic!("initial active project is new to the caller");
        };
        assert_eq!(first.project_id, "p1");
        let shown = ShownProject {
            project_id: first.project_id.clone(),
            catalog_signature: signature.clone(),
        };
        assert!(matches!(
            reader.compare(Some(&shown)).unwrap(),
            ActiveProjectChange::Same(_, _)
        ));

        let conn = Connection::open(root.path().join("p1").join("project.db")).unwrap();
        conn.execute(
            "INSERT INTO clips VALUES('clip-extra', 'Extra', '2026-09-23T00:00:01Z', 'imported', 2.0, 100, NULL, 0)",
            [],
        )
        .unwrap();
        assert!(matches!(
            reader.compare(Some(&shown)).unwrap(),
            ActiveProjectChange::SignatureChanged(_, _)
        ));

        let registry = Connection::open(root.path().join("data").join("qnc-projects.db")).unwrap();
        registry
            .execute(
                "UPDATE app_settings SET value='p2' WHERE key='active_project_id'",
                [],
            )
            .unwrap();
        assert!(matches!(
            reader.compare(Some(&shown)).unwrap(),
            ActiveProjectChange::ProjectChanged(_, _)
        ));
    }

    #[test]
    fn compare_reports_no_active_project_without_defaulting() {
        let root = root();
        let registry = Connection::open(root.path().join("data").join("qnc-projects.db")).unwrap();
        registry
            .execute("DELETE FROM app_settings WHERE key='active_project_id'", [])
            .unwrap();
        assert_eq!(
            ActiveProjectReader::from_root(root.path())
                .unwrap()
                .compare(None)
                .unwrap(),
            ActiveProjectChange::NoActiveProject
        );
    }
}
