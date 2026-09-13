use qnc_json_transport::JsonClient;
pub use qnc_json_transport::{Access, Credentials};
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub const MODULE_ID: &str = "qnc.module.project-close";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const LOCAL_REGISTRY_URI: &str = "qnc://local/db/project_registry";
pub const ENDPOINT: &str = "/v1/project-close";
pub const MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseProjectConfig {
    pub registry_uri: String,
    pub registry_file: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseProjectOutcome {
    pub closed: bool,
    pub previous_project_id: Option<String>,
}

pub struct CloseProjectComponent {
    registry_uri: String,
    endpoint: CloseEndpoint,
}

enum CloseEndpoint {
    Local(PathBuf),
    Remote(JsonClient),
}

impl std::fmt::Debug for CloseProjectComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloseProjectComponent")
            .field("registry_uri", &self.registry_uri)
            .finish_non_exhaustive()
    }
}

impl CloseProjectComponent {
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        Self::local(root.as_ref().join("data").join("project_store.db"))
    }

    pub fn local(registry_file: impl Into<PathBuf>) -> Self {
        Self {
            registry_uri: LOCAL_REGISTRY_URI.into(),
            endpoint: CloseEndpoint::Local(registry_file.into()),
        }
    }

    pub fn from_config(config: CloseProjectConfig) -> Result<Self, String> {
        let uri = validate_registry_uri(&config.registry_uri)?;
        if uri.environment == "local" {
            if config.endpoint.is_some() || config.token_env.is_some() {
                return Err("Lokalni Project close ne smije imati mrezni endpoint.".into());
            }
            return Ok(Self::local(config.registry_file.ok_or_else(|| {
                "Lokalni Project close zahtijeva registry file binding.".to_string()
            })?));
        }
        if config.registry_file.is_some() {
            return Err("Mrezni Project close ne smije imati lokalni file binding.".into());
        }
        let endpoint = config
            .endpoint
            .as_deref()
            .ok_or_else(|| "Mrezni Project close zahtijeva transport endpoint.".to_string())?;
        let authority = uri
            .authority
            .clone()
            .ok_or_else(|| "Mrezni Project close zahtijeva authority.".to_string())?;
        let resolver = if uri.environment == "lan" {
            ResolverConfig::new(PathBuf::new()).with_lan_authority(authority, endpoint)
        } else {
            ResolverConfig::new(PathBuf::new()).with_intranet_authority(authority, endpoint)
        };
        let token_env = config
            .token_env
            .as_deref()
            .ok_or_else(|| "Mrezni Project close zahtijeva write token env.".to_string())?;
        let token = std::env::var(token_env)
            .ok()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| "Project close write credential nije dostupan.".to_string())?;
        Self::open(&resolver, &config.registry_uri, &token)
    }

    pub fn open(
        resolver: &ResolverConfig,
        registry_uri: &str,
        write_token: &str,
    ) -> Result<Self, String> {
        validate_registry_uri(registry_uri)?;
        let endpoint = match resolver
            .resolve(registry_uri)
            .map_err(|_| "Project close registry transport nije dostupan.".to_string())?
            .endpoint
        {
            ResolvedEndpoint::LocalPath(path) => CloseEndpoint::Local(path),
            ResolvedEndpoint::NetworkEndpoint { .. } => CloseEndpoint::Remote(
                JsonClient::connect(resolver, registry_uri, ENDPOINT, write_token, MAX_BYTES)
                    .map_err(transport_error)?,
            ),
        };
        Ok(Self {
            registry_uri: registry_uri.into(),
            endpoint,
        })
    }

    pub fn close_active_project(&self) -> Result<CloseProjectOutcome, String> {
        let request = CloseProjectRequest {
            version: VERSION.into(),
            registry_uri: self.registry_uri.clone(),
            operation: CloseProjectOperation::CloseActive,
        };
        match &self.endpoint {
            CloseEndpoint::Local(path) => close_local_registry(path),
            CloseEndpoint::Remote(client) => {
                validate_reply(client.post(&request).map_err(transport_error)?, &request)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseProjectRequest {
    pub version: String,
    pub registry_uri: String,
    pub operation: CloseProjectOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseProjectOperation {
    CloseActive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseProjectReply {
    pub version: String,
    pub registry_uri: String,
    pub result: Result<CloseProjectOutcome, String>,
}

pub fn respond(
    request: tiny_http::Request,
    registry_file: &Path,
    published_uri: &str,
    credentials: &Credentials,
) {
    qnc_json_transport::respond_json(
        request,
        ENDPOINT,
        credentials,
        MAX_BYTES,
        |body: CloseProjectRequest, access| {
            let result = if body.version != VERSION {
                Err("Pogresna Project close verzija.".into())
            } else if validate_registry_uri(published_uri).is_err()
                || body.registry_uri != published_uri
            {
                Err("Pogresan Project registry.".into())
            } else if access != Access::ReadWrite {
                Err("Project close zahtijeva write ovlast.".into())
            } else {
                match body.operation {
                    CloseProjectOperation::CloseActive => close_local_registry(registry_file),
                }
            };
            CloseProjectReply {
                version: VERSION.into(),
                registry_uri: body.registry_uri,
                result,
            }
        },
    );
}

fn close_local_registry(path: &Path) -> Result<CloseProjectOutcome, String> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "Project registry baza nije dostupna.".to_string())?;
    conn.busy_timeout(Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    let previous_project_id: Option<String> = conn
        .query_row(
            "SELECT value FROM public_app_settings WHERE key = 'active_project_id'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| "Project registry nema javni active_project_id zapis.".to_string())?
        .filter(|id: &String| !id.trim().is_empty());
    let Some(previous_project_id) = previous_project_id else {
        return Ok(CloseProjectOutcome {
            closed: false,
            previous_project_id: None,
        });
    };
    conn.execute(
        "UPDATE app_settings SET value = '' WHERE key = ?1",
        params!["active_project_id"],
    )
    .map_err(|_| "Project registry ne dopusta zatvaranje aktivnog projekta.".to_string())?;
    Ok(CloseProjectOutcome {
        closed: true,
        previous_project_id: Some(previous_project_id),
    })
}

fn validate_registry_uri(uri: &str) -> Result<qnc_contracts::QncUri, String> {
    let parsed = qnc_contracts::parse_qnc_uri(uri)
        .map_err(|_| "Neispravan registry QNC URI.".to_string())?;
    if parsed.resource_kind != "db" || parsed.resource_id != "project_registry" {
        return Err("Project close zahtijeva project_registry DB URI.".into());
    }
    if !matches!(parsed.environment.as_str(), "local" | "lan" | "intranet") {
        return Err("Project close zahtijeva local, LAN ili Intranet registry URI.".into());
    }
    Ok(parsed)
}

fn validate_reply(
    reply: CloseProjectReply,
    request: &CloseProjectRequest,
) -> Result<CloseProjectOutcome, String> {
    if reply.version != VERSION || reply.registry_uri != request.registry_uri {
        return Err("Pogresan Project close odgovor.".into());
    }
    reply.result
}

fn transport_error(error: qnc_json_transport::Error) -> String {
    match error {
        qnc_json_transport::Error::Configuration => "Project close transport nije konfiguriran.",
        qnc_json_transport::Error::AccessDenied => "Project close transport nije odobren.",
        qnc_json_transport::Error::TooLarge => "Project close transport odgovor je prevelik.",
        qnc_json_transport::Error::Unavailable => "Project close transport nije dostupan.",
        qnc_json_transport::Error::Protocol => "Project close transport odgovor nije valjan.",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, thread, time::Duration};

    const LAN_URI: &str = "qnc://lan/studio/db/project_registry";
    const READ_TOKEN: &str = "project-close-read";
    const WRITE_TOKEN: &str = "project-close-write";

    #[test]
    fn close_active_project_clears_only_active_flag() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let workspace = root.path().join("projects").join("p1");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("qnc_project.db"), []).unwrap();
        let db = data.join("project_store.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE VIEW public_app_settings AS SELECT key, value FROM app_settings;
            INSERT INTO app_settings VALUES('active_project_id', 'p1');
            ",
        )
        .unwrap();

        let result = CloseProjectComponent::from_root(root.path())
            .close_active_project()
            .unwrap();

        assert!(result.closed);
        assert_eq!(result.previous_project_id.as_deref(), Some("p1"));
        let active: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='active_project_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(active.is_empty());
        assert!(workspace.join("qnc_project.db").is_file());
    }

    #[test]
    fn close_active_project_without_active_project_does_not_insert_or_delete() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        fs::create_dir_all(&data).unwrap();
        let db = data.join("project_store.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE VIEW public_app_settings AS SELECT key, value FROM app_settings;
            ",
        )
        .unwrap();

        let result = CloseProjectComponent::from_root(root.path())
            .close_active_project()
            .unwrap();

        assert!(!result.closed);
        assert!(result.previous_project_id.is_none());
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_settings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn missing_registry_is_an_error_not_a_new_database() {
        let root = tempfile::tempdir().unwrap();
        let db = root.path().join("data").join("project_store.db");

        let error = CloseProjectComponent::from_root(root.path())
            .close_active_project()
            .expect_err("missing registry must fail");

        assert!(error.contains("Project registry baza nije dostupna"));
        assert!(!db.exists());
    }

    #[test]
    fn lan_close_active_project_uses_public_write_adapter() {
        let root = tempfile::tempdir().unwrap();
        let db = project_registry(root.path(), "p1");
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let credentials = Credentials::new(READ_TOKEN, WRITE_TOKEN).unwrap();
        let handle = thread::spawn({
            let db = db.clone();
            move || {
                let request = server
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap()
                    .unwrap();
                respond(request, &db, LAN_URI, &credentials);
            }
        });
        let resolver = ResolverConfig::new(PathBuf::new()).with_lan_authority("studio", &url);
        let component = CloseProjectComponent::open(&resolver, LAN_URI, WRITE_TOKEN).unwrap();

        let result = component.close_active_project().unwrap();

        assert!(result.closed);
        assert_eq!(result.previous_project_id.as_deref(), Some("p1"));
        assert!(active_project_id(&db).unwrap().is_empty());
        handle.join().unwrap();
    }

    #[test]
    fn lan_close_active_project_rejects_read_token_without_changing_state() {
        let root = tempfile::tempdir().unwrap();
        let db = project_registry(root.path(), "p1");
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let credentials = Credentials::new(READ_TOKEN, WRITE_TOKEN).unwrap();
        let handle = thread::spawn({
            let db = db.clone();
            move || {
                let request = server
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap()
                    .unwrap();
                respond(request, &db, LAN_URI, &credentials);
            }
        });
        let resolver = ResolverConfig::new(PathBuf::new()).with_lan_authority("studio", &url);
        let component = CloseProjectComponent::open(&resolver, LAN_URI, READ_TOKEN).unwrap();

        let error = component.close_active_project().unwrap_err();

        assert!(error.contains("write ovlast"));
        assert_eq!(active_project_id(&db).as_deref(), Some("p1"));
        handle.join().unwrap();
    }

    fn project_registry(root: &Path, active_id: &str) -> PathBuf {
        let data = root.join("data");
        fs::create_dir_all(&data).unwrap();
        let db = data.join("project_store.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE VIEW public_app_settings AS SELECT key, value FROM app_settings;
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings VALUES('active_project_id', ?1)",
            [active_id],
        )
        .unwrap();
        db
    }

    fn active_project_id(db: &Path) -> Option<String> {
        let conn = Connection::open(db).unwrap();
        conn.query_row(
            "SELECT value FROM app_settings WHERE key='active_project_id'",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
    }
}
