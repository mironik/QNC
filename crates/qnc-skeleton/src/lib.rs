//! The S-Story skeleton of a project (user 2026-10-31: "in S-Story they are not segments as in
//! Story; they are clips, source-story, elements with the same standing and handling as source
//! clips"). Its elements (sent from the script of Media Assist Audio, `qnc-skeleton-db`) lie one
//! after another on one flat timeline in place of the source timeline and are edited as the
//! parts of a SubSegment are (`qnc-subsegment-edit`): their edges, A1, places, the pause and
//! join before each. Every change is written to the skeleton at once; neither the script nor
//! the Story changes.

use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use qnc_clip_row::ClipRow;
use qnc_script_db::Piece;
use qnc_skeleton_db::{Element, ElementKind, Lane, Operation};
use qnc_source_preview::SourcePreview;
use qnc_subsegment_edit::{Answer, Clips, PieceWrite, SubsegmentEdit, SubsegmentView};

pub const MODULE_ID: &str = "qnc.module.skeleton";
/// The panel the skeleton is shown on.
pub const PANEL: &str = "skeleton";
/// The skeleton opens on the timeline, at element `target` (its index; none: the start).
pub const OPEN: &str = "skeleton_open";

/// The key the skeleton is open under in the edit (one skeleton per project).
const KEY: i64 = 0;
/// How often the skeleton is read again (another process may change it).
const READ_EVERY: Duration = Duration::from_secs(2);

/// One element as the panel lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementRow {
    pub kind: ElementKind,
    pub clip_name: String,
    pub length: String,
    /// Its start on the skeleton's axis.
    pub start: u64,
}

/// What the skeleton shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkeletonView {
    pub elements: Vec<ElementRow>,
    pub total_frames: u64,
    /// The skeleton open in place of the source timeline.
    pub subsegment: Option<SubsegmentView>,
    pub message: String,
}

#[derive(Default)]
pub struct Skeleton {
    view: SkeletonView,
    elements: Vec<Element>,
    project: Option<String>,
    reading: Option<mpsc::Receiver<Result<Vec<Element>, String>>>,
    read_at: Option<Instant>,
    writing: Option<mpsc::Receiver<Result<(), String>>>,
    clips: Clips,
    edit: SubsegmentEdit,
    /// The skeleton opens once its project's elements are first read.
    opened: bool,
}

impl std::fmt::Debug for Skeleton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Skeleton").field("view", &self.view).finish_non_exhaustive()
    }
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &SkeletonView {
        &self.view
    }

    /// Whether a read or a write runs (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.writing.is_some() || self.edit.busy()
    }

    /// The parts the player should play and the frame to show, once.
    pub fn take_pieces_request(&mut self) -> Option<(Vec<qnc_source_preview::PieceInput>, u64)> {
        self.edit.take_pieces_request()
    }

    /// The elements as the edit takes them: source parts with their clips' names and lengths.
    fn pieces(&self) -> Vec<Piece> {
        self.elements.iter().map(|element| piece_of(element, &self.clips)).collect()
    }

    /// Reads the skeleton again now and then, follows what the player plays. Returns whether
    /// the view changed.
    pub fn poll(&mut self, preview: &SourcePreview, clips: &[ClipRow]) -> bool {
        let before = self.view.clone();
        if self.clips.len() != clips.len() || clips.iter().any(|clip| !self.clips.contains_key(&clip.clip_id)) {
            self.clips = clips.iter().map(|clip| (clip.clip_id.clone(), (clip.name.clone(), clip.duration_frames, clip.duration_seconds))).collect();
        }
        let project = preview.project_target().cloned();
        let key = project.as_ref().map(|target| target.uri().to_string());
        if key != self.project {
            *self = Self { project: key, clips: std::mem::take(&mut self.clips), ..Self::default() };
        }
        if let Some(done) = &self.writing {
            if let Ok(written) = done.try_recv() {
                self.writing = None;
                self.read_at = None;
                if let Err(error) = written {
                    self.view.message = error;
                }
            }
        }
        if let Some(done) = &self.reading {
            if let Ok(read) = done.try_recv() {
                self.reading = None;
                match read {
                    Ok(elements) => self.elements = elements,
                    Err(error) => self.view.message = error,
                }
                if !self.opened && !self.elements.is_empty() {
                    self.opened = true;
                    self.edit.open(KEY, &self.pieces());
                }
            }
        }
        if let Some(project) = project.filter(|_| self.reading.is_none() && self.writing.is_none() && self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY)) {
            self.read_at = Some(Instant::now());
            let (send, read) = mpsc::channel();
            if std::thread::Builder::new().name("qnc-skeleton-read".into()).spawn(move || _ = send.send(qnc_skeleton_db::read(&project))).is_ok() {
                self.reading = Some(read);
            }
        }
        let pieces = self.pieces();
        self.view.elements = rows(&pieces, &self.elements);
        self.view.total_frames = qnc_subsegment_edit::subsegment_view(KEY, &pieces, None).total_frames;
        if self.edit.key().is_some() {
            let settled = self.reading.is_none() && self.writing.is_none();
            self.edit.follow(Some(&pieces), settled, &self.clips, preview, self.project.as_deref());
        }
        self.view.subsegment = self.edit.view().cloned();
        self.view != before
    }

    /// The skeleton's own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>, preview: &SourcePreview) -> Option<bool> {
        let pieces = self.pieces();
        if action_id == OPEN {
            if pieces.is_empty() {
                self.view.message = qnc_ui_text::text("skeleton.empty").into();
                return Some(true);
            }
            self.edit.open(KEY, &pieces);
            if let Some(start) = target.and_then(|index| index.parse::<usize>().ok()).and_then(|index| self.view.elements.get(index)).map(|row| row.start) {
                self.edit.act(qnc_subsegment_edit::SUBSEGMENT_CUE, Some(&start.to_string()), &pieces, &self.clips);
            }
            self.view.subsegment = self.edit.view().cloned();
            return Some(true);
        }
        let answer = self.edit.act(action_id, target, &pieces, &self.clips)?;
        self.view.subsegment = self.edit.view().cloned();
        Some(match answer {
            Answer::Done(changed) => changed,
            Answer::Write(write) => self.write(write, preview),
        })
    }

    /// A change of the edit written to the skeleton (off the form thread); it is read again
    /// when the write is done.
    fn write(&mut self, write: PieceWrite, preview: &SourcePreview) -> bool {
        let Some(target) = preview.project_target().cloned() else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        let id = |index: usize| self.elements.get(index).map(|element| element.element_id);
        let operation = match write {
            PieceWrite::Set { index, piece } => {
                let Some(element) = self.elements.get(index) else { return false };
                Operation::Update {
                    element_id: element.element_id,
                    in_frame: piece.from_frame,
                    out_frame: piece.to_frame,
                    a1: Lane { channel: piece.a1_channel.unwrap_or(element.a1.channel), level_db: piece.a1_level_db, off: piece.a1_off },
                    gap_before: piece.gap_before,
                    cut: piece.cut,
                }
            }
            PieceWrite::Join { index, gap, cut } => {
                let Some(element) = self.elements.get(index) else { return false };
                Operation::Update { element_id: element.element_id, in_frame: element.in_frame, out_frame: element.out_frame, a1: element.a1, gap_before: gap, cut }
            }
            PieceWrite::Move { index, earlier } => match id(index) {
                Some(element_id) => Operation::Move { element_id, earlier },
                None => return false,
            },
            PieceWrite::Remove { index } => match id(index) {
                Some(element_id) => Operation::Remove { element_id },
                None => return false,
            },
        };
        let (send, done) = mpsc::channel();
        if std::thread::Builder::new().name("qnc-skeleton-write".into()).spawn(move || _ = send.send(qnc_skeleton_db::change(&target, operation))).is_ok() {
            self.writing = Some(done);
        }
        true
    }
}

/// An element as a source part: its clip's name and length, its own A1 (the channel, level and
/// off it keeps), the pause and join before it.
fn piece_of(element: &Element, clips: &Clips) -> Piece {
    let clip = clips.get(&element.clip_id);
    let frames = element.out_frame.saturating_sub(element.in_frame);
    let fps = clip.filter(|(_, _, seconds)| *seconds > 0.0).map_or(0, |(_, all, seconds)| (*all as f64 / seconds).round() as u32);
    let name = match (clip, element.kind) {
        (Some((name, _, _)), _) => name.clone(),
        // A voice record is no clip of the pool: its name is its record's.
        (None, ElementKind::Off) => element.clip_id.trim_start_matches("voice-").to_string(),
        (None, ElementKind::Ton) => element.clip_id.clone(),
    };
    Piece {
        clip_id: element.clip_id.clone(),
        clip_name: name,
        from_frame: element.in_frame,
        to_frame: element.out_frame,
        start_tc: String::new(),
        end_tc: String::new(),
        length: if fps > 0 { qnc_source_timecode::format_frames(frames, fps) } else { frames.to_string() },
        seconds: clip.filter(|(_, all, seconds)| *all > 0 && *seconds > 0.0).map_or(0.0, |(_, all, seconds)| frames as f64 * seconds / *all as f64),
        text: String::new(),
        gap_before: element.gap_before,
        cut: element.cut,
        a1_channel: Some(element.a1.channel),
        a1_level_db: element.a1.level_db,
        a1_off: element.a1.off,
    }
}

fn rows(pieces: &[Piece], elements: &[Element]) -> Vec<ElementRow> {
    let shown = qnc_subsegment_edit::subsegment_view(KEY, pieces, None);
    shown
        .parts
        .iter()
        .zip(elements)
        .map(|(part, element)| ElementRow { kind: element.kind, clip_name: part.clip_name.clone(), length: part.length.clone(), start: part.start })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(id: i64, kind: ElementKind, clip: &str, in_frame: u64, out_frame: u64, gap: u64) -> Element {
        let lane = Lane { channel: 0, level_db: 0, off: false };
        Element { element_id: id, sort_index: id, kind, clip_id: clip.into(), in_frame, out_frame, a1: lane, a2: lane, a1_lead_frames: 0, a1_tail_frames: 0, gap_before: gap, cut: false, origin: String::new() }
    }

    #[test]
    fn the_elements_lie_one_after_another_as_source_parts() {
        let clips: Clips = [("clip-b".to_string(), ("Mironik 2002".to_string(), 5000, 100.0))].into();
        let elements = vec![element(1, ElementKind::Off, "voice-P_OFF1", 0, 120, 0), element(2, ElementKind::Ton, "clip-b", 50, 300, 10)];
        let pieces: Vec<Piece> = elements.iter().map(|element| piece_of(element, &clips)).collect();
        assert_eq!(pieces[0].clip_name, "P_OFF1", "a voice record is named by its record");
        assert_eq!(pieces[1].clip_name, "Mironik 2002");
        let rows = rows(&pieces, &elements);
        assert_eq!(rows.iter().map(|row| row.start).collect::<Vec<_>>(), vec![0, 130], "a pause moves the next element on");
    }
}
