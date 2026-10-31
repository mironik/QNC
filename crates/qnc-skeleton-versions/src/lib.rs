//! The versions of the S-Story (user 2026-10-31: "from the same elements of MA Audio several
//! S-Stories can be put together; the media pool of S-Story needs a tab Stories with the
//! versions of the story"). It reads the versions and their elements from the project database
//! outside the UI thread, again now and then, gives them as cards of the pool and keeps which
//! one is open. The open version is the S-Story's right panel: its elements as Wrap rows one
//! under another, like the Story's, but SubClips: no M markers, slots or covers, order only.
//! The elements play one after another in the one player, as a SubSegment does. It writes
//! nothing.

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use qnc_action_event::ActionEvent;
use qnc_clip_row::{ClipRow, TabCard};
use qnc_program_commands::SegmentKind;
use qnc_program_input::PieceInput;
use qnc_program_view::{SegmentRow, SegmentsView};
use qnc_skeleton_db::{Element, ElementKind, Version};
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
    /// The open version as Wrap rows (empty without one).
    pub segments: SegmentsView,
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

/// Where each element plays on the version's axis: one after another, a pause before an
/// element moves it on (the first has none), as a SubSegment's parts.
fn spans(elements: &[Element]) -> Vec<(u64, u64)> {
    let mut start = 0;
    elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            start += if index == 0 { 0 } else { element.gap_before };
            let span = (start, start + element.out_frame.saturating_sub(element.in_frame));
            start = span.1;
            span
        })
        .collect()
}

/// The elements as the parts the player plays: each with its own A1.
pub fn piece_inputs(elements: &[Element]) -> Vec<PieceInput> {
    elements
        .iter()
        .enumerate()
        .map(|(index, element)| PieceInput {
            clip_id: element.clip_id.clone(),
            from_frame: element.in_frame,
            to_frame: element.out_frame,
            gap_before: if index == 0 { 0 } else { element.gap_before },
            crossfade: !element.cut,
            a1: Some((Some(element.a1.channel), element.a1.level_db, element.a1.off)),
        })
        .collect()
}

/// Whole frames a second of a clip of the pool (its stored frames over its seconds).
fn clip_rate(clips: &HashMap<String, (u64, f64)>, clip_id: &str) -> Option<u32> {
    clips.get(clip_id).filter(|(frames, seconds)| *frames > 0 && *seconds > 0.0).map(|(frames, seconds)| (*frames as f64 / seconds).round() as u32)
}

/// The rows of a version: one Wrap row per element, a SubClip (Ton or Off by what it is).
fn rows(elements: &[Element], spans: &[(u64, u64)], rate: Option<u32>) -> Vec<SegmentRow> {
    elements
        .iter()
        .zip(spans)
        .map(|(element, (start, end))| {
            let frames = end - start;
            let fps = rate.unwrap_or(0);
            SegmentRow {
                segment_id: element.element_id.to_string(),
                kind: if element.kind == ElementKind::Off { SegmentKind::Off } else { SegmentKind::Ton },
                clip_id: element.clip_id.clone(),
                source_in_frame: element.in_frame,
                source_out_frame: element.out_frame,
                a1_source_channel: element.a1.channel,
                a1_level_db: element.a1.level_db,
                a1_off: element.a1.off,
                start_frame: *start,
                end_frame: *end,
                duration_label: if fps > 0 { qnc_program_view::duration_label(frames, fps, 1) } else { String::new() },
                duration_color_key: qnc_program_view::duration_color_key(frames, fps, 1),
                selected: false,
                lead: false,
            }
        })
        .collect()
}

type Read = Result<Vec<VersionRow>, String>;

/// The versions of the project on the preview.
#[derive(Default)]
pub struct SkeletonVersions {
    view: VersionsView,
    project: Option<String>,
    reading: Option<mpsc::Receiver<Read>>,
    read_at: Option<Instant>,
    waves: qnc_program_waveform::ProgramWaves,
    filmstrips: qnc_program_filmstrip::ProgramFilmstrips,
    readers_set: bool,
}

impl std::fmt::Debug for SkeletonVersions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkeletonVersions").field("view", &self.view).field("project", &self.project).finish_non_exhaustive()
    }
}

impl SkeletonVersions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &VersionsView {
        &self.view
    }

    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.filmstrips.reading() || self.waves.reading()
    }

    /// Takes a finished read, reads again now and then, lays out the open version. Returns
    /// whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview, clips: &[ClipRow]) -> bool {
        let before = self.view.clone();
        let project = preview.project_target().cloned();
        let key = project.as_ref().map(|target| target.uri().to_string());
        if key != self.project {
            *self = Self { project: key, ..Self::default() };
        }
        if !self.readers_set {
            self.waves.set_reader(preview.artifact_reader());
            self.filmstrips.set_reader(preview.artifact_reader());
            self.readers_set = true;
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
                if self.view.opened().is_none() {
                    self.view.open = self.view.versions.last().map(|row| row.version.version_id);
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
        self.lay_out(preview, clips);
        self.view != before
    }

    /// The open version as Wrap rows: its elements, their waves and filmstrips, and the
    /// player's frame while it plays this version.
    fn lay_out(&mut self, preview: &SourcePreview, clips: &[ClipRow]) {
        let Some(elements) = self.view.opened().map(|row| row.elements.clone()) else {
            self.view.segments = SegmentsView::default();
            return;
        };
        let known: HashMap<String, (u64, f64)> = clips.iter().map(|clip| (clip.clip_id.clone(), (clip.duration_frames, clip.duration_seconds))).collect();
        let spans = spans(&elements);
        let total = spans.last().map_or(0, |span| span.1);
        let rate = elements.iter().find_map(|element| clip_rate(&known, &element.clip_id));
        let frames = |clip: &str| known.get(clip).map(|(frames, _)| *frames);
        let placements: Vec<_> = elements
            .iter()
            .zip(&spans)
            .map(|(element, (start, end))| qnc_program_waveform::Placement { clip_id: element.clip_id.clone(), channel: element.a1.channel, program_start: *start, program_end: *end, source_in: element.in_frame, source_out: element.out_frame, level_db: element.a1.level_db, off: element.a1.off })
            .collect();
        let pictures: Vec<_> = elements
            .iter()
            .zip(&spans)
            .filter(|(element, _)| element.kind == ElementKind::Ton)
            .map(|(element, (start, end))| qnc_program_filmstrip::PicturePlacement { clip_id: element.clip_id.clone(), program_start: *start, program_end: *end, source_in: element.in_frame, source_out: element.out_frame })
            .collect();
        let inputs = piece_inputs(&elements);
        let playhead = preview.shown_pieces().filter(|shown| *shown == inputs.as_slice()).and_then(|_| preview.program_frame());
        let mut segments = SegmentsView {
            rows: rows(&elements, &spans, rate),
            timebase: rate.map(|rate| (rate, 1)),
            total_frames: total,
            playhead,
            ..SegmentsView::default()
        };
        segments.peaks = self.waves.peaks(total, &placements, &[], frames);
        segments.pictures = self.filmstrips.pictures(&pictures, frames, |_| None);
        self.view.segments = segments;
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

    /// An action on the rows of the open version (a pointer on them, a key while they have the
    /// focus): a click is the playhead, Space plays or pauses, the arrows step a frame. The
    /// version plays as a SubSegment does; anything else on the rows does nothing (they are
    /// never the Story's).
    pub fn act_on_rows(&mut self, event: &ActionEvent, preview: &mut SourcePreview) -> bool {
        let Some(elements) = self.view.opened().map(|row| row.elements.clone()).filter(|elements| !elements.is_empty()) else {
            return false;
        };
        let inputs = piece_inputs(&elements);
        let shown = preview.shown_pieces() == Some(inputs.as_slice());
        let at = if shown { preview.program_frame().unwrap_or(0) } else { 0 };
        let last = spans(&elements).last().map_or(0, |span| span.1.saturating_sub(1));
        let frame = |value: Option<i64>| value.and_then(|frame| u64::try_from(frame).ok()).map(|frame| frame.min(last));
        match event.action_id.as_str() {
            "timeline_cue_frame" => frame(event.value).is_some_and(|frame| preview.show_pieces(&inputs, frame)),
            "play_pause" => {
                if !shown {
                    preview.show_pieces(&inputs, 0);
                }
                preview.toggle_play()
            }
            "step_back_frame" => preview.show_pieces(&inputs, at.saturating_sub(1)),
            "step_forward_frame" => preview.show_pieces(&inputs, (at + 1).min(last)),
            _ => false,
        }
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
    use qnc_skeleton_db::Lane;

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

    #[test]
    fn the_elements_are_wrap_rows_one_after_another_with_their_pauses_and_no_markers() {
        let mut second = element("b", 10, 60);
        (second.gap_before, second.kind, second.element_id) = (25, ElementKind::Off, 2);
        let elements = vec![element("a", 100, 200), second];
        let spans = spans(&elements);
        assert_eq!(spans, vec![(0, 100), (125, 175)]);
        let rows = rows(&elements, &spans, Some(50));
        assert_eq!((rows[0].kind, rows[1].kind), (SegmentKind::Ton, SegmentKind::Off));
        assert_eq!(rows[0].duration_label, "2:00");
        assert_eq!((rows[1].start_frame, rows[1].end_frame, rows[1].source_in_frame), (125, 175, 10));
        let inputs = piece_inputs(&elements);
        assert_eq!((inputs[0].gap_before, inputs[1].gap_before), (0, 25));
        assert_eq!(inputs[1].a1, Some((Some(0), 0, false)));
    }
}
