use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::env;
#[cfg(any(windows, target_os = "macos"))]
use std::process::{Command, Stdio};

use qnc_application_catalog::SelectionSnapshot;
use qnc_contracts::parse_qnc_uri;
use qnc_db_contract::DatabaseContract;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

mod project_origin;
use project_origin::ProjectOrigin;

const PROJECT_REGISTRY_DB_CONTRACT: &str =
    include_str!("../../../contracts/databases/project-registry.database.json");
const PROJECT_WORKSPACE_DB_CONTRACT: &str =
    include_str!("../../../contracts/databases/project-workspace.database.json");
const SYSTEM_SEED_JSON: &str = include_str!("../../../seed/system_seed.json");
const APPLICATION_ID: &str = "qnc.project";
const PROJECT_REGISTRY_DB_URI: &str = "qnc://local/db/project_registry";
const SELECTED_TEMPLATE_KEY: &str = "selected_template_id";
const PROJECTS_ROOT_KEY: &str = "projects_root_path";
const DEFAULT_EXPORT_DIRECTORY: &str = "exports/projekti";
const PROJECT_DIRECTORIES: &[&str] = &[
    "proxy",
    "original",
    "audio",
    "incoming/card",
    "incoming/ftp",
    "ingest/thumbnails",
    "filmstrip",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project_id: String,
    pub name: String,
    pub created_date: String,
    pub project_uri: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNavigationStep {
    pub application_id: String,
    pub tab_id: String,
    pub priority_group: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTemplateRow {
    pub template_id: String,
    pub name: String,
    pub description: String,
    pub system: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectTemplateSettings {
    pub template_id: String,
    pub settings: Value,
}

#[derive(Debug, Clone)]
pub struct ProjectStore {
    root: PathBuf,
    data_dir: PathBuf,
    projects_root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SeedFile {
    #[serde(default)]
    source_templates: Vec<Value>,
    #[serde(default)]
    project_templates: Vec<Value>,
}

#[derive(Debug, Clone)]
struct StoredTemplate {
    template_id: String,
    name: String,
    system: bool,
    settings: Value,
    source_template_ids: Value,
}

impl ProjectStore {
    pub fn validate_embedded_contracts() -> Result<(), String> {
        validate_db_contracts()
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Self, String> {
        validate_db_contracts()?;
        let root = root.into();
        let data_dir = root.join("data");
        let projects_root = root.join("projects");
        let mut store = Self {
            root,
            data_dir,
            projects_root,
        };
        let conn = store.open_registry()?;
        init_registry_schema(&conn)?;
        ensure_templates_seeded(&conn)?;
        if let Some(projects_root) = get_local_setting(&conn, PROJECTS_ROOT_KEY)? {
            store.projects_root = PathBuf::from(projects_root);
        }
        ensure_selected_template(&conn)?;
        store.lock_registered_project_dirs(&conn)?;
        Ok(store)
    }

    pub fn projects_root_display(&self) -> String {
        self.projects_root.to_string_lossy().to_string()
    }

    pub fn set_projects_root(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err("Lokacija projekata je prazna.".to_string());
        }
        fs::create_dir_all(path).map_err(|error| error.to_string())?;
        let conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        let value = path.to_string_lossy().to_string();
        set_local_setting(&conn, PROJECTS_ROOT_KEY, &value)?;
        self.projects_root = PathBuf::from(value);
        lock_projects_root_dir(&self.projects_root)?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRow>, String> {
        let conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        let active_project_id = get_setting(&conn, "active_project_id", "")?;
        let mut statement = conn
            .prepare(
                "SELECT project_id, name, project_uri,
                    COALESCE(strftime('%d.%m.%Y.', substr(created_at, 7), 'unixepoch', 'localtime'), '')
                 FROM projects
                 ORDER BY created_at, name, project_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let project_id: String = row.get(0)?;
                Ok(ProjectRow {
                    active: project_id == active_project_id,
                    project_id,
                    name: row.get(1)?,
                    project_uri: row.get(2)?,
                    created_date: row.get(3)?,
                })
            })
            .map_err(|error| error.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn list_project_templates(&self) -> Result<Vec<ProjectTemplateRow>, String> {
        let conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        ensure_templates_seeded(&conn)?;
        let selected_template_id = selected_template_id(&conn)?;
        let mut statement = conn
            .prepare(
                "SELECT template_id, name, description, system
                 FROM project_templates
                 ORDER BY system DESC, name",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let template_id: String = row.get(0)?;
                Ok(ProjectTemplateRow {
                    selected: template_id == selected_template_id,
                    template_id,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    system: row.get::<_, i64>(3)? != 0,
                })
            })
            .map_err(|error| error.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn set_selected_template(&self, template_id: &str) -> Result<(), String> {
        let template_id = template_id.trim();
        if template_id.is_empty() {
            return Err("template_id je prazan.".to_string());
        }
        let conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        ensure_templates_seeded(&conn)?;
        if get_template(&conn, template_id)?.is_none() {
            return Err(format!("Template '{template_id}' ne postoji."));
        }
        set_local_setting(&conn, SELECTED_TEMPLATE_KEY, template_id)
    }

    pub fn selected_template_settings(&self) -> Result<ProjectTemplateSettings, String> {
        let conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        ensure_templates_seeded(&conn)?;
        let template_id = selected_template_id(&conn)?;
        let template = get_template(&conn, &template_id)?
            .ok_or_else(|| format!("Template '{template_id}' ne postoji."))?;
        Ok(ProjectTemplateSettings {
            template_id,
            settings: template.settings,
        })
    }

    pub fn create_user_template(
        &self,
        name: &str,
        description: &str,
        base_template_id: &str,
        settings: Option<&Value>,
        selection: &SelectionSnapshot,
    ) -> Result<ProjectTemplateRow, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Naziv templatea je prazan.".to_string());
        }
        let conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        ensure_templates_seeded(&conn)?;
        let base_template_id = if base_template_id.trim().is_empty() {
            selected_template_id(&conn)?
        } else {
            base_template_id.trim().to_string()
        };
        let base = get_template(&conn, &base_template_id)?
            .ok_or_else(|| format!("Template '{base_template_id}' ne postoji."))?;
        let mut settings = settings.cloned().unwrap_or_else(|| base.settings.clone());
        apply_application_selection(&mut settings, selection)?;
        let template_id = format!("tpl_user_{}", slug_id(name));
        let now = now_str();
        let settings_json = json_string(&settings)?;
        let source_template_ids_json = json_string(&base.source_template_ids)?;
        conn.execute(
            "INSERT INTO project_templates
                (template_id, name, description, system, settings_json, source_template_ids_json,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?6)",
            params![
                template_id,
                name,
                description.trim(),
                settings_json,
                source_template_ids_json,
                now
            ],
        )
        .map_err(|error| error.to_string())?;
        set_local_setting(&conn, SELECTED_TEMPLATE_KEY, &template_id)?;
        Ok(ProjectTemplateRow {
            template_id,
            name: name.to_string(),
            description: description.trim().to_string(),
            system: false,
            selected: true,
        })
    }

    pub fn delete_user_template(&self, template_id: &str) -> Result<(), String> {
        let template_id = template_id.trim();
        if template_id.is_empty() {
            return Err("template_id je prazan.".to_string());
        }

        let mut conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        ensure_templates_seeded(&conn)?;
        let template = get_template(&conn, template_id)?
            .ok_or_else(|| format!("Template '{template_id}' ne postoji."))?;
        if template.system {
            return Err("System template se ne može obrisati.".to_string());
        }

        let selected_before = selected_template_id(&conn)?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        tx.execute(
            "DELETE FROM project_template_kv WHERE template_id = ?1",
            params![template_id],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "DELETE FROM project_template_sources WHERE template_id = ?1",
            params![template_id],
        )
        .map_err(|error| error.to_string())?;
        let deleted = tx
            .execute(
                "DELETE FROM project_templates WHERE template_id = ?1 AND system = 0",
                params![template_id],
            )
            .map_err(|error| error.to_string())?;
        if deleted == 0 {
            return Err(format!("Template '{template_id}' nije obrisan."));
        }
        if selected_before == template_id {
            tx.execute(
                "DELETE FROM local_runtime_settings WHERE key = ?1",
                params![SELECTED_TEMPLATE_KEY],
            )
            .map_err(|error| error.to_string())?;
        }
        tx.commit().map_err(|error| error.to_string())?;
        ensure_selected_template(&conn)
    }

    pub fn create_project(
        &self,
        name: &str,
        template_id: &str,
        settings: Option<&Value>,
        selection: &SelectionSnapshot,
    ) -> Result<ProjectRow, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Naziv projekta je prazan.".to_string());
        }

        let mut conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        ensure_templates_seeded(&conn)?;
        let template_id = if template_id.trim().is_empty() {
            selected_template_id(&conn)?
        } else {
            template_id.trim().to_string()
        };
        let mut template = get_template(&conn, &template_id)?
            .ok_or_else(|| format!("Template '{template_id}' ne postoji."))?;
        if let Some(settings) = settings {
            template.settings = settings.clone();
        }
        apply_application_selection(&mut template.settings, selection)?;
        let project_id = format!("{}_{}", slug_base(name), uuid::Uuid::new_v4().simple());
        let project_uri = format!("qnc://local/project/{project_id}");
        parse_qnc_uri(&project_uri)?;
        let now = now_str();
        let created_date = project_origin::display_date(&conn, &now)?;
        // The public reader runs on the originating workstation, outside DB transactions.
        let origin = ProjectOrigin {
            name: name.to_string(),
            created_at: now.clone(),
            identity: qnc_workstation_identity::read_local_identity(),
        };
        let projects_root = self.create_projects_root(&conn, &template.settings)?;
        let project_dir = projects_root.join(safe_dir_name(&project_id));
        let export_path = project_export_directory(&project_dir, &mut template.settings)?;
        fs::create_dir_all(&projects_root).map_err(|error| error.to_string())?;
        let confirmed_root = projects_root
            .canonicalize()
            .map_err(|error| error.to_string())?;
        unlock_projects_root_dir(&projects_root)?;
        let mut created_directory = false;
        let result: Result<ProjectRow, String> = (|| {
            // Claim a new directory before writing; never reuse or clean an existing one.
            fs::create_dir(&project_dir).map_err(|error| error.to_string())?;
            created_directory = true;
            if let Some(export_path) = &export_path {
                fs::create_dir_all(export_path).map_err(|error| {
                    format!(
                        "Ne mogu kreirati export direktorij '{}': {error}",
                        export_path.display()
                    )
                })?;
            }
            self.ensure_workspace_db(&project_id, &project_dir, Some(&template), &origin)?;
            lock_project_dir(&project_dir)?;
            lock_projects_root_dir(&projects_root)?;

            // Publish only a prepared workspace, in one short registry transaction.
            let tx = conn.transaction().map_err(|error| error.to_string())?;
            tx.execute(
                "INSERT INTO projects
                    (project_id, name, project_uri, created_at, updated_at, last_opened_at)
                 VALUES (?1, ?2, ?3, ?4, ?4, ?4)",
                params![project_id, name, project_uri, now],
            )
            .map_err(|error| error.to_string())?;
            project_origin::insert(&tx, &project_id, &origin)?;
            tx.execute(
                "INSERT INTO project_storage_locations
                    (project_id, local_path, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)",
                params![project_id, project_dir.to_string_lossy().to_string(), now],
            )
            .map_err(|error| error.to_string())?;
            set_local_setting(&tx, PROJECTS_ROOT_KEY, &projects_root.to_string_lossy())?;
            set_setting(&tx, "active_project_id", &project_id)?;
            tx.commit().map_err(|error| error.to_string())?;
            Ok(ProjectRow {
                project_id,
                name: name.to_string(),
                created_date,
                project_uri,
                active: true,
            })
        })();
        match result {
            Ok(row) => Ok(row),
            Err(mut error) => {
                if created_directory {
                    if let Err(cleanup_error) =
                        remove_unpublished_project(&confirmed_root, &project_dir)
                    {
                        error.push_str(&format!(
                            "; uklanjanje nedovrsenog projekta: {cleanup_error}"
                        ));
                    }
                }
                if let Err(lock_error) = lock_projects_root_dir(&projects_root) {
                    error.push_str(&format!("; {lock_error}"));
                }
                Err(error)
            }
        }
    }

    fn create_projects_root(&self, conn: &Connection, settings: &Value) -> Result<PathBuf, String> {
        let requested = settings_path_string(settings, &["storage", "projects_root"]);
        let location = if requested.is_empty() || requested == "$QNC_STANDARD_PROJECTS_ROOT" {
            get_local_setting(conn, PROJECTS_ROOT_KEY)?
                .map(PathBuf::from)
                .unwrap_or_else(|| self.projects_root.clone())
        } else {
            if requested.starts_with("qnc://") {
                return Err("Lokacija projekta zahtijeva konfiguriran transport binding; nema lokalnog fallbacka.".to_string());
            }
            if requested.starts_with('$') {
                return Err(format!("Nepoznata lokacija projekta: {requested}"));
            }
            PathBuf::from(requested)
        };
        if location.is_absolute() {
            Ok(location)
        } else if location
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            Err("Relativna lokacija projekta ne smije izlaziti iz QNC direktorija.".to_string())
        } else {
            Ok(self.root.join(location))
        }
    }

    pub fn navigation_sequence(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectNavigationStep>, String> {
        let registry = Connection::open_with_flags(
            self.project_registry_db_path()?,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|error| error.to_string())?;
        let dir = project_dir_from_conn(&registry, &self.projects_root, project_id)?;
        let db = Connection::open_with_flags(
            self.project_workspace_db_path(project_id, &dir)?,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|error| error.to_string())?;
        let sql = "SELECT application_id, tab_id, priority_group FROM public_project_application_sequence WHERE project_id=?1 ORDER BY position";
        let mut stmt = db.prepare(sql).map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([project_id], |row| {
                Ok(ProjectNavigationStep {
                    application_id: row.get(0)?,
                    tab_id: row.get(1)?,
                    priority_group: row.get(2)?,
                })
            })
            .map_err(|error| error.to_string())?;
        let steps = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        if steps.first().is_none_or(|step| step.priority_group != "a")
            || steps.iter().any(|step| {
                step.priority_group.len() != 1
                    || !step.priority_group.as_bytes()[0].is_ascii_lowercase()
                    || !step.application_id.starts_with("qnc.")
                    || step.tab_id.is_empty()
            })
            || steps
                .windows(2)
                .any(|pair| pair[0].priority_group >= pair[1].priority_group)
        {
            return Err("Projekt nema valjan novi slijed prioritetnih grupa.".into());
        }
        Ok(steps)
    }

    pub fn open_project(&self, project_id: &str) -> Result<(), String> {
        let project_id = project_id.trim();
        if project_id.is_empty() {
            return Err("project_id je prazan.".to_string());
        }
        let conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if exists == 0 {
            return Err(format!("Projekt '{project_id}' ne postoji."));
        }
        self.navigation_sequence(project_id)?;
        let project_dir = project_dir_from_conn(&conn, &self.projects_root, project_id)?;
        let workspace_db = self.project_workspace_db_path(project_id, &project_dir)?;
        if !workspace_db.is_file() {
            return Err(format!(
                "Direktorij projekta '{}' nije ispravan. Nema automatskog popravljanja.",
                project_dir.display()
            ));
        }
        let now = now_str();
        conn.execute(
            "UPDATE projects
             SET updated_at = ?2, last_opened_at = ?2
             WHERE project_id = ?1",
            params![project_id, now],
        )
        .map_err(|error| error.to_string())?;
        lock_project_dir(&project_dir)?;
        self.lock_registered_project_dirs(&conn)?;
        set_setting(&conn, "active_project_id", project_id)?;
        Ok(())
    }

    pub fn delete_project(&self, project_id: &str) -> Result<ProjectRow, String> {
        let project_id = project_id.trim();
        if project_id.is_empty() {
            return Err("project_id je prazan.".to_string());
        }

        let mut conn = self.open_registry()?;
        init_registry_schema(&conn)?;
        let project = conn
            .query_row(
                "SELECT project_id, name, project_uri,
                    COALESCE(strftime('%d.%m.%Y.', substr(created_at, 7), 'unixepoch', 'localtime'), '')
                 FROM projects
                 WHERE project_id = ?1",
                params![project_id],
                |row| {
                    Ok(ProjectRow {
                        active: false,
                        project_id: row.get(0)?,
                        name: row.get(1)?,
                        project_uri: row.get(2)?,
                        created_date: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("Projekt '{project_id}' ne postoji."))?;
        let project_dir = project_dir_from_conn(&conn, &self.projects_root, project_id)?;
        let workspace_db = self.project_workspace_db_path(project_id, &project_dir)?;
        validate_project_delete_dir(project_id, &project_dir, &workspace_db)?;
        let projects_root = project_dir
            .parent()
            .ok_or("Projekt nema parent direktorij.")?;

        let active_project_id = get_setting(&conn, "active_project_id", "")?;
        let next_active_project_id = if active_project_id == project_id {
            conn.query_row(
                "SELECT project_id
                 FROM projects
                 WHERE project_id <> ?1
                 ORDER BY project_id
                 LIMIT 1",
                params![project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
        } else {
            None
        };

        let tx = conn.transaction().map_err(|error| error.to_string())?;
        tx.execute(
            "DELETE FROM project_origin WHERE project_id = ?1",
            params![project_id],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "DELETE FROM project_storage_locations WHERE project_id = ?1",
            params![project_id],
        )
        .map_err(|error| error.to_string())?;
        tx.execute(
            "DELETE FROM projects WHERE project_id = ?1",
            params![project_id],
        )
        .map_err(|error| error.to_string())?;
        let next_active_project_id = next_active_project_id.unwrap_or_default();
        if active_project_id == project_id {
            tx.execute(
                "INSERT INTO app_settings (key, value)
                 VALUES ('active_project_id', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![&next_active_project_id],
            )
            .map_err(|error| error.to_string())?;
        }
        if project_dir.exists() {
            unlock_projects_root_dir(projects_root)?;
            if let Err(error) = unlock_project_dir(&project_dir) {
                let _ = lock_projects_root_dir(projects_root);
                return Err(error);
            }
            let remove_result = fs::remove_dir_all(&project_dir).map_err(|error| {
                format!(
                    "Ne mogu obrisati direktorij projekta '{}': {error}",
                    project_dir.display()
                )
            });
            let lock_result = lock_projects_root_dir(projects_root);
            remove_result?;
            lock_result?;
        }
        tx.commit().map_err(|error| error.to_string())?;
        self.lock_registered_project_dirs(&conn)?;

        Ok(project)
    }

    fn open_registry(&self) -> Result<Connection, String> {
        fs::create_dir_all(&self.data_dir).map_err(|error| error.to_string())?;
        let db_path = self.project_registry_db_path()?;
        let conn = Connection::open(db_path).map_err(|error| error.to_string())?;
        configure_connection(&conn)?;
        Ok(conn)
    }

    fn lock_registered_project_dirs(&self, conn: &Connection) -> Result<(), String> {
        for (_project_id, project_dir) in project_storage_paths(conn)? {
            lock_project_dir(&project_dir)?;
        }
        lock_projects_root_dir(&self.projects_root)?;
        Ok(())
    }

    fn ensure_workspace_db(
        &self,
        project_id: &str,
        dir: &Path,
        template: Option<&StoredTemplate>,
        origin: &ProjectOrigin,
    ) -> Result<(), String> {
        ensure_project_dirs_at(dir)?;
        let db_path = self.project_workspace_db_path(project_id, dir)?;
        let mut conn = Connection::open(db_path).map_err(|error| error.to_string())?;
        configure_connection(&conn)?;
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        init_workspace_schema(&tx)?;
        project_origin::insert(&tx, project_id, origin)?;
        let template_id = template
            .map(|template| template.template_id.as_str())
            .unwrap_or("");
        let settings = template
            .map(|template| template.settings.clone())
            .unwrap_or(Value::Object(Default::default()));
        let settings_json = json_string(&settings)?;
        tx.execute(
            "INSERT INTO project_settings
                (project_id, template_id, settings_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(project_id) DO NOTHING",
            params![project_id, template_id, settings_json, origin.created_at],
        )
        .map_err(|error| error.to_string())?;
        if let Some(template) = template {
            save_template_snapshot(&tx, project_id, template)?;
            write_project_workflow(&tx, project_id, &template.settings)?;
        }
        tx.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn project_registry_db_path(&self) -> Result<PathBuf, String> {
        let resolver = ResolverConfig::new(&self.root).with_local_binding(
            PROJECT_REGISTRY_DB_URI,
            self.data_dir.join("project_store.db"),
        );
        resolve_local_path(&resolver, PROJECT_REGISTRY_DB_URI)
    }

    fn project_workspace_db_path(
        &self,
        project_id: &str,
        project_dir: &Path,
    ) -> Result<PathBuf, String> {
        let uri = project_workspace_db_uri(project_id);
        let resolver = ResolverConfig::new(&self.root)
            .with_local_binding(uri.clone(), project_dir.join("qnc_project.db"));
        resolve_local_path(&resolver, &uri)
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn ensure_project_dirs_at(base: &Path) -> Result<(), String> {
    fs::create_dir_all(base).map_err(|error| error.to_string())?;
    for subdir in PROJECT_DIRECTORIES {
        fs::create_dir_all(base.join(subdir)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn project_export_directory(
    project_dir: &Path,
    settings: &mut Value,
) -> Result<Option<PathBuf>, String> {
    let mut export_directory = settings_path_string(settings, &["export", "directory"]);
    if export_directory.trim().is_empty() {
        export_directory = settings_path_string(settings, &["export", "output_directory"]);
    }
    if export_directory.trim().is_empty() {
        export_directory = DEFAULT_EXPORT_DIRECTORY.to_string();
        set_settings_string_path(settings, &["export", "directory"], DEFAULT_EXPORT_DIRECTORY);
    }

    let export_directory = export_directory.trim();
    if export_directory.starts_with("qnc://") {
        parse_qnc_uri(export_directory)?;
        return Ok(None);
    }

    export_directory_path(project_dir, export_directory).map(Some)
}

fn remove_unpublished_project(projects_root: &Path, project_dir: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(project_dir).map_err(|error| error.to_string())?;
    let resolved = project_dir
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || resolved.parent() != Some(projects_root) {
        return Err(
            "Direktorij za uklanjanje nije unutar potvrdene lokacije projekta.".to_string(),
        );
    }
    unlock_projects_root_dir(projects_root)?;
    unlock_project_dir(project_dir)?;
    fs::remove_dir_all(project_dir).map_err(|error| error.to_string())
}

fn export_directory_path(project_dir: &Path, export_directory: &str) -> Result<PathBuf, String> {
    let export_path = PathBuf::from(export_directory);
    if export_path.is_absolute() {
        return Ok(export_path);
    }
    if export_path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "Export direktorij mora biti relativan unutar projekta ili apsolutan: {export_directory}"
        ));
    }
    Ok(project_dir.join(export_path))
}

fn settings_path_string(settings: &Value, path: &[&str]) -> String {
    let mut current = settings;
    for segment in path {
        let Some(next) = current.get(*segment) else {
            return String::new();
        };
        current = next;
    }
    current.as_str().unwrap_or("").trim().to_string()
}

fn set_settings_string_path(settings: &mut Value, path: &[&str], value: &str) {
    if path.is_empty() {
        return;
    }

    let mut current = settings;
    for segment in &path[..path.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Default::default());
        }
        let object = current.as_object_mut().expect("settings object");
        current = object
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }

    if !current.is_object() {
        *current = Value::Object(Default::default());
    }
    current.as_object_mut().expect("settings object").insert(
        path[path.len() - 1].to_string(),
        Value::String(value.to_string()),
    );
}

fn project_dir_from_conn(
    conn: &Connection,
    projects_root: &Path,
    project_id: &str,
) -> Result<PathBuf, String> {
    let row: Option<String> = conn
        .query_row(
            "SELECT local_path FROM project_storage_locations WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(local_path) = row.filter(|value| !value.trim().is_empty()) {
        return Ok(PathBuf::from(local_path));
    }
    Ok(projects_root.join(safe_dir_name(project_id)))
}

fn validate_project_delete_dir(
    project_id: &str,
    project_dir: &Path,
    workspace_db: &Path,
) -> Result<(), String> {
    let expected_leaf = safe_dir_name(project_id);
    let actual_leaf = project_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if actual_leaf != expected_leaf {
        return Err(format!(
            "Brisanje zaustavljeno: direktorij projekta '{}' ne odgovara project_id '{}'.",
            project_dir.display(),
            project_id
        ));
    }
    if !project_dir.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(project_dir).map_err(|error| error.to_string())?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(format!(
            "Brisanje zaustavljeno: direktorij projekta '{}' je link.",
            project_dir.display()
        ));
    }
    if !file_type.is_dir() {
        return Err(format!(
            "Brisanje zaustavljeno: '{}' nije direktorij.",
            project_dir.display()
        ));
    }
    let has_workspace_db = workspace_db.is_file();
    let is_empty = fs::read_dir(project_dir)
        .map_err(|error| error.to_string())?
        .next()
        .is_none();
    if !has_workspace_db && !is_empty {
        return Err(format!(
            "Brisanje zaustavljeno: '{}' nema qnc_project.db.",
            project_dir.display()
        ));
    }
    Ok(())
}

fn project_storage_paths(conn: &Connection) -> Result<Vec<(String, PathBuf)>, String> {
    let mut statement = conn
        .prepare(
            "SELECT project_id, local_path
             FROM project_storage_locations
             WHERE local_path IS NOT NULL AND TRIM(local_path) <> ''
             ORDER BY project_id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PathBuf::from(row.get::<_, String>(1)?),
            ))
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn project_workspace_db_uri(project_id: &str) -> String {
    format!(
        "qnc://local/db/project_workspace/{}",
        safe_dir_name(project_id)
    )
}

fn resolve_local_path(resolver: &ResolverConfig, uri: &str) -> Result<PathBuf, String> {
    match resolver
        .resolve(uri)
        .map_err(|error| format!("Resolver: {error}"))?
        .endpoint
    {
        ResolvedEndpoint::LocalPath(path) => Ok(path),
        ResolvedEndpoint::NetworkEndpoint { .. } => Err(format!(
            "Resolver: '{uri}' nije lokalni filesystem endpoint."
        )),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn lock_projects_root_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        set_path_read_only(path, true)?;
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn unlock_projects_root_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        set_path_read_only(path, false)?;
    }
    Ok(())
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn lock_projects_root_dir(path: &Path) -> Result<(), String> {
    let _ = path;
    Ok(())
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn unlock_projects_root_dir(path: &Path) -> Result<(), String> {
    let _ = path;
    Ok(())
}

fn lock_project_dir(path: &Path) -> Result<(), String> {
    set_project_delete_lock(path, false)?;
    set_project_tree_read_only(path, true)?;
    set_project_hidden(path, true)?;
    set_project_delete_lock(path, true)
}

fn unlock_project_dir(path: &Path) -> Result<(), String> {
    set_project_delete_lock(path, false)?;
    set_project_hidden(path, false)?;
    set_project_tree_read_only(path, false)
}

fn set_project_tree_read_only(path: &Path, read_only: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Ok(());
    }
    if file_type.is_dir() {
        if !read_only {
            set_path_read_only(path, false)?;
        }
        for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            set_project_tree_read_only(&entry.path(), read_only)?;
        }
        if read_only {
            set_path_read_only(path, true)?;
        }
    } else {
        set_path_read_only(path, read_only)?;
    }
    Ok(())
}

#[cfg(windows)]
fn set_path_read_only(path: &Path, read_only: bool) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() == read_only {
        return Ok(());
    }
    permissions.set_readonly(read_only);
    fs::set_permissions(path, permissions).map_err(|error| {
        format!(
            "Ne mogu promijeniti read-only stanje za '{}': {error}",
            path.display()
        )
    })
}

#[cfg(unix)]
fn set_path_read_only(path: &Path, read_only: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let next_mode = if read_only {
        mode & !0o222
    } else {
        mode | 0o200
    };
    if mode == next_mode {
        return Ok(());
    }
    permissions.set_mode(next_mode);
    fs::set_permissions(path, permissions).map_err(|error| {
        format!(
            "Ne mogu promijeniti read-only stanje za '{}': {error}",
            path.display()
        )
    })
}

#[cfg(windows)]
fn set_project_delete_lock(path: &Path, locked: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let user = windows_current_user()?;
    let mut command = Command::new("icacls");
    command.arg(path);
    if locked {
        command.arg("/deny").arg(format!("{user}:(OI)(CI)(DE,DC)"));
    } else {
        command.arg("/remove:d").arg(user);
    }
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let status = command
        .status()
        .map_err(|error| format!("Ne mogu pokrenuti icacls za '{}': {error}", path.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Ne mogu promijeniti delete lock za '{}'.",
            path.display()
        ))
    }
}

#[cfg(windows)]
fn windows_current_user() -> Result<String, String> {
    let username = env::var("USERNAME")
        .map_err(|error| format!("Ne mogu procitati USERNAME za Windows ACL: {error}"))?;
    let domain = env::var("USERDOMAIN").unwrap_or_default();
    if domain.trim().is_empty() {
        Ok(username)
    } else {
        Ok(format!("{domain}\\{username}"))
    }
}

#[cfg(target_os = "macos")]
fn set_project_delete_lock(path: &Path, locked: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let flag = if locked { "uchg" } else { "nouchg" };
    let status = Command::new("chflags")
        .arg(flag)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("Ne mogu pokrenuti chflags za '{}': {error}", path.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Ne mogu promijeniti delete lock za '{}'.",
            path.display()
        ))
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn set_project_delete_lock(path: &Path, _locked: bool) -> Result<(), String> {
    let _ = path;
    Ok(())
}

#[cfg(windows)]
fn set_project_hidden(path: &Path, hidden: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let flag = if hidden { "+h" } else { "-h" };
    let status = Command::new("attrib")
        .arg(flag)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("Ne mogu pokrenuti attrib za '{}': {error}", path.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Ne mogu promijeniti hidden stanje za '{}'.",
            path.display()
        ))
    }
}

#[cfg(target_os = "macos")]
fn set_project_hidden(path: &Path, hidden: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let flag = if hidden { "hidden" } else { "nohidden" };
    let status = Command::new("chflags")
        .arg(flag)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("Ne mogu pokrenuti chflags za '{}': {error}", path.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Ne mogu promijeniti hidden stanje za '{}'.",
            path.display()
        ))
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn set_project_hidden(path: &Path, _hidden: bool) -> Result<(), String> {
    let _ = path;
    Ok(())
}

fn validate_db_contracts() -> Result<(), String> {
    for (name, contents) in [
        (
            "contracts/databases/project-registry.database.json",
            PROJECT_REGISTRY_DB_CONTRACT,
        ),
        (
            "contracts/databases/project-workspace.database.json",
            PROJECT_WORKSPACE_DB_CONTRACT,
        ),
    ] {
        let contract = DatabaseContract::from_json_str(name, contents)
            .map_err(|report| report.errors.join("; "))?;
        contract
            .validate_write(APPLICATION_ID)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

fn configure_connection(conn: &Connection) -> Result<(), String> {
    conn.busy_timeout(Duration::from_millis(5_000))
        .map_err(|error| error.to_string())?;
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|error| error.to_string())?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn init_registry_schema(conn: &Connection) -> Result<(), String> {
    project_origin::init_schema(conn)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS projects (
            project_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            project_uri TEXT NOT NULL,
            created_at TEXT,
            updated_at TEXT,
            created_by TEXT,
            updated_by TEXT,
            last_opened_at TEXT
        );
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS users (
            user_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'editor',
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            station_id TEXT NOT NULL,
            client_label TEXT NOT NULL DEFAULT '',
            created_at TEXT,
            last_seen_at TEXT,
            FOREIGN KEY(user_id) REFERENCES users(user_id)
        );
        CREATE TABLE IF NOT EXISTS source_templates (
            source_template_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            source_kind TEXT NOT NULL DEFAULT 'local',
            system INTEGER NOT NULL DEFAULT 0,
            config_json TEXT NOT NULL DEFAULT '{}',
            created_by TEXT,
            updated_by TEXT,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS project_templates (
            template_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            system INTEGER NOT NULL DEFAULT 0,
            settings_json TEXT NOT NULL DEFAULT '{}',
            source_template_ids_json TEXT NOT NULL DEFAULT '[]',
            created_by TEXT,
            updated_by TEXT,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS module_state (
            module_id TEXT PRIMARY KEY,
            enabled INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS local_runtime_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS project_storage_locations (
            project_id TEXT PRIMARY KEY,
            local_path TEXT NOT NULL,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS project_template_kv (
            template_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (template_id, setting_key)
        );
        CREATE TABLE IF NOT EXISTS project_template_sources (
            template_id TEXT NOT NULL,
            source_template_id TEXT NOT NULL,
            PRIMARY KEY (template_id, source_template_id)
        );
        CREATE TABLE IF NOT EXISTS source_template_kv (
            source_template_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (source_template_id, setting_key)
        );
        CREATE VIEW IF NOT EXISTS public_projects AS
            SELECT project_id, name, project_uri, created_at, updated_at, last_opened_at
            FROM projects;
        CREATE VIEW IF NOT EXISTS public_app_settings AS
            SELECT key, value FROM app_settings;
        CREATE VIEW IF NOT EXISTS public_project_templates AS
            SELECT template_id, name, description, system, created_at, updated_at
            FROM project_templates;
        CREATE VIEW IF NOT EXISTS public_source_templates AS
            SELECT source_template_id, name, description, source_kind, system, created_at, updated_at
            FROM source_templates;
        CREATE VIEW IF NOT EXISTS public_module_state AS
            SELECT module_id, enabled, updated_at FROM module_state;
        ",
    )
    .map_err(|error| error.to_string())
}

fn init_workspace_schema(conn: &Connection) -> Result<(), String> {
    project_origin::init_schema(conn)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS project_settings (
            project_id TEXT PRIMARY KEY,
            template_id TEXT,
            settings_json TEXT NOT NULL DEFAULT '{}',
            created_by TEXT,
            updated_by TEXT,
            created_at TEXT,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS project_members (
            project_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'editor',
            joined_at TEXT,
            last_seen_at TEXT,
            PRIMARY KEY(project_id, user_id)
        );
        CREATE TABLE IF NOT EXISTS project_template_snapshot (
            project_id TEXT PRIMARY KEY,
            template_id TEXT,
            template_name TEXT NOT NULL DEFAULT '',
            template_version TEXT NOT NULL DEFAULT '',
            snapshot_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT
        );
        CREATE TABLE IF NOT EXISTS project_workflow_steps (
            step_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            plugin_id TEXT NOT NULL,
            tab_id TEXT NOT NULL,
            label TEXT NOT NULL,
            position INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'locked',
            next_step_id TEXT,
            settings_json TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS project_workflow_state (
            project_id TEXT PRIMARY KEY,
            active_step_id TEXT,
            entry_step_id TEXT,
            updated_at TEXT
        );
        CREATE TABLE IF NOT EXISTS project_data_revisions (
            scope TEXT PRIMARY KEY,
            revision INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS project_settings_kv (
            project_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (project_id, setting_key)
        );
        CREATE TABLE IF NOT EXISTS project_snapshot_kv (
            project_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (project_id, setting_key)
        );
        CREATE TABLE IF NOT EXISTS project_workflow_step_kv (
            step_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (step_id, setting_key)
        );
        CREATE VIEW IF NOT EXISTS public_project_settings AS
            SELECT project_id, template_id, created_at, updated_at, settings_json FROM project_settings;
        CREATE VIEW IF NOT EXISTS public_project_members AS
            SELECT project_id, user_id, role, joined_at, last_seen_at FROM project_members;
        CREATE VIEW IF NOT EXISTS public_project_template_snapshot AS
            SELECT project_id, template_id, template_name, template_version, created_at
            FROM project_template_snapshot;
        CREATE VIEW IF NOT EXISTS public_project_workflow_steps AS
            SELECT step_id, project_id, plugin_id, tab_id, label, position, status, next_step_id
            FROM project_workflow_steps;
        CREATE VIEW IF NOT EXISTS public_project_workflow_state AS
            SELECT project_id, active_step_id, entry_step_id, updated_at
            FROM project_workflow_state;
        CREATE VIEW IF NOT EXISTS public_project_application_sequence AS
            SELECT project_id, step_id, plugin_id AS application_id, tab_id, label, position,
                json_extract(settings_json, '$.priority_group') AS priority_group,
                status, next_step_id
            FROM project_workflow_steps;
        CREATE VIEW IF NOT EXISTS public_project_data_revisions AS
            SELECT scope, revision, updated_at FROM project_data_revisions;
        ",
    )
    .map_err(|error| error.to_string())
}

fn ensure_templates_seeded(conn: &Connection) -> Result<(), String> {
    let seed: SeedFile = serde_json::from_str(SYSTEM_SEED_JSON).map_err(|error| {
        format!("seed/system_seed.json nije ispravan Project seed JSON: {error}")
    })?;
    let now = now_str();

    for source in seed.source_templates {
        let source_template_id = value_string(&source, "source_template_id");
        if source_template_id.is_empty() {
            continue;
        }
        let config = source.get("config").cloned().unwrap_or_else(|| json!({}));
        conn.execute(
            "INSERT INTO source_templates
                (source_template_id, name, description, source_kind, system, config_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?6)
             ON CONFLICT(source_template_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                source_kind = excluded.source_kind,
                system = 1,
                config_json = excluded.config_json,
                updated_at = excluded.updated_at",
            params![
                source_template_id,
                value_string_or(&source, "name", &source_template_id),
                value_string(&source, "description"),
                value_string_or(&source, "source_kind", "local"),
                json_string(&config)?,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    }

    for template in seed.project_templates {
        let template_id = value_string(&template, "template_id");
        if template_id.is_empty() {
            continue;
        }
        let settings = template
            .get("settings")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let source_template_ids = template
            .get("source_template_ids")
            .cloned()
            .unwrap_or_else(|| json!([]));
        conn.execute(
            "INSERT INTO project_templates
                (template_id, name, description, system, settings_json, source_template_ids_json,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?6)
             ON CONFLICT(template_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                system = 1,
                settings_json = excluded.settings_json,
                source_template_ids_json = excluded.source_template_ids_json,
                updated_at = excluded.updated_at",
            params![
                template_id,
                value_string_or(&template, "name", &template_id),
                value_string(&template, "description"),
                json_string(&settings)?,
                json_string(&source_template_ids)?,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn ensure_selected_template(conn: &Connection) -> Result<(), String> {
    let current = get_local_setting(conn, SELECTED_TEMPLATE_KEY)?;
    if let Some(current) = current {
        if get_template(conn, &current)?.is_some() {
            return Ok(());
        }
    }

    let first: Option<String> = conn
        .query_row(
            "SELECT template_id FROM project_templates ORDER BY system DESC, name LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(template_id) = first {
        set_local_setting(conn, SELECTED_TEMPLATE_KEY, &template_id)?;
    }
    Ok(())
}

fn selected_template_id(conn: &Connection) -> Result<String, String> {
    ensure_selected_template(conn)?;
    Ok(get_local_setting(conn, SELECTED_TEMPLATE_KEY)?.unwrap_or_default())
}

fn get_template(conn: &Connection, template_id: &str) -> Result<Option<StoredTemplate>, String> {
    let row = conn.query_row(
        "SELECT template_id, name, system, settings_json, source_template_ids_json
         FROM project_templates
         WHERE template_id = ?1",
        params![template_id.trim()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    );
    let Ok((template_id, name, system, settings_json, source_template_ids_json)) = row else {
        return Ok(None);
    };
    Ok(Some(StoredTemplate {
        template_id,
        name,
        system,
        settings: parse_json(&settings_json, json!({})),
        source_template_ids: parse_json(&source_template_ids_json, json!([])),
    }))
}

fn save_template_snapshot(
    conn: &Connection,
    project_id: &str,
    template: &StoredTemplate,
) -> Result<(), String> {
    let snapshot = json!({
        "template_id": template.template_id,
        "name": template.name,
        "system": template.system,
        "settings": template.settings,
        "source_template_ids": template.source_template_ids,
    });
    conn.execute(
        "INSERT INTO project_template_snapshot
            (project_id, template_id, template_name, template_version, snapshot_json, created_at)
         VALUES (?1, ?2, ?3, '', ?4, ?5)
         ON CONFLICT(project_id) DO UPDATE SET
            template_id = excluded.template_id,
            template_name = excluded.template_name,
            snapshot_json = excluded.snapshot_json",
        params![
            project_id,
            template.template_id,
            template.name,
            json_string(&snapshot)?,
            now_str()
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn apply_application_selection(
    settings: &mut Value,
    selection: &SelectionSnapshot,
) -> Result<(), String> {
    selection.validate().map_err(|error| error.to_string())?;
    if !selection
        .applications
        .iter()
        .any(|app| app.priority_group == "a")
    {
        return Err("Odaberite aplikaciju iz grupe a.".into());
    }
    if selection.applications.is_empty() {
        return Err("Odaberite barem jednu aplikaciju.".into());
    }
    let root = settings
        .as_object_mut()
        .ok_or("Postavke moraju biti objekt.")?;
    let workspace = root.entry("workspace").or_insert_with(|| json!({}));
    let workspace = workspace
        .as_object_mut()
        .ok_or("Workspace postavke moraju biti objekt.")?;
    workspace.insert(
        "application_selection".into(),
        serde_json::to_value(selection).map_err(|e| e.to_string())?,
    );
    workspace.insert(
        "tabs".into(),
        json!(selection
            .applications
            .iter()
            .map(|app| &app.tab_id)
            .collect::<Vec<_>>()),
    );
    workspace.insert(
        "tab_labels".into(),
        Value::Object(
            selection
                .applications
                .iter()
                .map(|app| (app.tab_id.clone(), json!(app.label)))
                .collect(),
        ),
    );
    Ok(())
}

fn write_project_workflow(
    conn: &Connection,
    project_id: &str,
    settings: &Value,
) -> Result<(), String> {
    let selection: SelectionSnapshot = settings
        .pointer("/workspace/application_selection")
        .ok_or("Nedostaje odabrani slijed aplikacija.")
        .and_then(|value| {
            serde_json::from_value(value.clone()).map_err(|_| "Neispravan slijed aplikacija.")
        })
        .map_err(|e| e.to_string())?;
    selection.validate().map_err(|e| e.to_string())?;
    if selection
        .applications
        .first()
        .is_none_or(|app| app.priority_group != "a")
    {
        return Err("Slijed mora poceti odabranom grupom a.".into());
    }
    conn.execute(
        "DELETE FROM project_workflow_steps WHERE project_id = ?1",
        params![project_id],
    )
    .map_err(|error| error.to_string())?;

    let mut entry_step_id = String::new();
    let mut previous_step_id = String::new();
    for (index, application) in selection.applications.iter().enumerate() {
        let tab_id = &application.tab_id;
        let metadata = serde_json::to_value(application).map_err(|e| e.to_string())?;
        let step_id = step_id_for_tab(tab_id);
        let initial_group = application.priority_group == "a";
        if !initial_group && entry_step_id.is_empty() {
            entry_step_id = step_id.clone();
        }
        if !previous_step_id.is_empty() {
            conn.execute(
                "UPDATE project_workflow_steps
                 SET next_step_id = ?2
                 WHERE project_id = ?1 AND step_id = ?3",
                params![project_id, step_id, previous_step_id],
            )
            .map_err(|error| error.to_string())?;
        }
        let status = if initial_group {
            "complete"
        } else if step_id == entry_step_id {
            "active"
        } else {
            "locked"
        };
        conn.execute(
            "INSERT INTO project_workflow_steps
                (step_id, project_id, plugin_id, tab_id, label, position, status, next_step_id, settings_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
            params![
                step_id,
                project_id,
                application.application_id,
                tab_id,
                application.label,
                index as i64,
                status,
                json_string(&metadata)?,
            ],
        )
        .map_err(|error| error.to_string())?;
        previous_step_id = step_id;
    }

    let active_step_id = if entry_step_id.is_empty() {
        step_id_for_tab(&selection.applications[0].tab_id)
    } else {
        entry_step_id.clone()
    };
    conn.execute(
        "INSERT INTO project_workflow_state
            (project_id, active_step_id, entry_step_id, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(project_id) DO UPDATE SET
            active_step_id = excluded.active_step_id,
            entry_step_id = excluded.entry_step_id,
            updated_at = excluded.updated_at",
        params![project_id, active_step_id, entry_step_id, now_str()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn step_id_for_tab(tab_id: &str) -> String {
    format!("step_{tab_id}")
}

fn get_local_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM local_runtime_settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok();
    Ok(row.filter(|value| !value.trim().is_empty()))
}

fn set_local_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO local_runtime_settings (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at",
        params![key, value, now_str()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn value_string(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn value_string_or(value: &Value, field: &str, fallback: &str) -> String {
    let value = value_string(value, field);
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn json_string(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn parse_json(raw: &str, fallback: Value) -> Value {
    serde_json::from_str(raw).unwrap_or(fallback)
}

fn get_setting(conn: &Connection, key: &str, default: &str) -> Result<String, String> {
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok();
    Ok(row.unwrap_or_else(|| default.to_string()))
}

fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (key, value)
         VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn now_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("epoch_{secs}")
}

fn slug_base(name: &str) -> String {
    let mut base: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    while base.contains("__") {
        base = base.replace("__", "_");
    }
    base = base.trim_matches('_').chars().take(40).collect();
    if base.is_empty() {
        base = "projekt".to_string();
    }
    base
}

fn slug_id(name: &str) -> String {
    let base = slug_base(name);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{base}_{millis}")
}

fn safe_dir_name(project_id: &str) -> String {
    let mut output: String = project_id
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if output.len() > 80 {
        output.truncate(80);
    }
    if output.is_empty() {
        "_invalid_project_id".to_string()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_settings_exposes_exact_saved_payload_without_changing_it() {
        let conn = Connection::open_in_memory().unwrap();
        init_workspace_schema(&conn).unwrap();
        let saved = r#"{"storage":{"ingest_media":"link"},"input":{"mode":"auto"}}"#;
        conn.execute(
            "INSERT INTO project_settings(project_id, settings_json) VALUES ('p1', ?1)",
            [saved],
        )
        .unwrap();
        let public: String = conn
            .query_row(
                "SELECT settings_json FROM public_project_settings WHERE project_id='p1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(public, saved);
    }

    fn test_selection(tabs: &[&str]) -> SelectionSnapshot {
        SelectionSnapshot {
            schema_version: 1,
            catalog_uri: qnc_application_catalog::DEFAULT_URI.into(),
            observed_at_unix_ms: 1,
            applications: tabs
                .iter()
                .enumerate()
                .map(|(i, tab)| qnc_application_catalog::SelectedApplication {
                    application_id: format!("qnc.{tab}"),
                    tab_id: (*tab).into(),
                    label: (*tab).into(),
                    priority_group: char::from(b'a' + i as u8).to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn navigation_reads_only_new_public_sequence_without_modifying_database() {
        let root = temp_root("navigation_readonly");
        let store = ProjectStore::open(&root).unwrap();
        let project = store
            .create_project(
                "Navigation",
                "tpl_breaking_news",
                None,
                &test_selection(&["alternative", "next"]),
            )
            .unwrap();
        let path = root
            .join("projects")
            .join(&project.project_id)
            .join("qnc_project.db");
        let before = fs::read(&path).unwrap();
        let steps = store.navigation_sequence(&project.project_id).unwrap();
        assert_eq!(steps[0].application_id, "qnc.alternative");
        assert_eq!(steps[1].priority_group, "b");
        assert_eq!(before, fs::read(&path).unwrap());
        unlock_project_dir(path.parent().unwrap()).unwrap();
        let db = Connection::open(&path).unwrap();
        let entry: String = db
            .query_row(
                "SELECT entry_step_id FROM project_workflow_state",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(entry, "step_next");
        db.execute("UPDATE project_workflow_steps SET settings_json = '{}'", [])
            .unwrap();
        drop(db);
        let before_invalid_read = fs::read(&path).unwrap();
        assert!(store.navigation_sequence(&project.project_id).is_err());
        assert!(store.open_project(&project.project_id).is_err());
        assert_eq!(before_invalid_read, fs::read(&path).unwrap());
        cleanup_temp_root(&root);
    }

    #[test]
    fn workflow_writer_rejects_legacy_tabs_without_selection() {
        let db = Connection::open_in_memory().unwrap();
        init_workspace_schema(&db).unwrap();
        assert!(write_project_workflow(
            &db,
            "legacy",
            &json!({"workspace":{"tabs":["project","ingest"]}})
        )
        .is_err());
        let count: i64 = db
            .query_row("SELECT count(*) FROM project_workflow_steps", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn application_sequence_round_trips_through_template_and_public_project_db() {
        let root = temp_root("application_sequence");
        let store = ProjectStore::open(&root).unwrap();
        let mut selection = test_selection(&["first", "variant", "last"]);
        selection.applications[1].priority_group = "c".into();
        selection.applications[2].priority_group = "d".into();
        let template = store
            .create_user_template("Grouped", "", "tpl_breaking_news", None, &selection)
            .unwrap();
        let saved = store.selected_template_settings().unwrap();
        assert_eq!(
            saved.settings["workspace"]["application_selection"],
            serde_json::to_value(&selection).unwrap()
        );
        let project = store
            .create_project("Grouped", &template.template_id, None, &selection)
            .unwrap();
        let dir = root
            .join("projects")
            .join(safe_dir_name(&project.project_id));
        unlock_project_dir(&dir).unwrap();
        let db = Connection::open(dir.join("qnc_project.db")).unwrap();
        let mut statement = db.prepare("SELECT application_id, priority_group FROM public_project_application_sequence ORDER BY position").unwrap();
        let rows: Vec<(String, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            vec![
                ("qnc.first".into(), "a".into()),
                ("qnc.variant".into(), "c".into()),
                ("qnc.last".into(), "d".into())
            ]
        );
        let settings: String = db
            .query_row("SELECT settings_json FROM project_settings", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&settings).unwrap()["workspace"]["application_selection"],
            serde_json::to_value(&selection).unwrap()
        );
        drop(statement);
        drop(db);
        cleanup_temp_root(&root);
    }

    #[test]
    fn owner_rejects_duplicate_groups_before_creating_project_or_template() {
        let root = temp_root("reject_group");
        let store = ProjectStore::open(&root).unwrap();
        let before = store.list_project_templates().unwrap().len();
        let mut selection = test_selection(&["first", "variant"]);
        selection.applications[1].priority_group = "a".into();
        assert!(store
            .create_project("Invalid", "tpl_breaking_news", None, &selection)
            .is_err());
        assert!(store
            .create_user_template("Invalid", "", "tpl_breaking_news", None, &selection)
            .is_err());
        assert!(store.list_projects().unwrap().is_empty());
        assert_eq!(store.list_project_templates().unwrap().len(), before);
        selection.applications.remove(1);
        selection.applications[0].priority_group = "b".into();
        assert!(store
            .create_project("Without A", "tpl_breaking_news", None, &selection)
            .is_err());
        cleanup_temp_root(&root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "qnc_project_store_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn cleanup_temp_root(root: &Path) {
        let projects_root = root.join("projects");
        let _ = unlock_projects_root_dir(&projects_root);
        if let Ok(entries) = fs::read_dir(&projects_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let _ = unlock_project_dir(&path);
                }
            }
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn registry_db_path_is_resolved_from_qnc_uri() {
        let root = temp_root("registry_resolver");
        let store = ProjectStore::open(&root).expect("store");

        assert_eq!(
            store.project_registry_db_path().expect("registry path"),
            root.join("data").join("project_store.db")
        );
        assert!(root.join("data").join("project_store.db").is_file());
        cleanup_temp_root(&root);
    }

    #[test]
    fn registry_connections_use_busy_timeout_and_wal() {
        let root = temp_root("sqlite_multi_process_mode");
        let store = ProjectStore::open(&root).expect("store");
        let conn = store.open_registry().expect("registry");

        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("busy timeout");
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");

        assert!(busy_timeout >= 5_000);
        assert_eq!(journal_mode.to_lowercase(), "wal");
        cleanup_temp_root(&root);
    }

    #[test]
    fn creates_registry_and_workspace_db_with_qnc_uri() {
        let root = temp_root("create");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        assert!(templates.len() >= 4);
        let row = store
            .create_project(
                "Test Projekt",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        assert!(row.project_id.starts_with("test_projekt_"));
        assert!(row.project_uri.starts_with("qnc://local/project/"));
        assert!(parse_qnc_uri(&row.project_uri).is_ok());
        assert!(root.join("data").join("project_store.db").is_file());
        assert!(root
            .join("projects")
            .join(safe_dir_name(&row.project_id))
            .join("qnc_project.db")
            .is_file());
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));
        for subdir in PROJECT_DIRECTORIES {
            assert!(
                project_dir.join(subdir).is_dir(),
                "missing project directory {subdir}"
            );
        }
        let rows = store.list_projects().expect("rows");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].active);
        cleanup_temp_root(&root);
    }

    #[test]
    fn project_origin_is_identical_in_both_databases_and_immutable_on_open() {
        let root = temp_root("origin");
        let store = ProjectStore::open(&root).unwrap();
        let row = store
            .create_project(
                "Origin Test",
                "tpl_breaking_news",
                None,
                &test_selection(&["project", "ingest"]),
            )
            .unwrap();
        let registry = store.open_registry().unwrap();
        let workspace_path = root
            .join("projects")
            .join(safe_dir_name(&row.project_id))
            .join("qnc_project.db");
        let workspace = Connection::open_with_flags(
            &workspace_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let read = |db: &Connection| -> (String, String, String, String) {
            db.query_row("SELECT project_id, project_name, created_at, identity_json FROM public_project_origin", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap()
        };
        let before = read(&registry);
        assert_eq!(before, read(&workspace));
        assert_eq!(before.0, row.project_id);
        assert_eq!(before.1, "Origin Test");
        let identity: qnc_workstation_identity::IdentitySnapshot =
            serde_json::from_str(&before.3).unwrap();
        assert_eq!(
            identity.contract_version,
            qnc_workstation_identity::CONTRACT_VERSION
        );
        let creation: String = registry
            .query_row(
                "SELECT created_at FROM projects WHERE project_id = ?1",
                params![row.project_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(creation, before.2);
        assert_eq!(row.created_date.len(), 11);
        assert_eq!(
            row.created_date,
            store.list_projects().unwrap()[0].created_date
        );
        store.open_project(&row.project_id).unwrap();
        assert_eq!(before, read(&registry));
        assert_eq!(before, read(&workspace));
        drop(workspace);
        drop(registry);
        // The public snapshot remains readable without registry/store files beside it.
        let portable = root.join("portable.db");
        fs::copy(&workspace_path, &portable).unwrap();
        let copied =
            Connection::open_with_flags(portable, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        assert_eq!(before, read(&copied));
        drop(copied);
        cleanup_temp_root(&root);
    }

    #[test]
    fn same_name_projects_have_distinct_random_ids_and_separate_origins() {
        let root = temp_root("unique_origins");
        let store = ProjectStore::open(&root).unwrap();
        let selection = test_selection(&["project", "ingest"]);
        let first = store
            .create_project("Same name", "tpl_breaking_news", None, &selection)
            .unwrap();
        let second = store
            .create_project("Same name", "tpl_breaking_news", None, &selection)
            .unwrap();
        assert_ne!(first.project_id, second.project_id);
        for row in [&first, &second] {
            let suffix = row.project_id.rsplit('_').next().unwrap();
            assert_eq!(uuid::Uuid::parse_str(suffix).unwrap().get_version_num(), 4);
        }
        let registry = store.open_registry().unwrap();
        let count: i64 = registry
            .query_row("SELECT count(*) FROM project_origin", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        store.delete_project(&first.project_id).unwrap();
        let remaining: String = registry
            .query_row("SELECT project_id FROM public_project_origin", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(remaining, second.project_id);
        drop(registry);
        cleanup_temp_root(&root);
    }

    #[test]
    fn missing_origin_is_not_backfilled_when_project_is_opened() {
        let root = temp_root("no_origin_backfill");
        let store = ProjectStore::open(&root).unwrap();
        let row = store
            .create_project(
                "No Backfill",
                "tpl_breaking_news",
                None,
                &test_selection(&["project"]),
            )
            .unwrap();
        let registry = store.open_registry().unwrap();
        registry
            .execute(
                "DELETE FROM project_origin WHERE project_id=?1",
                params![row.project_id],
            )
            .unwrap();
        store.open_project(&row.project_id).unwrap();
        let count: i64 = registry
            .query_row("SELECT count(*) FROM project_origin", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        drop(registry);
        cleanup_temp_root(&root);
    }

    #[test]
    fn create_project_creates_default_export_directory_when_unset() {
        let root = temp_root("default_export_dir");
        let store = ProjectStore::open(&root).expect("store");
        let row = store
            .create_project(
                "Default Export",
                "tpl_breaking_news",
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));
        assert!(
            project_dir.join(DEFAULT_EXPORT_DIRECTORY).is_dir(),
            "missing default export directory"
        );

        unlock_project_dir(&project_dir).expect("unlock for DB inspection");
        let conn = Connection::open(project_dir.join("qnc_project.db")).expect("workspace db");
        let raw: String = conn
            .query_row(
                "SELECT settings_json FROM project_settings WHERE project_id = ?1",
                params![row.project_id],
                |row| row.get(0),
            )
            .expect("settings json");
        let stored = parse_json(&raw, json!({}));
        assert_eq!(stored["export"]["directory"], DEFAULT_EXPORT_DIRECTORY);
        cleanup_temp_root(&root);
    }

    #[test]
    fn failed_create_keeps_registry_and_active_project_unchanged() {
        let root = temp_root("failed_create");
        let store = ProjectStore::open(&root).unwrap();
        let selection = test_selection(&["project", "ingest"]);
        let first = store
            .create_project("Existing", "tpl_breaking_news", None, &selection)
            .unwrap();
        let error = store
            .create_project(
                "Invalid",
                "tpl_breaking_news",
                Some(&json!({"export": {"directory": "../invalid"}})),
                &selection,
            )
            .unwrap_err();
        assert!(error.contains("Export direktorij"));
        assert_eq!(store.list_projects().unwrap(), vec![first]);
        let conn = store.open_registry().unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM project_storage_locations", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        assert_eq!(fs::read_dir(root.join("projects")).unwrap().count(), 1);
        drop(conn);
        cleanup_temp_root(&root);
    }

    #[test]
    fn workspace_failure_removes_only_the_unpublished_directory() {
        let root = temp_root("workspace_failure");
        let store = ProjectStore::open(&root).unwrap();
        let error = store
            .create_project(
                "Failed workspace",
                "tpl_breaking_news",
                Some(&json!({"export": {"directory": "qnc_project.db"}})),
                &test_selection(&["project", "ingest"]),
            )
            .unwrap_err();
        assert!(!error.is_empty());
        assert!(store.list_projects().unwrap().is_empty());
        assert_eq!(fs::read_dir(root.join("projects")).unwrap().count(), 0);
        cleanup_temp_root(&root);
    }

    #[test]
    fn publication_failure_rolls_back_registry_and_preserves_external_export() {
        let root = temp_root("publication_failure");
        let store = ProjectStore::open(&root).unwrap();
        let selection = test_selection(&["project", "ingest"]);
        let first = store
            .create_project("Existing", "tpl_breaking_news", None, &selection)
            .unwrap();
        let export = root.join("shared-export");
        fs::create_dir(&export).unwrap();
        fs::write(export.join("keep.txt"), "existing export").unwrap();
        let conn = store.open_registry().unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_new_location BEFORE INSERT ON project_storage_locations
            BEGIN SELECT RAISE(ABORT, 'test publication failure'); END;",
        )
        .unwrap();
        let settings = json!({"export": {"directory": export}});
        let error = store
            .create_project(
                "Unpublished",
                "tpl_breaking_news",
                Some(&settings),
                &selection,
            )
            .unwrap_err();
        assert!(error.contains("test publication failure"));
        assert!(!error.contains("uklanjanje nedovrsenog"), "{error}");
        assert_eq!(store.list_projects().unwrap(), vec![first]);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM project_storage_locations", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        assert_eq!(fs::read_dir(root.join("projects")).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(export.join("keep.txt")).unwrap(),
            "existing export"
        );
        drop(conn);
        cleanup_temp_root(&root);
    }

    #[test]
    fn typed_projects_root_controls_workspace_and_persisted_location() {
        let root = temp_root("typed_projects_root");
        let store = ProjectStore::open(&root).unwrap();
        let requested = root.join("typed location");
        let settings = json!({"storage": {"projects_root": requested}});
        let project = store
            .create_project(
                "Typed location",
                "tpl_breaking_news",
                Some(&settings),
                &test_selection(&["project", "ingest"]),
            )
            .unwrap();
        let dir = requested.join(&project.project_id);
        assert!(dir.join("qnc_project.db").is_file());
        assert!(dir.join(DEFAULT_EXPORT_DIRECTORY).is_dir());
        assert!(!root.join("projects").join(&project.project_id).exists());
        let conn = store.open_registry().unwrap();
        assert_eq!(
            project_dir_from_conn(&conn, &store.projects_root, &project.project_id)
                .unwrap()
                .canonicalize()
                .unwrap(),
            dir.canonicalize().unwrap()
        );
        assert_eq!(
            PathBuf::from(
                get_local_setting(&conn, PROJECTS_ROOT_KEY)
                    .unwrap()
                    .unwrap()
            )
            .canonicalize()
            .unwrap(),
            requested.canonicalize().unwrap()
        );
        let db = Connection::open_with_flags(
            dir.join("qnc_project.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let saved: String = db
            .query_row("SELECT settings_json FROM project_settings", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&saved).unwrap()["storage"]["projects_root"],
            settings["storage"]["projects_root"]
        );
        drop(db);
        drop(conn);
        store.delete_project(&project.project_id).unwrap();
        unlock_projects_root_dir(&requested).unwrap();
        cleanup_temp_root(&root);
    }

    #[test]
    fn create_without_override_reads_the_latest_root_from_registry() {
        let root = temp_root("latest_root");
        let first = ProjectStore::open(&root).unwrap();
        let mut second = ProjectStore::open(&root).unwrap();
        let requested = root.join("selected_by_other_instance");
        second.set_projects_root(&requested).unwrap();
        let project = first
            .create_project(
                "Current root",
                "tpl_breaking_news",
                None,
                &test_selection(&["project", "ingest"]),
            )
            .unwrap();
        assert!(requested
            .join(&project.project_id)
            .join("qnc_project.db")
            .is_file());
        assert!(!root.join("projects").join(&project.project_id).exists());
        first.delete_project(&project.project_id).unwrap();
        unlock_projects_root_dir(&requested).unwrap();
        cleanup_temp_root(&root);
    }

    #[test]
    fn unconfigured_transport_root_is_rejected_without_local_fallback() {
        let root = temp_root("unconfigured_transport_root");
        let store = ProjectStore::open(&root).unwrap();
        for requested in [
            "qnc://local/source/unknown",
            "qnc://lan/server/source/projects",
            "qnc://intranet/server/source/projects",
        ] {
            let error = store
                .create_project(
                    "Remote",
                    "tpl_breaking_news",
                    Some(&json!({"storage": {"projects_root": requested}})),
                    &test_selection(&["project", "ingest"]),
                )
                .unwrap_err();
            assert!(error.contains("transport binding"));
        }
        assert!(store.list_projects().unwrap().is_empty());
        assert!(!root.join("projects").exists());
        cleanup_temp_root(&root);
    }

    #[test]
    fn unpublished_cleanup_refuses_a_directory_outside_the_confirmed_root() {
        let root = temp_root("cleanup_boundary");
        let projects = root.join("projects");
        let unrelated = root.join("unrelated");
        fs::create_dir_all(&projects).unwrap();
        fs::create_dir(&unrelated).unwrap();
        fs::write(unrelated.join("keep.txt"), "keep").unwrap();
        assert!(remove_unpublished_project(&projects.canonicalize().unwrap(), &unrelated).is_err());
        assert_eq!(
            fs::read_to_string(unrelated.join("keep.txt")).unwrap(),
            "keep"
        );
        cleanup_temp_root(&root);
    }

    #[test]
    fn create_project_uses_custom_export_directory_when_set() {
        let root = temp_root("custom_export_dir");
        let store = ProjectStore::open(&root).expect("store");
        let custom_export_dir = root.join("custom_export");
        let settings = json!({
            "export": {
                "directory": custom_export_dir.to_string_lossy().to_string()
            }
        });
        let row = store
            .create_project(
                "Custom Export",
                "tpl_breaking_news",
                Some(&settings),
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));
        assert!(
            custom_export_dir.is_dir(),
            "missing custom export directory"
        );
        assert!(
            !project_dir.join(DEFAULT_EXPORT_DIRECTORY).exists(),
            "default export directory must not be created when a custom location is set"
        );

        unlock_project_dir(&project_dir).expect("unlock for DB inspection");
        let conn = Connection::open(project_dir.join("qnc_project.db")).expect("workspace db");
        let raw: String = conn
            .query_row(
                "SELECT settings_json FROM project_settings WHERE project_id = ?1",
                params![row.project_id],
                |row| row.get(0),
            )
            .expect("settings json");
        let stored = parse_json(&raw, json!({}));
        assert_eq!(
            stored["export"]["directory"],
            custom_export_dir.to_string_lossy().to_string()
        );
        cleanup_temp_root(&root);
    }

    #[test]
    fn open_does_not_repair_registered_project_directories() {
        let root = temp_root("no_repair_dirs");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        let row = store
            .create_project(
                "No Repair Dirs",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));
        unlock_project_dir(&project_dir).expect("unlock for external test delete");
        fs::remove_dir_all(project_dir.join("proxy")).expect("remove proxy");
        fs::remove_dir_all(project_dir.join("incoming")).expect("remove incoming");

        let _reopened = ProjectStore::open(&root).expect("reopen");

        assert!(!project_dir.join("proxy").exists());
        assert!(!project_dir.join("incoming").exists());
        unlock_project_dir(&project_dir).expect("unlock cleanup");
        cleanup_temp_root(&root);
    }

    #[test]
    fn created_project_directory_is_locked_after_create() {
        let root = temp_root("locked_project");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        let row = store
            .create_project(
                "Locked Project",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));
        assert!(project_dir
            .metadata()
            .expect("project dir metadata")
            .permissions()
            .readonly());
        #[cfg(windows)]
        assert!(project_dir_is_hidden(&project_dir));
        unlock_project_dir(&project_dir).expect("unlock cleanup");
        cleanup_temp_root(&root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_locked_project_directory_rejects_external_delete_until_unlocked() {
        let root = temp_root("windows_delete_lock");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        let row = store
            .create_project(
                "Windows Delete Lock",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));

        fs::remove_dir_all(&project_dir).expect_err("locked project dir must reject OS delete");
        assert!(project_dir.is_dir());
        assert!(project_dir.join("qnc_project.db").is_file());

        unlock_project_dir(&project_dir).expect("unlock for delete");
        fs::remove_dir_all(&project_dir).expect("delete after unlock");
        assert!(!project_dir.exists());
        cleanup_temp_root(&root);
    }

    #[test]
    fn open_project_marks_active_project() {
        let root = temp_root("active");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        let first = store
            .create_project(
                "Prvi",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("first");
        let second = store
            .create_project(
                "Drugi",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("second");
        store.open_project(&first.project_id).expect("open first");
        let rows = store.list_projects().expect("rows");
        assert!(rows
            .iter()
            .any(|row| row.project_id == first.project_id && row.active));
        assert!(rows
            .iter()
            .any(|row| row.project_id == second.project_id && !row.active));
        cleanup_temp_root(&root);
    }

    #[test]
    fn seeds_project_templates_from_qnc_v4_seed_copy() {
        let root = temp_root("templates");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        assert!(templates
            .iter()
            .any(|template| template.template_id == "tpl_breaking_news"));
        assert!(templates.iter().any(|template| template.selected));
        cleanup_temp_root(&root);
    }

    #[test]
    fn selected_projects_root_controls_new_project_private_location() {
        let root = temp_root("root");
        let selected_root = root.join("custom_projects");
        let mut store = ProjectStore::open(&root).expect("store");
        store
            .set_projects_root(&selected_root)
            .expect("set projects root");
        let templates = store.list_project_templates().expect("templates");
        let row = store
            .create_project(
                "Root Test",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        assert!(selected_root
            .join(safe_dir_name(&row.project_id))
            .join("qnc_project.db")
            .is_file());
        assert!(!root
            .join("projects")
            .join(safe_dir_name(&row.project_id))
            .join("qnc_project.db")
            .exists());
        cleanup_temp_root(&root);
    }

    #[test]
    fn creates_user_template_from_selected_base() {
        let root = temp_root("user_template");
        let store = ProjectStore::open(&root).expect("store");
        let created = store
            .create_user_template(
                "Moj template",
                "Opis",
                "tpl_breaking_news",
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("custom template");
        assert!(!created.system);
        assert!(created.template_id.starts_with("tpl_user_moj_template_"));
        let templates = store.list_project_templates().expect("templates");
        assert!(templates
            .iter()
            .any(|template| template.template_id == created.template_id && template.selected));
        cleanup_temp_root(&root);
    }

    #[test]
    fn delete_user_template_removes_custom_template_only() {
        let root = temp_root("delete_user_template");
        let store = ProjectStore::open(&root).expect("store");
        let created = store
            .create_user_template(
                "Moj template",
                "Opis",
                "tpl_breaking_news",
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("custom template");
        store
            .delete_user_template(&created.template_id)
            .expect("delete custom template");
        let templates = store.list_project_templates().expect("templates");
        assert!(!templates
            .iter()
            .any(|template| template.template_id == created.template_id));
        assert!(templates.iter().any(|template| template.selected));
        cleanup_temp_root(&root);
    }

    #[test]
    fn delete_user_template_refuses_system_template() {
        let root = temp_root("delete_system_template");
        let store = ProjectStore::open(&root).expect("store");
        let error = store
            .delete_user_template("tpl_breaking_news")
            .expect_err("system template must be guarded");
        assert!(error.contains("System template se ne može obrisati"));
        let templates = store.list_project_templates().expect("templates");
        assert!(templates
            .iter()
            .any(|template| template.template_id == "tpl_breaking_news" && template.system));
        cleanup_temp_root(&root);
    }

    #[test]
    fn project_workspace_contains_template_workflow() {
        let root = temp_root("workflow");
        let store = ProjectStore::open(&root).expect("store");
        let row = store
            .create_project(
                "Workflow Test",
                "tpl_breaking_news",
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));
        unlock_project_dir(&project_dir).expect("unlock for DB inspection");
        let conn = Connection::open(project_dir.join("qnc_project.db")).expect("workspace db");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_workflow_steps WHERE project_id = ?1",
                params![row.project_id],
                |row| row.get(0),
            )
            .expect("workflow count");
        assert!(count >= 2);
        let entry: String = conn
            .query_row(
                "SELECT entry_step_id FROM project_workflow_state WHERE project_id = ?1",
                params![row.project_id],
                |row| row.get(0),
            )
            .expect("entry step");
        assert!(!entry.trim().is_empty());
        cleanup_temp_root(&root);
    }

    #[test]
    fn draft_settings_are_saved_into_project_workspace() {
        let root = temp_root("draft_settings");
        let store = ProjectStore::open(&root).expect("store");
        let settings = json!({
            "workflow": "manual_test",
            "storage": { "ingest_profile": "house" },
            "workspace": {
                "tabs": ["project", "media_assist"],
                "tab_labels": { "media_assist": "Media Assist" }
            }
        });
        let row = store
            .create_project(
                "Draft Test",
                "tpl_breaking_news",
                Some(&settings),
                &test_selection(&["project", "media_assist"]),
            )
            .expect("project");
        let project_dir = root.join("projects").join(safe_dir_name(&row.project_id));
        unlock_project_dir(&project_dir).expect("unlock for DB inspection");
        let conn = Connection::open(project_dir.join("qnc_project.db")).expect("workspace db");
        let raw: String = conn
            .query_row(
                "SELECT settings_json FROM project_settings WHERE project_id = ?1",
                params![row.project_id],
                |row| row.get(0),
            )
            .expect("settings json");
        let stored = parse_json(&raw, json!({}));
        assert_eq!(stored["storage"]["ingest_profile"], "house");
        let entry: String = conn
            .query_row(
                "SELECT entry_step_id FROM project_workflow_state WHERE project_id = ?1",
                params![row.project_id],
                |row| row.get(0),
            )
            .expect("entry");
        assert_eq!(entry, "step_media_assist");
        cleanup_temp_root(&root);
    }

    #[test]
    fn delete_project_removes_registry_row_and_workspace_dir() {
        let root = temp_root("delete_project");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        let first = store
            .create_project(
                "Prvi",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("first");
        let second = store
            .create_project(
                "Drugi",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("second");
        let second_dir = root
            .join("projects")
            .join(safe_dir_name(&second.project_id));
        assert!(second_dir.join("qnc_project.db").is_file());

        let deleted = store.delete_project(&second.project_id).expect("delete");
        assert_eq!(deleted.name, second.name);
        assert!(!second_dir.exists());

        let rows = store.list_projects().expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].project_id, first.project_id);
        assert!(rows[0].active);
        cleanup_temp_root(&root);
    }

    #[test]
    fn delete_project_refuses_unexpected_storage_directory() {
        let root = temp_root("delete_guard");
        let store = ProjectStore::open(&root).expect("store");
        let templates = store.list_project_templates().expect("templates");
        let project = store
            .create_project(
                "Guard",
                &templates[0].template_id,
                None,
                &test_selection(&["project", "ingest"]),
            )
            .expect("project");
        let bad_dir = root.join("unexpected_project_path");
        fs::create_dir_all(&bad_dir).expect("bad dir");
        fs::write(bad_dir.join("keep.txt"), "keep").expect("marker");
        let conn = store.open_registry().expect("registry");
        conn.execute(
            "UPDATE project_storage_locations
             SET local_path = ?2
             WHERE project_id = ?1",
            params![project.project_id, bad_dir.to_string_lossy().to_string()],
        )
        .expect("bad path update");

        let error = store
            .delete_project(&project.project_id)
            .expect_err("delete must be guarded");
        assert!(error.contains("Brisanje zaustavljeno"));
        assert!(bad_dir.join("keep.txt").is_file());
        assert!(store
            .list_projects()
            .expect("rows")
            .iter()
            .any(|row| row.project_id == project.project_id));
        cleanup_temp_root(&root);
    }

    #[cfg(windows)]
    fn project_dir_is_hidden(path: &Path) -> bool {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        path.metadata()
            .map(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
            .unwrap_or(false)
    }
}
