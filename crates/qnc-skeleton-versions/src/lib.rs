//! The versions of the S-Story (user 2026-10-31: "from the same elements of MA Audio several
//! S-Stories can be put together; the media pool of S-Story needs a tab Stories with the
//! versions of the story"). Each version is a copy of the Story of its own; this piece only
//! reads the list of versions (outside the UI thread, again now and then), gives them as cards
//! of the pool and says which one is open. The Story machinery shows, plays and edits the open
//! version. It writes nothing.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use qnc_clip_row::TabCard;
use qnc_skeleton_db::Version;
use qnc_source_preview::SourcePreview;

pub const MODULE_ID: &str = "qnc.module.skeleton-versions";

/// A click on the card of a version opens it (target: its number).
pub const OPEN: &str = "skeleton_open_version";

const READ_EVERY: Duration = Duration::from_secs(2);

/// What the versions show.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VersionsView {
    pub versions: Vec<Version>,
    /// The number of the version open in the right panel.
    pub open: Option<u32>,
    pub message: String,
}

impl VersionsView {
    /// The versions as cards of the pool's Stories tab: "Version n".
    pub fn cards(&self) -> Vec<TabCard<'static>> {
        self.versions
            .iter()
            .map(|version| TabCard {
                id: version.number.to_string(),
                title: qnc_ui_text::fill("skeleton.version_card", &[("n", &version.number.to_string())]).into(),
                seconds: 0.0,
                clip: None,
                icon: qnc_clip_row::PoolIcon::None,
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.versions.len()
    }
}

type Read = Result<Vec<Version>, String>;

/// The versions of the project on the preview.
#[derive(Debug, Default)]
pub struct SkeletonVersions {
    view: VersionsView,
    project: Option<String>,
    reading: Option<mpsc::Receiver<Read>>,
    read_at: Option<Instant>,
    /// The open version changed and the story machinery has not been told yet.
    switched: bool,
}

impl SkeletonVersions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &VersionsView {
        &self.view
    }

    pub fn busy(&self) -> bool {
        self.reading.is_some()
    }

    /// The version the story machinery shows (version 1 until the list is read).
    pub fn open_number(&self) -> u32 {
        self.view.open.unwrap_or(1)
    }

    /// The number of a newly opened version, once.
    pub fn take_switch(&mut self) -> Option<u32> {
        std::mem::take(&mut self.switched).then(|| self.open_number())
    }

    /// Takes a finished read, reads again now and then. Returns whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview) -> bool {
        let before = self.view.clone();
        let project = preview.project_target().cloned();
        let key = project.as_ref().map(|target| target.uri().to_string());
        if key != self.project {
            *self = Self { project: key, ..Self::default() };
        }
        if let Some(reading) = &self.reading {
            if let Ok(result) = reading.try_recv() {
                match result {
                    Ok(versions) => {
                        self.view.versions = versions;
                        self.view.message.clear();
                    }
                    Err(error) => self.view.message = error,
                }
                // Without a version chosen (or once it is gone) the newest one is shown.
                if !self.view.open.is_some_and(|open| self.view.versions.iter().any(|version| version.number == open)) {
                    self.choose(self.view.versions.last().map(|version| version.number));
                }
                self.reading = None;
            }
        }
        if let Some(project) = project.filter(|_| self.reading.is_none() && self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY)) {
            self.read_at = Some(Instant::now());
            let (send, read) = mpsc::channel();
            if std::thread::Builder::new().name("qnc-skeleton-versions-read".into()).spawn(move || _ = send.send(qnc_skeleton_db::versions(&project))).is_ok() {
                self.reading = Some(read);
            }
        }
        self.view != before
    }

    /// Its own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>) -> Option<bool> {
        if action_id != OPEN {
            return None;
        }
        let number = target.and_then(|number| number.parse::<u32>().ok())?;
        let known = self.view.versions.iter().any(|version| version.number == number);
        if known {
            self.choose(Some(number));
        }
        Some(known)
    }

    fn choose(&mut self, number: Option<u32>) {
        if number.is_some() && number != self.view.open {
            self.switched = number != Some(self.open_number());
            self.view.open = number;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_a_card_and_opening_another_one_is_told_once() {
        let mut versions = SkeletonVersions::new();
        versions.view.versions = vec![Version { version_id: 7, number: 1 }, Version { version_id: 8, number: 2 }];
        versions.choose(Some(2));
        assert_eq!(versions.take_switch(), Some(2), "version 1 was shown before the list was read");
        assert_eq!(versions.take_switch(), None);
        let cards = versions.view().cards();
        assert_eq!(cards.iter().map(|card| card.id.as_str()).collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(versions.act(OPEN, Some("1")), Some(true));
        assert_eq!((versions.open_number(), versions.take_switch()), (1, Some(1)));
        assert_eq!(versions.act(OPEN, Some("9")), Some(false));
        assert_eq!(versions.act("play_pause", Some("1")), None);
    }

    #[test]
    fn the_first_version_read_needs_no_switch_when_it_is_version_one() {
        let mut versions = SkeletonVersions::new();
        versions.view.versions = vec![Version { version_id: 7, number: 1 }];
        versions.choose(Some(1));
        assert_eq!(versions.take_switch(), None, "the story machinery already shows version 1");
    }
}
