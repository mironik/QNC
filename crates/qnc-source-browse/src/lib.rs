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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowseLocationEntry {
    pub name: String,
    pub qnc_uri: String,
    pub serial_number: String,
    pub volume_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowseState {
    pub roots: bool,
    pub path_label: String,
    pub current_uri: Option<String>,
    pub parent_available: bool,
    pub entries: Vec<BrowseLocationEntry>,
}

impl From<BrowserState> for BrowseState {
    fn from(state: BrowserState) -> Self {
        Self {
            roots: state.roots,
            path_label: state.path_label,
            current_uri: state.current_uri,
            parent_available: state.parent_available,
            entries: state
                .entries
                .into_iter()
                .map(|entry| BrowseLocationEntry {
                    name: entry.name,
                    qnc_uri: entry.qnc_uri,
                    serial_number: entry.serial_number,
                    volume_name: entry.volume_name,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    pub name: String,
    pub serial_number: String,
    pub volume_name: String,
}

pub trait BrowseEntry {
    fn browse_uri(&self) -> &str;
    fn browse_name(&self) -> &str;
    fn browse_serial_number(&self) -> &str;
    fn browse_volume_name(&self) -> &str;
}

pub fn selected_metadata<T: BrowseEntry>(
    entries: &[T],
    uri: &str,
    current_name_is_empty: bool,
) -> Option<SourceMetadata> {
    let entry = entries.iter().find(|entry| entry.browse_uri() == uri)?;
    let has_identity = !entry.browse_serial_number().trim().is_empty()
        || !entry.browse_volume_name().trim().is_empty();
    if !has_identity && !current_name_is_empty {
        return None;
    }
    Some(SourceMetadata {
        name: entry.browse_name().to_string(),
        serial_number: if has_identity {
            entry.browse_serial_number().to_string()
        } else {
            String::new()
        },
        volume_name: if has_identity {
            entry.browse_volume_name().to_string()
        } else {
            String::new()
        },
    })
}

type Outcome = (TransportBrowserSession, Result<BrowseState, String>);

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

    /// Owner-side diagnostic/local binding for a confirmed location. It is not part
    /// of the public browser state and is available only for registered local roots.
    pub fn selected_private_local_path(&self, uri: &str) -> Option<std::path::PathBuf> {
        self.session.as_ref()?.selected_private_local_path(uri)
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
                }
                .map(Into::into);
                let _ = send.send((session, state));
            })
            .map_err(|error| error.to_string())?;
        self.result = Some(receive);
        Ok(())
    }

    /// The finished step, `None` while it still runs or when none was started.
    pub fn poll(&mut self) -> Option<Result<BrowseState, String>> {
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

    #[derive(Debug)]
    struct TestEntry {
        uri: &'static str,
        name: &'static str,
        serial_number: &'static str,
        volume_name: &'static str,
    }

    impl BrowseEntry for TestEntry {
        fn browse_uri(&self) -> &str {
            self.uri
        }

        fn browse_name(&self) -> &str {
            self.name
        }

        fn browse_serial_number(&self) -> &str {
            self.serial_number
        }

        fn browse_volume_name(&self) -> &str {
            self.volume_name
        }
    }

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

    #[test]
    fn selected_metadata_prefers_card_identity_and_preserves_existing_name_without_it() {
        let entries = vec![
            TestEntry {
                uri: "qnc://local/source/card",
                name: "Card",
                serial_number: "S1",
                volume_name: "VOL",
            },
            TestEntry {
                uri: "qnc://local/source/folder",
                name: "Folder",
                serial_number: "",
                volume_name: "",
            },
        ];

        assert_eq!(
            selected_metadata(&entries, "qnc://local/source/card", false),
            Some(SourceMetadata {
                name: "Card".into(),
                serial_number: "S1".into(),
                volume_name: "VOL".into()
            })
        );
        assert_eq!(
            selected_metadata(&entries, "qnc://local/source/folder", false),
            None
        );
        assert_eq!(
            selected_metadata(&entries, "qnc://local/source/folder", true),
            Some(SourceMetadata {
                name: "Folder".into(),
                serial_number: String::new(),
                volume_name: String::new()
            })
        );
    }
}
