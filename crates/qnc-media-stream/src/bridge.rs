use crate::{HttpEndpoint, MediaStream, server};
use qnc_json_transport::Credentials;
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

/// Session-private codec bridge. Stop the codec/close its sockets before dropping this owner.
/// Remote bytes still pass through MediaStream validation; no remote credential reaches the codec.
pub struct LoopbackBridge {
    endpoint: HttpEndpoint,
    stop: Arc<AtomicBool>,
    workers: Vec<thread::JoinHandle<()>>,
}
impl LoopbackBridge {
    pub fn new(media: MediaStream) -> io::Result<Self> {
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(io::Error::other)?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let credentials = Credentials::new(&token, &uuid::Uuid::new_v4().simple().to_string())
            .map_err(io::Error::other)?;
        let endpoint = HttpEndpoint::for_owner_endpoint(
            &format!("http://{}", server.server_addr()),
            &media.info().media_uri,
            &token,
        )?;
        let stop = Arc::new(AtomicBool::new(false));
        let media = Arc::new(Mutex::new(media));
        let server = Arc::new(server);
        let credentials = Arc::new(credentials);
        let mut bridge = Self {
            endpoint,
            stop,
            workers: vec![],
        };
        for index in 0..2 {
            let cancelled = bridge.stop.clone();
            let media = media.clone();
            let server = server.clone();
            let credentials = credentials.clone();
            bridge.workers.push(
                thread::Builder::new()
                    .name(format!("qnc-media-bridge-{index}"))
                    .spawn(move || {
                        while !cancelled.load(Ordering::Acquire) {
                            match server.recv_timeout(Duration::from_millis(20)) {
                                Ok(Some(request)) => {
                                    let _ = server::respond_shared(
                                        request,
                                        media.clone(),
                                        &credentials,
                                    );
                                }
                                Ok(None) => (),
                                Err(_) => break,
                            }
                        }
                    })?,
            );
        }
        Ok(bridge)
    }
    pub fn endpoint(&self) -> &HttpEndpoint {
        &self.endpoint
    }
}
impl Drop for LoopbackBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
