//! The versions of the S-Story (user 2026-10-31: "from the same elements of MA Audio several
//! S-Stories can be put together; the media pool of S-Story needs a tab Stories with the
//! versions of the story"). It reads the versions and their elements from the project database
//! outside the UI thread, again now and then, gives them as cards of the pool and keeps which
//! one is open. It writes nothing.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use qnc_clip_row::{ClipRow, TabCard};
use qnc_skeleton_db::{Element, Version};
use qnc_source_preview::SourcePreview;

pub const MODULE_ID: &str = "qnc.module.skeleton-versions";

/// A click on the card of a version opens it (target: the version id).
pub const OPEN: &str = "skeleton_open_version";

const READ_EVERY: Duration = Duration::from_secs(2);

/// One version with its elements in order.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionRow {
    pub version: Version,
    pub elements: Vec<Element>,
}

/// What the versions show.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VersionsView {
    pub versions: Vec<VersionRow>,
    /// The version open in the right panel.
    pub open: Option<i64>,
    pub message: String,
}

impl VersionsView {
    /// The versions as cards of the pool's Stories tab: "Version n" with its number of
    /// elements, its length and the poster of its first clip.
    pub fn cards<'a>(&self, clips: &'a [ClipRow]) -> Vec<TabCard<'a>> {
        self.versions
            .iter()
            .map(|row| TabCard {
                id: row.version.version_id.to_string(),
                title: qnc_ui_text::fill("skeleton.version_card", &[("n", &row.version.number.to_string()), ("count", &row.elements.len().to_string())]).into(),
                seconds: row.elements.iter().map(|element| seconds(element, clips)).sum(),
                clip: row.elements.first().and_then(|element| clips.iter().find(|clip| clip.clip_id == element.clip_id)),
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.versions.len()
    }

    /// The open version.
    pub fn opened(&self) -> Option<&VersionRow> {
        self.versions.iter().find(|row| Some(row.version.version_id) == self.open)
    }
}

/// The length of an element in seconds, by the rate of its clip (unknown clip: nothing).
fn seconds(element: &Element, clips: &[ClipRow]) -> f64 {
    let frames = element.out_frame.saturating_sub(element.in_frame) as f64;
    clips
        .iter()
        .find(|clip| clip.clip_id == element.clip_id)
        .filter(|clip| clip.duration_frames > 0)
        .map_or(0.0, |clip| frames * clip.duration_seconds / clip.duration_frames as f64)
}

type Read = Result<Vec<VersionRow>, String>;

/// The versions of the project on the preview.
#[derive(Debug, Default)]
pub struct SkeletonVersions {
    view: VersionsView,
    project: Option<String>,
    reading: Option<mpsc::Receiver<Read>>,
    read_at: Option<Instant>,
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
                if self.view.open.is_some() && self.view.opened().is_none() {
                    self.view.open = None;
                }
                self.reading = None;
            }
        }
        if let Some(project) = project.filter(|_| self.reading.is_none() && self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY)) {
            self.read_at = Some(Instant::now());
            let (send, read) = mpsc::channel();
            if std::thread::Builder::new().name("qnc-skeleton-versions-read".into()).spawn(move || _ = send.send(read_versions(&project))).is_ok() {
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
        let id = target.and_then(|id| id.parse::<i64>().ok())?;
        let known = self.view.versions.iter().any(|row| row.version.version_id == id);
        if known {
            self.view.open = Some(id);
        }
        Some(known)
    }
}

fn read_versions(target: &qnc_db_broker::ProjectDbTarget) -> Read {
    qnc_skeleton_db::versions(target)?
        .into_iter()
        .map(|version| Ok(VersionRow { elements: qnc_skeleton_db::read(target, version.version_id)?, version }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_skeleton_db::{ElementKind, Lane};

    fn element(clip: &str, from: u64, to: u64) -> Element {
        let lane = Lane { channel: 0, level_db: 0, off: false };
        Element { element_id: 1, sort_index: 0, kind: ElementKind::Ton, clip_id: clip.into(), in_frame: from, out_frame: to, a1: lane, a2: lane, a1_lead_frames: 0, a1_tail_frames: 0, gap_before: 0, cut: false, origin: String::new() }
    }

    #[test]
    fn a_version_is_a_card_with_its_number_length_and_first_clip_and_opens_by_its_id() {
        let clips = vec![ClipRow { clip_id: "a".into(), duration_frames: 500, duration_seconds: 10.0, ..ClipRow::default() }];
        let mut versions = SkeletonVersions::new();
        versions.view.versions = vec![VersionRow { version: Version { version_id: 7, number: 2, elements: 2 }, elements: vec![element("a", 0, 100), element("a", 200, 250)] }];
        let cards = versions.view().cards(&clips);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, "7");
        assert!(cards[0].title.contains('2'));
        assert!((cards[0].seconds - 3.0).abs() < 1e-9);
        assert_eq!(cards[0].clip.map(|clip| clip.clip_id.as_str()), Some("a"));
        assert_eq!(versions.act(OPEN, Some("7")), Some(true));
        assert_eq!(versions.view().opened().map(|row| row.version.number), Some(2));
        assert_eq!(versions.act(OPEN, Some("8")), Some(false));
        assert_eq!(versions.act("play_pause", Some("7")), None);
    }
}
