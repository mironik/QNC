use super::*;
use qnc_json_transport::JsonClient;
use qnc_transport_resolver::ResolverConfig;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread::JoinHandle,
    time::Instant,
};

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
    /// The DB owner supplies the private file binding. Public identity remains `uri`.
    pub fn from_owner_binding(file: &Path, uri: &str) -> Result<Self> {
        project_id(uri)?;
        Ok(Self {
            uri: uri.into(),
            binding: TargetBinding::Local(file.to_path_buf()),
        })
    }

    pub fn from_remote_binding(resolver: ResolverConfig, uri: &str, token: String) -> Result<Self> {
        project_id(uri)?;
        if token.is_empty() {
            return Err("Nedostaje DB credential.".into());
        }
        Ok(Self {
            uri: uri.into(),
            binding: TargetBinding::Remote { resolver, token },
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test_owner_binding(file: &Path, uri: &str) -> Self {
        Self {
            uri: uri.into(),
            binding: TargetBinding::Local(file.to_path_buf()),
        }
    }

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

    pub fn uri(&self) -> &str {
        &self.uri
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
    pub fn publish_filmstrip(&mut self, artifact: FilmstripArtifactRecord) -> Result<()> {
        match self.execute(Operation::PublishFilmstrip(Box::new(artifact)))? {
            Data::Changed => Ok(()),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn read_filmstrip(&mut self, clip_id: &str) -> Result<Option<FilmstripArtifactRecord>> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        match self.execute(Operation::ReadFilmstrip {
            clip_id: clip_id.into(),
        })? {
            Data::Filmstrip(artifact) => Ok(artifact.map(|record| *record)),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn publish_wave(&mut self, artifact: WaveArtifactRecord) -> Result<()> {
        match self.execute(Operation::PublishWave(Box::new(artifact)))? {
            Data::Changed => Ok(()),
            _ => Err("Neispravan odgovor baze.".into()),
        }
    }
    pub fn read_wave(&mut self, clip_id: &str) -> Result<Option<WaveArtifactRecord>> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        match self.execute(Operation::ReadWave {
            clip_id: clip_id.into(),
        })? {
            Data::Wave(artifact) => Ok(artifact.map(|record| *record)),
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
    /// Renews the lease of the clip this importer is working on.
    pub fn heartbeat(&mut self, clip_id: String) -> Result<()> {
        self.execute(Operation::Heartbeat { clip_id }).map(|_| ())
    }
    pub fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        self.execute(Operation::FinishImport {
            clip_id,
            media_uri,
            thumbnail_uri,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWriteResult {
    pub elapsed_ms: u128,
    pub data: ContentWriteData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentWriteData {
    Changed,
    Removed(Vec<String>),
    /// The clip taken from the import queue, if any.
    Claimed(Option<Box<StoredClip>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWriteCompletion {
    pub key: String,
    pub result: Result<ContentWriteResult>,
}

#[derive(Debug)]
struct ContentWriteCommand {
    key: String,
    operation: Operation,
}

#[derive(Debug)]
pub struct ContentWriteTransport {
    pending: usize,
    send: Option<Sender<ContentWriteCommand>>,
    receive: Receiver<ContentWriteCompletion>,
    thread: Option<JoinHandle<()>>,
}

impl ContentWriteTransport {
    pub fn start(target: ContentTarget) -> Result<Self> {
        let (send, receive_commands) = mpsc::channel();
        let (send_results, receive) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("qnc-content-write-transport".into())
            .spawn(move || run_content_write_transport(target, receive_commands, send_results))
            .map_err(err)?;
        Ok(Self {
            pending: 0,
            send: Some(send),
            receive,
            thread: Some(thread),
        })
    }

    pub fn publish_filmstrip(
        &mut self,
        key: String,
        artifact: FilmstripArtifactRecord,
    ) -> Result<()> {
        let command = ContentWriteCommand {
            key,
            operation: Operation::PublishFilmstrip(Box::new(artifact)),
        };
        self.send
            .as_ref()
            .ok_or_else(|| "Content write transport nije aktivan.".to_string())?
            .send(command)
            .map_err(err)?;
        self.pending += 1;
        Ok(())
    }

    pub fn publish_batch(&mut self, key: String, clips: Vec<CatalogClip>) -> Result<()> {
        let command = ContentWriteCommand {
            key,
            operation: Operation::PublishBatch(clips),
        };
        self.send
            .as_ref()
            .ok_or_else(|| "Content write transport nije aktivan.".to_string())?
            .send(command)
            .map_err(err)?;
        self.pending += 1;
        Ok(())
    }

    pub fn remove_missing(&mut self, key: String, clips: Vec<InventoryClip>) -> Result<()> {
        let command = ContentWriteCommand {
            key,
            operation: Operation::RemoveMissing { clips },
        };
        self.send
            .as_ref()
            .ok_or_else(|| "Content write transport nije aktivan.".to_string())?
            .send(command)
            .map_err(err)?;
        self.pending += 1;
        Ok(())
    }

    pub fn select(&mut self, key: String, clip_ids: Vec<String>, selected: bool) -> Result<()> {
        let command = ContentWriteCommand {
            key,
            operation: Operation::Select { clip_ids, selected },
        };
        self.send
            .as_ref()
            .ok_or_else(|| "Content write transport nije aktivan.".to_string())?
            .send(command)
            .map_err(err)?;
        self.pending += 1;
        Ok(())
    }

    /// Puts the selected, ready clips into the import queue.
    pub fn queue_selected(&mut self, key: String) -> Result<()> {
        self.send_operation(key, Operation::QueueSelected)
    }

    /// Takes the next queued clip for import (`Claimed`).
    pub fn claim_next(&mut self, key: String) -> Result<()> {
        self.send_operation(key, Operation::ClaimNext)
    }

    /// Renews the lease of the clip an importer is working on.
    pub fn heartbeat(&mut self, key: String, clip_id: String) -> Result<()> {
        self.send_operation(key, Operation::Heartbeat { clip_id })
    }

    /// Records the outcome of one import: the imported media URI or an error.
    pub fn finish_import(
        &mut self,
        key: String,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        self.send_operation(
            key,
            Operation::FinishImport {
                clip_id,
                media_uri,
                thumbnail_uri,
                error,
            },
        )
    }

    fn send_operation(&mut self, key: String, operation: Operation) -> Result<()> {
        let command = ContentWriteCommand { key, operation };
        self.send
            .as_ref()
            .ok_or_else(|| "Content write transport nije aktivan.".to_string())?
            .send(command)
            .map_err(err)?;
        self.pending += 1;
        Ok(())
    }

    pub fn publish_wave(&mut self, key: String, artifact: WaveArtifactRecord) -> Result<()> {
        let command = ContentWriteCommand {
            key,
            operation: Operation::PublishWave(Box::new(artifact)),
        };
        self.send
            .as_ref()
            .ok_or_else(|| "Content write transport nije aktivan.".to_string())?
            .send(command)
            .map_err(err)?;
        self.pending += 1;
        Ok(())
    }

    pub fn poll(&mut self) -> Vec<ContentWriteCompletion> {
        let mut completions = Vec::new();
        loop {
            match self.receive.try_recv() {
                Ok(completion) => {
                    self.pending = self.pending.saturating_sub(1);
                    completions.push(completion);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.pending = 0;
                    break;
                }
            }
        }
        completions
    }

    pub fn has_pending(&self) -> bool {
        self.pending > 0
    }

    pub fn close(&mut self) {
        self.send.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ContentWriteTransport {
    fn drop(&mut self) {
        self.close();
    }
}

fn run_content_write_transport(
    target: ContentTarget,
    receive: Receiver<ContentWriteCommand>,
    send: Sender<ContentWriteCompletion>,
) {
    let mut client = None;
    while let Ok(command) = receive.recv() {
        let started = Instant::now();
        let result = execute_write_command(&target, &mut client, command.operation).map(|data| {
            ContentWriteResult {
                elapsed_ms: started.elapsed().as_millis(),
                data,
            }
        });
        if result.is_err() {
            client = None;
        }
        let _ = send.send(ContentWriteCompletion {
            key: command.key,
            result,
        });
    }
}

fn execute_write_command(
    target: &ContentTarget,
    client: &mut Option<ContentClient>,
    operation: Operation,
) -> Result<ContentWriteData> {
    if client.is_none() {
        *client = Some(target.open(Access::ReadWrite)?);
    }
    let Some(client) = client.as_mut() else {
        return Err("Content write transport nije otvorio bazu.".into());
    };
    match operation {
        Operation::PublishBatch(clips) => {
            client.publish_batch(clips)?;
            Ok(ContentWriteData::Changed)
        }
        Operation::RemoveMissing { clips } => {
            let removed = client.remove_missing(clips)?;
            Ok(ContentWriteData::Removed(removed))
        }
        Operation::Select { clip_ids, selected } => {
            client.select(clip_ids, selected)?;
            Ok(ContentWriteData::Changed)
        }
        Operation::PublishFilmstrip(artifact) => {
            client.publish_filmstrip(*artifact)?;
            Ok(ContentWriteData::Changed)
        }
        Operation::PublishWave(artifact) => {
            client.publish_wave(*artifact)?;
            Ok(ContentWriteData::Changed)
        }
        Operation::QueueSelected => {
            client.queue_selected()?;
            Ok(ContentWriteData::Changed)
        }
        Operation::ClaimNext => Ok(ContentWriteData::Claimed(client.claim_next()?.map(Box::new))),
        Operation::Heartbeat { clip_id } => {
            client.heartbeat(clip_id)?;
            Ok(ContentWriteData::Changed)
        }
        Operation::FinishImport {
            clip_id,
            media_uri,
            thumbnail_uri,
            error,
        } => {
            client.finish_import(clip_id, media_uri, thumbnail_uri, error)?;
            Ok(ContentWriteData::Changed)
        }
        _ => Err("Nepodrzana content write transport operacija.".into()),
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
