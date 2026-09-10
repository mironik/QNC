use super::*;
use qnc_json_transport::JsonClient;
use qnc_transport_resolver::ResolverConfig;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct ContentTarget {
    uri: String,
    binding: TargetBinding,
}
#[derive(Clone)]
enum TargetBinding {
    Local(PathBuf),
    Remote {
        resolver: ResolverConfig,
        token: String,
    },
}
impl std::fmt::Debug for ContentTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentTarget")
            .field("uri", &self.uri)
            .finish_non_exhaustive()
    }
}
impl ContentTarget {
    pub fn for_project(
        reader: &qnc_work_settings::SettingsReader,
        settings: &qnc_work_settings::WorkSettings,
    ) -> Result<Self> {
        let owner = reader.workspace_binding(settings).map_err(err)?;
        let uri = content_uri(&settings.workspace_db_uri)?;
        let binding = match owner
            .resolver
            .resolve(&settings.workspace_db_uri)
            .map_err(err)?
            .endpoint
        {
            qnc_transport_resolver::ResolvedEndpoint::LocalPath(file) => TargetBinding::Local(file),
            qnc_transport_resolver::ResolvedEndpoint::NetworkEndpoint { .. } => {
                TargetBinding::Remote {
                    resolver: owner.resolver,
                    token: owner.token.ok_or("Nedostaje DB credential.")?,
                }
            }
        };
        Ok(Self { uri, binding })
    }
    pub fn open(&self, access: Access) -> Result<ContentClient> {
        match &self.binding {
            TargetBinding::Local(file) => {
                if !file.is_file() {
                    return Err("Projektna baza vise ne postoji.".into());
                }
                ContentClient::from_owner_binding(file, &self.uri, access)
            }
            TargetBinding::Remote { resolver, token } => {
                ContentClient::from_remote(resolver, &self.uri, access, token)
            }
        }
    }
}

pub const ENDPOINT: &str = "/v1/ingest-content";
enum Endpoint {
    Local(ContentStore),
    Remote(JsonClient),
}
pub struct ContentClient {
    uri: String,
    access: Access,
    endpoint: Endpoint,
}

impl ContentClient {
    /// The storage owner supplies the exact file binding; this module chooses no directory.
    pub fn from_owner_binding(file: &Path, uri: &str, access: Access) -> Result<Self> {
        Ok(Self {
            uri: uri.into(),
            access,
            endpoint: Endpoint::Local(ContentStore::open_owner_binding(file, uri, access)?),
        })
    }

    pub fn from_remote(
        resolver: &ResolverConfig,
        uri: &str,
        access: Access,
        token: &str,
    ) -> Result<Self> {
        project_id(uri)?;
        Ok(Self {
            uri: uri.into(),
            access,
            endpoint: Endpoint::Remote(
                JsonClient::connect(resolver, uri, ENDPOINT, token, MAX_BYTES).map_err(err)?,
            ),
        })
    }
    pub fn publish(&mut self, clip: CatalogClip) -> Result<StoredClip> {
        match self.execute(Operation::Publish(Box::new(clip)))? {
            Data::Saved(clip) => Ok(*clip),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn list(&mut self, after: Option<String>) -> Result<Vec<StoredClip>> {
        match self.execute(Operation::List { after })? {
            Data::Clips(clips) => {
                for clip in &clips {
                    clip.clip.validate()?;
                }
                Ok(clips)
            }
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn list_summary(&mut self, after: Option<String>) -> Result<Vec<StoredClipSummary>> {
        match self.execute(Operation::ListSummary { after })? {
            Data::ClipSummaries(clips) => {
                for clip in &clips {
                    clip.validate()?;
                }
                Ok(clips)
            }
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn stats(&mut self) -> Result<CatalogStats> {
        match self.execute(Operation::Stats)? {
            Data::CatalogStats(stats) => Ok(stats),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn read(&mut self, clip_id: &str) -> Result<Option<StoredClip>> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        match self.execute(Operation::Read {
            clip_id: clip_id.into(),
        })? {
            Data::Clip(clip) => {
                if let Some(stored) = &clip {
                    stored.clip.validate()?;
                    if stored.clip.id() != clip_id {
                        return Err("Odgovor ne pripada trazenom klipu.".into());
                    }
                }
                Ok(clip.map(|c| *c))
            }
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn publish_batch(&mut self, clips: Vec<CatalogClip>) -> Result<()> {
        match self.execute(Operation::PublishBatch(clips))? {
            Data::Changed => Ok(()),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn inventory(
        &mut self,
        source_uri: &str,
        after: Option<String>,
    ) -> Result<Vec<InventoryClip>> {
        match self.execute(Operation::Inventory {
            source_uri: source_uri.into(),
            after,
        })? {
            Data::Inventory(clips) => Ok(clips),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    /// Caller supplies explicit source absence observations, never a failed listing.
    pub fn remove_missing(&mut self, clips: Vec<InventoryClip>) -> Result<Vec<String>> {
        match self.execute(Operation::RemoveMissing { clips })? {
            Data::Removed(ids) => Ok(ids),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn select(&mut self, clip_ids: Vec<String>, selected: bool) -> Result<()> {
        self.execute(Operation::Select { clip_ids, selected })
            .map(|_| ())
    }
    pub fn queue_selected(&mut self) -> Result<()> {
        self.execute(Operation::QueueSelected).map(|_| ())
    }
    pub fn claim_next(&mut self) -> Result<Option<StoredClip>> {
        match self.execute(Operation::ClaimNext)? {
            Data::Claimed(clip) => Ok(clip.map(|c| *c)),
            _ => Err("Neispravan claim odgovor.".into()),
        }
    }
    pub fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        self.execute(Operation::FinishImport {
            clip_id,
            media_uri,
            error,
        })
        .map(|_| ())
    }
    fn execute(&mut self, operation: Operation) -> Result<Data> {
        if operation.is_write() && self.access == Access::ReadOnly {
            return Err("Pristup je read-only.".into());
        }
        let request = Request {
            version: VERSION.into(),
            db_uri: self.uri.clone(),
            operation,
        };
        match &mut self.endpoint {
            Endpoint::Local(store) => store.execute(&request),
            Endpoint::Remote(client) => {
                let reply: Reply = client.post(&request).map_err(err)?;
                if reply.version != VERSION || reply.db_uri != request.db_uri {
                    return Err("Pogresan DB odgovor.".into());
                }
                reply.result
            }
        }
    }
}

pub fn respond(
    request: tiny_http::Request,
    store: &mut ContentStore,
    published_uri: &str,
    credentials: &Credentials,
) {
    qnc_json_transport::respond_json(
        request,
        ENDPOINT,
        credentials,
        MAX_BYTES,
        |body: Request, access| {
            let result = if body.db_uri != published_uri {
                Err("Pogresna projektna baza.".into())
            } else if body.operation.is_write() && access == Access::ReadOnly {
                Err("Pristup je read-only.".into())
            } else {
                store.execute(&body)
            };
            Reply {
                version: VERSION.into(),
                db_uri: body.db_uri,
                result,
            }
        },
    );
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
