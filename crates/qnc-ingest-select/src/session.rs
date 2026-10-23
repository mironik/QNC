use crate::*;

const CANCEL_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Default)]
pub struct SelectSession {
    pub(crate) result: Option<Receiver<Event>>,
    pub(crate) cancel: Option<Arc<AtomicBool>>,
    pub(crate) thread: Option<JoinHandle<()>>,
}

impl SelectSession {
    pub fn start(
        &mut self,
        config: SelectionConfig,
        selected: SourceReference,
        target: SelectTarget,
        registry: Arc<CameraRegistry>,
    ) -> std::result::Result<(), String> {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (send, receive) = mpsc::sync_channel(32);
        let thread = std::thread::Builder::new()
            .name("qnc-ingest-select".into())
            .spawn(move || run(config, selected, target, registry, send, worker_cancel))
            .map_err(|error| error.to_string())?;
        self.cancel = Some(cancel);
        self.result = Some(receive);
        self.thread = Some(thread);
        Ok(())
    }

    /// Stops the running Select. Late events are discarded (the receiver is gone) and
    /// the worker starts no new database write once it sees the flag. The thread is
    /// waited for a short, bounded time so that a normal cancel ends deterministically
    /// without freezing the caller behind a long media read.
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.result = None;
        if let Some(thread) = self.thread.take() {
            let deadline = std::time::Instant::now() + CANCEL_WAIT;
            while !thread.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }

    pub fn poll(&mut self, limit: usize) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..limit {
            let Some(receiver) = self.result.as_ref() else {
                break;
            };
            let event = match receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    Event::Finished(Err(qnc_ui_text::text("select.select_process_interrupted").into()))
                }
            };
            let finished = matches!(event, Event::Finished(_));
            events.push(event);
            if finished {
                self.result = None;
                self.cancel = None;
                drop(self.thread.take());
                break;
            }
        }
        events
    }

    pub fn has_pending_work(&self) -> bool {
        self.result.is_some()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn from_receiver_for_test(receiver: Receiver<Event>) -> Self {
        Self {
            result: Some(receiver),
            cancel: None,
            thread: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_cancel_for_test(receiver: Receiver<Event>, cancel: Arc<AtomicBool>) -> Self {
        Self {
            result: Some(receiver),
            cancel: Some(cancel),
            thread: None,
        }
    }
}

impl fmt::Debug for SelectSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectSession")
            .field("pending", &self.has_pending_work())
            .finish()
    }
}

impl Drop for SelectSession {
    fn drop(&mut self) {
        self.cancel();
    }
}
