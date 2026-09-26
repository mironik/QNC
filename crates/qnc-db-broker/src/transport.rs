//! Where the project database is (local file or LAN/intranet endpoint) and the one
//! serial writer every process uses for it.

use std::{
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
};

use qnc_json_transport::JsonClient;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::{
    project_db_uri, project_id, Access, Credentials, ProjectDb, Reply, Request, Result,
    TableModuleFactory, MAX_BYTES, VERSION,
};

pub const ENDPOINT: &str = "/v1/project-db";

/// The project database of the active project: its public URI and the private
/// binding the project gives (a local file or a LAN/intranet endpoint).
#[derive(Clone)]
pub struct ProjectDbTarget {
    uri: String,
    binding: Binding,
}

#[derive(Clone)]
enum Binding {
    Local(PathBuf),
    Remote { resolver: ResolverConfig, token: String },
}

impl std::fmt::Debug for ProjectDbTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectDbTarget").field("uri", &self.uri).finish_non_exhaustive()
    }
}

impl ProjectDbTarget {
    /// From the active project settings: the same binding as its workspace database.
    pub fn for_project(
        reader: &qnc_work_settings::SettingsReader,
        settings: &qnc_work_settings::WorkSettings,
    ) -> Result<Self> {
        let owner = reader.workspace_binding(settings).map_err(|e| e.to_string())?;
        let uri = project_db_uri(&settings.workspace_db_uri)?;
        let binding = match owner
            .resolver
            .resolve(&settings.workspace_db_uri)
            .map_err(|e| e.to_string())?
            .endpoint
        {
            ResolvedEndpoint::LocalPath(file) => Binding::Local(file),
            ResolvedEndpoint::NetworkEndpoint { .. } => Binding::Remote {
                resolver: owner.resolver,
                token: owner.token.ok_or("Nedostaje DB credential.")?,
            },
        };
        Ok(Self { uri, binding })
    }

    /// The owner supplies the private file binding; the public identity stays `uri`.
    pub fn from_owner_binding(file: &Path, uri: &str) -> Result<Self> {
        project_id(uri)?;
        Ok(Self {
            uri: uri.into(),
            binding: Binding::Local(file.to_path_buf()),
        })
    }

    pub fn from_remote_binding(resolver: ResolverConfig, uri: &str, token: String) -> Result<Self> {
        project_id(uri)?;
        if token.is_empty() {
            return Err("Nedostaje DB credential.".into());
        }
        Ok(Self {
            uri: uri.into(),
            binding: Binding::Remote { resolver, token },
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn open(
        &self,
        access: Access,
        factories: Vec<Arc<dyn TableModuleFactory>>,
    ) -> Result<ProjectDbClient> {
        let endpoint = match &self.binding {
            Binding::Local(file) => {
                Endpoint::Local(ProjectDb::open(file, &self.uri, access, factories.clone())?)
            }
            Binding::Remote { resolver, token } => Endpoint::Remote(
                JsonClient::connect(resolver, &self.uri, ENDPOINT, token, MAX_BYTES)
                    .map_err(|e| e.to_string())?,
            ),
        };
        Ok(ProjectDbClient {
            uri: self.uri.clone(),
            access,
            factories,
            endpoint,
        })
    }
}

enum Endpoint {
    Local(ProjectDb),
    Remote(JsonClient),
}

pub struct ProjectDbClient {
    uri: String,
    access: Access,
    factories: Vec<Arc<dyn TableModuleFactory>>,
    endpoint: Endpoint,
}

impl ProjectDbClient {
    pub fn execute(&mut self, module: &str, payload: Value) -> Result<Value> {
        let factory = self
            .factories
            .iter()
            .find(|factory| factory.id() == module)
            .ok_or_else(|| format!("Nepoznat modul baze projekta: {module}"))?;
        if factory.is_write(&payload) && self.access == Access::ReadOnly {
            return Err("Pristup je read-only.".into());
        }
        let request = Request {
            version: VERSION.into(),
            db_uri: self.uri.clone(),
            module: module.into(),
            payload,
        };
        match &mut self.endpoint {
            Endpoint::Local(db) => db.execute(&request),
            Endpoint::Remote(client) => {
                let reply: Reply = client.post(&request).map_err(|e| e.to_string())?;
                if reply.version != VERSION || reply.db_uri != request.db_uri {
                    return Err("Pogresan odgovor baze projekta.".into());
                }
                reply.result
            }
        }
    }
}

/// The one serial writer of a project database in this process (v5
/// `serialize_project_write`): any number of workers send requests; they run in
/// order and each waits for its own reply.
#[derive(Debug, Clone)]
pub struct ProjectDbWriter {
    requests: Sender<(String, Value, Sender<Result<Value>>)>,
}

impl ProjectDbWriter {
    pub fn start(
        target: ProjectDbTarget,
        factories: Vec<Arc<dyn TableModuleFactory>>,
    ) -> Result<Self> {
        let mut client = target.open(Access::ReadWrite, factories)?;
        let (requests, receive) = mpsc::channel::<(String, Value, Sender<Result<Value>>)>();
        std::thread::Builder::new()
            .name("qnc-db-broker-writer".into())
            .spawn(move || {
                for (module, payload, reply) in receive {
                    let _ = reply.send(client.execute(&module, payload));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { requests })
    }

    pub fn call(&self, module: &str, payload: Value) -> Result<Value> {
        let (reply, answer) = mpsc::channel();
        self.requests
            .send((module.into(), payload, reply))
            .map_err(|_| "Posrednik baze projekta nije dostupan.".to_string())?;
        answer
            .recv()
            .map_err(|_| "Posrednik baze projekta nije odgovorio.".to_string())?
    }

    /// Sends a request without waiting: it runs in order with every other request of
    /// this writer, and its reply is taken later (a form never waits for a write).
    pub fn submit(&self, module: &str, payload: Value) -> Result<Pending> {
        let (reply, answer) = mpsc::channel();
        self.requests
            .send((module.into(), payload, reply))
            .map_err(|_| "Posrednik baze projekta nije dostupan.".to_string())?;
        Ok(Pending(answer))
    }

    /// A typed request of a table module and its typed reply.
    pub fn request<Q: Serialize, R: DeserializeOwned>(&self, module: &str, request: &Q) -> Result<R> {
        let payload = serde_json::to_value(request).map_err(|e| e.to_string())?;
        serde_json::from_value(self.call(module, payload)?).map_err(|e| e.to_string())
    }
}

/// The reply of a submitted request, once it is there.
#[derive(Debug)]
pub struct Pending(mpsc::Receiver<Result<Value>>);

impl Pending {
    /// The reply when the request has run; `None` while it waits in line.
    pub fn try_take(&self) -> Option<Result<Value>> {
        match self.0.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Posrednik baze projekta nije odgovorio.".into()))
            }
        }
    }
}

/// Serves the project database on LAN/intranet with the same requests.
pub fn respond(
    request: tiny_http::Request,
    db: &mut ProjectDb,
    published_uri: &str,
    credentials: &Credentials,
    factories: &[Arc<dyn TableModuleFactory>],
) {
    qnc_json_transport::respond_json(
        request,
        ENDPOINT,
        credentials,
        MAX_BYTES,
        |body: Request, access| {
            let writes = factories
                .iter()
                .find(|factory| factory.id() == body.module)
                .is_some_and(|factory| factory.is_write(&body.payload));
            let result = if body.db_uri != published_uri {
                Err("Pogresna projektna baza.".into())
            } else if writes && access == Access::ReadOnly {
                Err("Pristup je read-only.".into())
            } else {
                db.execute(&body)
            };
            Reply {
                version: VERSION.into(),
                db_uri: body.db_uri,
                result,
            }
        },
    );
}
