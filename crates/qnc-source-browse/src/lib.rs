//! Browsing of registered sources without blocking the caller.
//!
//! The caller asks for one step (roots of an environment, the parent, or a folder),
//! polls for the outcome and paints it. Every step runs on its own thread over the
//! source transport, so a slow LAN or intranet source never freezes the form. Nothing
//! here knows a form or an application.

use qnc_dir_browser::{BrowserState, TransportBrowserSession};
use qnc_source_reader::SourceReference;
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub const MODULE_ID: &str = "qnc.module.source-browse";
pub const VERSION: &str = "0.1.0";

/// One browsing step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The roots of `local`, `lan` or `intranet`.
    Roots(String),
    Parent,
    Open(String),
}

type Outcome = (TransportBrowserSession, Result<BrowserState, String>);

#[derive(Debug, Default)]
pub struct SourceBrowse {
    session: Option<TransportBrowserSession>,
    result: Option<Receiver<Outcome>>,
}

impl SourceBrowse {
    pub fn new() -> Self {
        Self::default()
    }

    /// The registered sources of this machine, set once they are known.
    pub fn connect(&mut self, session: TransportBrowserSession) {
        self.session = Some(session);
    }

    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }

    pub fn is_busy(&self) -> bool {
        self.result.is_some()
    }

    /// The reference of a location the browser has offered.
    pub fn selected(&self, uri: &str) -> Option<SourceReference> {
        self.session.as_ref()?.selected(uri)
    }

    pub fn start(&mut self, step: Step) -> Result<(), String> {
        let Some(mut session) = self.session.clone() else {
            return Err("Izvor nije povezan.".into());
        };
        let (send, receive) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("source-browse".into())
            .spawn(move || {
                let state = match step {
                    Step::Roots(environment) => session.roots(&environment),
                    Step::Parent => session.parent(),
                    Step::Open(uri) => session.open(&uri),
                };
                let _ = send.send((session, state));
            })
            .map_err(|error| error.to_string())?;
        self.result = Some(receive);
        Ok(())
    }

    /// The finished step, `None` while it still runs or when none was started.
    pub fn poll(&mut self) -> Option<Result<BrowserState, String>> {
        let receiver = self.result.as_ref()?;
        let (session, state) = match receiver.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                self.result = None;
                return Some(Err("Citanje izvora je prekinuto.".into()));
            }
        };
        self.result = None;
        self.session = Some(session);
        Some(state)
    }

    /// Drops the running step; its late outcome is never delivered. Returns whether
    /// a step was running.
    pub fn cancel(&mut self) -> bool {
        self.result.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_sources_nothing_starts() {
        let mut browse = SourceBrowse::new();
        assert!(!browse.is_connected());
        assert_eq!(
            browse.start(Step::Parent).unwrap_err(),
            "Izvor nije povezan."
        );
        assert!(browse.poll().is_none());
        assert!(!browse.cancel());
        assert!(browse.selected("qnc://local/source/x").is_none());
    }

    #[test]
    fn a_started_step_delivers_its_outcome_once() {
        let mut browse = SourceBrowse::new();
        browse.connect(TransportBrowserSession::default());
        browse.start(Step::Roots("local".into())).unwrap();
        assert!(browse.is_busy());
        let outcome = loop {
            if let Some(outcome) = browse.poll() {
                break outcome;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        let _ = outcome;
        assert!(browse.poll().is_none());
        assert!(browse.is_connected());
        assert!(!browse.is_busy());
    }
}
