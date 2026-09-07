use crate::{contract::*, Access, Credentials, Store};
use qnc_json_transport::JsonClient;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
pub const ENDPOINT: &str = "/v1/media-records";
enum Endpoint {
    Local(Store),
    Remote(JsonClient),
}
pub struct Client {
    uri: String,
    access: Access,
    endpoint: Endpoint,
}
impl Client {
    pub fn open(
        resolver: &ResolverConfig,
        uri: &str,
        access: Access,
        token: Option<&str>,
    ) -> Result<Self> {
        Self::connect(resolver, uri, access, token, false)
    }
    pub fn create_local(resolver: &ResolverConfig, uri: &str) -> Result<Self> {
        Self::connect(resolver, uri, Access::ReadWrite, None, true)
    }
    fn connect(
        resolver: &ResolverConfig,
        uri: &str,
        access: Access,
        token: Option<&str>,
        initialize: bool,
    ) -> Result<Self> {
        validate_db_uri(uri)?;
        let endpoint = match resolver
            .resolve(uri)
            .map_err(|_| Error::InvalidRequest)?
            .endpoint
        {
            ResolvedEndpoint::LocalPath(path) => {
                Endpoint::Local(Store::open_owner_binding(&path, access, initialize)?)
            }
            ResolvedEndpoint::NetworkEndpoint { .. } => {
                if initialize {
                    return Err(Error::AccessDenied);
                }
                Endpoint::Remote(
                    JsonClient::connect(
                        resolver,
                        uri,
                        ENDPOINT,
                        token.ok_or(Error::AccessDenied)?,
                        MAX_BYTES,
                    )
                    .map_err(transport_error)?,
                )
            }
        };
        Ok(Self {
            uri: uri.into(),
            access,
            endpoint,
        })
    }
    pub fn write(&mut self, write: Write) -> Result<Receipt> {
        match self.execute(Operation::Write(Box::new(write)))? {
            Data::Written(r) => Ok(r),
            _ => Err(Error::Protocol),
        }
    }
    pub fn begin_acquisition(&mut self, begin: BeginAcquisition) -> Result<AcquisitionClaim> {
        match self.execute(Operation::BeginAcquisition(begin))? {
            Data::AcquisitionClaim(claim) => Ok(*claim),
            _ => Err(Error::Protocol),
        }
    }
    pub fn finish_acquisition(&mut self, finish: FinishAcquisition) -> Result<Acquisition> {
        match self.execute(Operation::FinishAcquisition(finish))? {
            Data::Acquisition(Some(attempt)) => Ok(*attempt),
            _ => Err(Error::Protocol),
        }
    }
    pub fn acquisition(&mut self, media_uri: &str) -> Result<Option<Acquisition>> {
        match self.execute(Operation::Acquisition {
            media_uri: media_uri.into(),
        })? {
            Data::Acquisition(attempt) => Ok(attempt.map(|a| *a)),
            _ => Err(Error::Protocol),
        }
    }
    pub fn read(&mut self, clip_id: &str, revision: Option<u32>) -> Result<Option<Snapshot>> {
        match self.execute(Operation::Read {
            clip_id: clip_id.into(),
            revision,
        })? {
            Data::Snapshot(s) => Ok(s.map(|s| *s)),
            _ => Err(Error::Protocol),
        }
    }
    pub fn document(&mut self, uri: &str) -> Result<Option<Document>> {
        match self.execute(Operation::Document {
            document_uri: uri.into(),
        })? {
            Data::Document(d) => Ok(d),
            _ => Err(Error::Protocol),
        }
    }
    fn execute(&mut self, operation: Operation) -> Result<Data> {
        if operation.is_write() && self.access == Access::ReadOnly {
            return Err(Error::AccessDenied);
        }
        let request = Request {
            version: VERSION.into(),
            db_uri: self.uri.clone(),
            operation,
        };
        request.validate()?;
        let reply = match &mut self.endpoint {
            Endpoint::Local(store) => Reply {
                version: VERSION.into(),
                db_uri: self.uri.clone(),
                result: store.execute(&request),
            },
            Endpoint::Remote(client) => {
                client.post::<_, Reply>(&request).map_err(transport_error)?
            }
        };
        reply.validate(&request)
    }
}
pub fn respond(
    request: tiny_http::Request,
    store: &mut Store,
    published_uri: &str,
    credentials: &Credentials,
) {
    qnc_json_transport::respond_json(
        request,
        ENDPOINT,
        credentials,
        MAX_BYTES,
        |body: Request, access| {
            let result = if validate_db_uri(published_uri).is_err() || body.db_uri != published_uri
            {
                Err(Error::WrongDatabase)
            } else if body.operation.is_write() && access != Access::ReadWrite {
                Err(Error::AccessDenied)
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
fn transport_error(error: qnc_json_transport::Error) -> Error {
    match error {
        qnc_json_transport::Error::Configuration => Error::InvalidRequest,
        qnc_json_transport::Error::AccessDenied => Error::AccessDenied,
        qnc_json_transport::Error::TooLarge => Error::TooLarge,
        qnc_json_transport::Error::Unavailable => Error::Unavailable,
        qnc_json_transport::Error::Protocol => Error::Protocol,
    }
}
