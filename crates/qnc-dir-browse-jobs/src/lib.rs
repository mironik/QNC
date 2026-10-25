//! A location browser whose listings run on their own thread (audit 2026-10-25: the Project
//! form listed drives and folders on the form thread, so a sleeping disk froze it). The
//! browser session lives in one background thread (`qnc-store-jobs`); steps run in the
//! order sent and their listings are taken when they are there. It knows no form and no
//! application.

use std::path::PathBuf;

use qnc_dir_browser::{BrowserState, DirectoryBrowserSession};
use qnc_store_jobs::StoreJobs;

pub const MODULE_ID: &str = "qnc.module.dir-browse-jobs";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One browsing step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseStep {
    Roots,
    Path(PathBuf),
    Uri(String),
    Parent,
}

/// A step's listing and the private path of the folder it shows (for its confirm).
struct Browsed {
    listing: Result<BrowserState, String>,
    current: Option<(String, PathBuf)>,
}

pub struct BackgroundBrowser {
    jobs: StoreJobs<DirectoryBrowserSession, Browsed>,
    current: Option<(String, PathBuf)>,
}

impl BackgroundBrowser {
    /// Starts the browser's thread `name`.
    pub fn start(name: &str) -> Self {
        Self { jobs: StoreJobs::start(name, DirectoryBrowserSession::default), current: None }
    }

    /// Sends a step; its listing comes with `poll`.
    pub fn browse(&mut self, step: BrowseStep) {
        self.jobs.submit(move |session| {
            let listing = match step {
                BrowseStep::Roots => session.load_roots(),
                BrowseStep::Path(path) => session.open_private_path(path),
                BrowseStep::Uri(uri) => session.open_uri(&uri),
                BrowseStep::Parent => session.open_parent(),
            };
            let current = listing
                .as_ref()
                .ok()
                .and_then(|state| state.current_uri.clone())
                .and_then(|uri| session.path_for_uri(&uri).map(|path| (uri, path)));
            Browsed { listing, current }
        });
    }

    /// The listings that came, in the order of their steps.
    pub fn poll(&mut self) -> Vec<Result<BrowserState, String>> {
        let mut listings = Vec::new();
        for browsed in self.jobs.poll() {
            self.current = browsed.current;
            listings.push(browsed.listing);
        }
        listings
    }

    /// The private path of the folder the last listing shows, when it is `uri`.
    pub fn path_for_uri(&self, uri: &str) -> Option<PathBuf> {
        self.current.as_ref().filter(|(current, _)| current == uri).map(|(_, path)| path.clone())
    }

    /// Whether a step still runs.
    pub fn busy(&self) -> bool {
        self.jobs.busy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_folder_is_listed_in_the_background_and_its_private_path_kept_for_confirm() {
        let root = std::env::temp_dir().join(format!("qnc-dir-browse-jobs-{}", std::process::id()));
        std::fs::create_dir_all(root.join("inner")).unwrap();
        let mut browser = BackgroundBrowser::start("test-browser");
        browser.browse(BrowseStep::Path(root.clone()));
        let mut listings = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while browser.busy() && Instant::now() < deadline {
            listings.extend(browser.poll());
            std::thread::sleep(Duration::from_millis(2));
        }
        let state = listings.pop().expect("a listing").expect("the folder lists");
        assert!(state.entries.iter().any(|entry| entry.name == "inner"));
        let uri = state.current_uri.expect("the folder has its URI");
        assert!(browser.path_for_uri(&uri).is_some());
        assert_eq!(browser.path_for_uri("qnc://local/other"), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
