//! Program segments of an edited story: Ton (picture and sound) and Off (sound
//! only), in the order of the program (docs/93 R8-R14).
//!
//! The segments live in the project database (`program_segments`, owner
//! `qnc-content-store`). This component reads them, writes through the serialized
//! content write transport without blocking the caller, and turns them into a
//! program model: consecutive windows on one program axis, the story timebase and
//! the total duration. It knows no form and no application; any editorial form may
//! use it. It never plays, seeks, probes or opens media.

use qnc_content_store::{
    Access, ContentTarget, ContentWriteData, ContentWriteTransport, ProgramSegment,
};

pub const MODULE_ID: &str = "qnc.module.program-segments";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ton carries its picture and sound; Off only its sound (the picture comes later
/// from covers, black until then).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Ton,
    Off,
}

impl SegmentKind {
    fn db(self) -> &'static str {
        match self {
            Self::Ton => "ton",
            Self::Off => "off",
        }
    }

    fn from_db(value: &str) -> Option<Self> {
        match value {
            "ton" => Some(Self::Ton),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ton => "Ton",
            Self::Off => "Off",
        }
    }

    pub fn has_base_video(self) -> bool {
        self == Self::Ton
    }
}

/// A segment placed on the program axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentRow {
    pub segment_id: String,
    pub kind: SegmentKind,
    pub clip_id: String,
    pub source_in_frame: u64,
    pub source_out_frame: u64,
    /// Program frames `[start, end)`: segments follow each other without gaps.
    pub start_frame: u64,
    pub end_frame: u64,
    /// Seconds and frames, e.g. `12:07`.
    pub duration_label: String,
    /// `under_3`, `under_5`, `under_7` or `over_7` seconds (v5 duration marks).
    pub duration_color_key: &'static str,
    pub selected: bool,
}

impl SegmentRow {
    pub fn duration_frames(&self) -> u64 {
        self.end_frame - self.start_frame
    }
}

/// What a form shows: the program in order, its timebase and length.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SegmentsView {
    pub rows: Vec<SegmentRow>,
    /// Story timebase (`num`, `den`); `None` while the program is empty.
    pub timebase: Option<(u32, u32)>,
    pub total_frames: u64,
    /// Last controlled error of a read or write; empty otherwise.
    pub message: String,
}

impl SegmentsView {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn selected(&self) -> Option<&SegmentRow> {
        self.rows.iter().find(|row| row.selected)
    }

    /// The segment that holds a program frame; at or after the end, the last one.
    pub fn segment_at(&self, program_frame: u64) -> Option<&SegmentRow> {
        self.rows
            .iter()
            .find(|row| (row.start_frame..row.end_frame).contains(&program_frame))
            .or_else(|| {
                self.rows
                    .last()
                    .filter(|_| program_frame >= self.total_frames)
            })
    }

    /// `seconds:frames` of a program length in the story timebase.
    pub fn label(&self, frames: u64) -> String {
        self.timebase
            .map(|(num, den)| duration_label(frames, num, den))
            .unwrap_or_else(|| "--".into())
    }
}

/// Builds the program: the stored order, one window after another.
pub fn program(segments: &[ProgramSegment], selected: Option<&str>) -> SegmentsView {
    let mut start = 0u64;
    let mut rows = Vec::with_capacity(segments.len());
    for segment in segments {
        let Some(kind) = SegmentKind::from_db(&segment.kind) else {
            continue;
        };
        let frames = segment.out_frame.saturating_sub(segment.in_frame);
        rows.push(SegmentRow {
            segment_id: segment.segment_id.clone(),
            kind,
            clip_id: segment.clip_id.clone(),
            source_in_frame: segment.in_frame,
            source_out_frame: segment.out_frame,
            start_frame: start,
            end_frame: start + frames,
            duration_label: duration_label(frames, segment.fps_num, segment.fps_den),
            duration_color_key: duration_color_key(frames, segment.fps_num, segment.fps_den),
            selected: selected == Some(segment.segment_id.as_str()),
        });
        start += frames;
    }
    SegmentsView {
        timebase: segments
            .iter()
            .find(|segment| SegmentKind::from_db(&segment.kind).is_some())
            .map(|first| (first.fps_num, first.fps_den)),
        total_frames: start,
        rows,
        message: String::new(),
    }
}

/// `seconds:frames` with the frame rate rounded to whole frames per second (v5).
pub fn duration_label(frames: u64, fps_num: u32, fps_den: u32) -> String {
    let fps = whole_fps(fps_num, fps_den);
    if fps == 0 {
        return "0:00".into();
    }
    format!("{}:{:02}", frames / fps, frames % fps)
}

/// v5 duration marks: 3, 5 and 7 seconds.
pub fn duration_color_key(frames: u64, fps_num: u32, fps_den: u32) -> &'static str {
    if fps_num == 0 || fps_den == 0 {
        return "over_7";
    }
    let seconds = frames as f64 * f64::from(fps_den) / f64::from(fps_num);
    match seconds {
        s if s < 3.0 => "under_3",
        s if s < 5.0 => "under_5",
        s if s < 7.0 => "under_7",
        _ => "over_7",
    }
}

fn whole_fps(fps_num: u32, fps_den: u32) -> u64 {
    if fps_den == 0 {
        return 0;
    }
    (f64::from(fps_num) / f64::from(fps_den)).round().max(0.0) as u64
}

/// What a form or the keyboard asks of the program segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentCommand {
    Select(String),
    /// Select the segment before (`up`) or after the selected one (arrow keys).
    Step {
        up: bool,
    },
    /// Swap the selected segment with its neighbour (Up / Down).
    Move {
        up: bool,
    },
    DeleteSelected,
}

/// A segment to append: the source IN/OUT the player confirmed and their timebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSegment {
    pub kind: SegmentKind,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub fps_num: u32,
    pub fps_den: u32,
}

/// The segments of the active project: read, written and turned into a view.
#[derive(Default)]
pub struct ProgramSegments {
    target: Option<ContentTarget>,
    project_id: String,
    stored: Vec<ProgramSegment>,
    selected: Option<String>,
    writes: Option<ContentWriteTransport>,
    /// Write whose completion selects the new segment.
    pending_create: Option<String>,
    sequence: u64,
    view: SegmentsView,
}

impl ProgramSegments {
    pub fn new() -> Self {
        Self::default()
    }

    /// Points to the content database of the active project and reads it.
    pub fn configure(&mut self, target: ContentTarget, project_id: &str) {
        let same = self.target.as_ref().map(ContentTarget::uri) == Some(target.uri());
        if !same {
            self.writes = None;
            self.selected = None;
            self.pending_create = None;
        }
        self.target = Some(target);
        self.project_id = project_id.to_string();
        self.reload();
    }

    pub fn view(&self) -> &SegmentsView {
        &self.view
    }

    pub fn has_pending_work(&self) -> bool {
        self.writes
            .as_ref()
            .is_some_and(ContentWriteTransport::has_pending)
    }

    /// Rereads the program from the database.
    pub fn reload(&mut self) {
        let Some(target) = &self.target else {
            return;
        };
        match target
            .open(Access::ReadOnly)
            .and_then(|mut client| client.list_segments())
        {
            Ok(stored) => {
                self.stored = stored;
                self.refresh_view(String::new());
            }
            Err(error) => self.refresh_view(error),
        }
    }

    /// Applies a command; returns true so the caller repaints.
    pub fn apply(&mut self, command: SegmentCommand) -> bool {
        match command {
            SegmentCommand::Select(segment_id) => self.select(&segment_id),
            SegmentCommand::Step { up } => self.step(up),
            SegmentCommand::Move { up } => self.move_selected(up),
            SegmentCommand::DeleteSelected => self.delete_selected(),
        }
        true
    }

    fn step(&mut self, up: bool) {
        let index = self
            .stored
            .iter()
            .position(|row| Some(row.segment_id.as_str()) == self.selected.as_deref());
        let next = match (index, up) {
            (None, _) if self.stored.is_empty() => return,
            (None, true) => self.stored.len() - 1,
            (None, false) => 0,
            (Some(index), true) => index.saturating_sub(1),
            (Some(index), false) => (index + 1).min(self.stored.len() - 1),
        };
        let segment_id = self.stored[next].segment_id.clone();
        self.select(&segment_id);
    }

    pub fn select(&mut self, segment_id: &str) {
        if self.stored.iter().any(|row| row.segment_id == segment_id) {
            self.selected = Some(segment_id.to_string());
            self.refresh_view(String::new());
        }
    }

    /// Appends a segment at the end of the program; it becomes selected once saved.
    pub fn create(&mut self, segment: NewSegment) {
        let key = self.next_key("create");
        let row = ProgramSegment {
            segment_id: String::new(),
            kind: segment.kind.db().into(),
            sort_index: 0,
            clip_id: segment.clip_id,
            in_frame: segment.in_frame,
            out_frame: segment.out_frame,
            fps_num: segment.fps_num,
            fps_den: segment.fps_den,
        };
        let project_id = self.project_id.clone();
        if self.send(|writes| writes.create_segment(key.clone(), project_id, row)) {
            self.pending_create = Some(key);
        }
    }

    /// Talking Head / Voice over from the source preview: the chosen clip between the
    /// confirmed IN/OUT, in the timebase the player confirmed. Missing parts end in
    /// a message, never in a guessed range or rate.
    pub fn create_from_source(
        &mut self,
        kind: SegmentKind,
        clip_id: Option<&str>,
        marks: Option<(u64, u64)>,
        timebase: Option<(i64, i64)>,
    ) {
        let (Some(clip_id), Some((in_frame, out_frame)), Some((num, den))) =
            (clip_id, marks, timebase)
        else {
            return self.refresh_view("Odaberi klip i potvrdi IN i OUT na playeru.".into());
        };
        self.create(NewSegment {
            kind,
            clip_id: clip_id.to_string(),
            in_frame,
            out_frame,
            fps_num: u32::try_from(num).unwrap_or(0),
            fps_den: u32::try_from(den).unwrap_or(0),
        });
    }

    pub fn delete_selected(&mut self) {
        let Some(segment_id) = self.selected.clone() else {
            return self.refresh_view("Odaberi segment.".into());
        };
        let key = self.next_key("delete");
        self.send(|writes| writes.delete_segment(key, segment_id));
    }

    pub fn move_selected(&mut self, up: bool) {
        let Some(segment_id) = self.selected.clone() else {
            return self.refresh_view("Odaberi segment.".into());
        };
        let key = self.next_key("move");
        self.send(|writes| writes.move_segment(key, segment_id, up));
    }

    /// Applies finished writes and rereads the program. Returns whether it changed.
    pub fn poll(&mut self) -> bool {
        let Some(writes) = self.writes.as_mut() else {
            return false;
        };
        let completions = writes.poll();
        if completions.is_empty() {
            return false;
        }
        let mut message = String::new();
        for completion in completions {
            match completion.result {
                Ok(result) => {
                    if let ContentWriteData::SegmentCreated(segment_id) = result.data {
                        if self.pending_create.as_deref() == Some(completion.key.as_str()) {
                            self.selected = Some(segment_id);
                        }
                    }
                }
                Err(error) => message = error,
            }
        }
        self.reload();
        if !message.is_empty() {
            self.refresh_view(message);
        }
        true
    }

    fn send(
        &mut self,
        write: impl FnOnce(&mut ContentWriteTransport) -> Result<(), String>,
    ) -> bool {
        let Some(target) = self.target.clone() else {
            self.refresh_view("Projektna baza nije dostupna.".into());
            return false;
        };
        if self.writes.is_none() {
            match ContentWriteTransport::start(target) {
                Ok(writes) => self.writes = Some(writes),
                Err(error) => {
                    self.refresh_view(error);
                    return false;
                }
            }
        }
        let result = self.writes.as_mut().map_or(Ok(()), write);
        if let Err(error) = result {
            self.refresh_view(error);
            return false;
        }
        true
    }

    fn next_key(&mut self, what: &str) -> String {
        self.sequence += 1;
        format!("program_segment:{what}:{}", self.sequence)
    }

    fn refresh_view(&mut self, message: String) {
        if !self
            .stored
            .iter()
            .any(|row| Some(row.segment_id.as_str()) == self.selected.as_deref())
        {
            self.selected = None;
        }
        self.view = program(&self.stored, self.selected.as_deref());
        self.view.message = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(id: &str, kind: &str, range: (u64, u64)) -> ProgramSegment {
        ProgramSegment {
            segment_id: id.into(),
            kind: kind.into(),
            sort_index: 0,
            clip_id: "c1".into(),
            in_frame: range.0,
            out_frame: range.1,
            fps_num: 50,
            fps_den: 1,
        }
    }

    #[test]
    fn segments_follow_each_other_on_one_program_axis() {
        let view = program(
            &[stored("a", "ton", (100, 350)), stored("b", "off", (0, 60))],
            Some("b"),
        );
        assert_eq!(view.total_frames, 310);
        assert_eq!(view.timebase, Some((50, 1)));
        assert_eq!((view.rows[0].start_frame, view.rows[0].end_frame), (0, 250));
        assert_eq!(
            (view.rows[1].start_frame, view.rows[1].end_frame),
            (250, 310)
        );
        assert_eq!(view.rows[0].duration_label, "5:00");
        assert_eq!(view.rows[1].duration_label, "1:10");
        assert_eq!(
            view.selected().map(|row| row.segment_id.as_str()),
            Some("b")
        );
        assert!(view.rows[0].kind.has_base_video() && !view.rows[1].kind.has_base_video());
    }

    #[test]
    fn the_segment_at_a_frame_and_at_the_end_is_found() {
        let view = program(
            &[stored("a", "ton", (0, 10)), stored("b", "ton", (0, 5))],
            None,
        );
        assert_eq!(view.segment_at(9).unwrap().segment_id, "a");
        assert_eq!(view.segment_at(10).unwrap().segment_id, "b");
        assert_eq!(view.segment_at(99).unwrap().segment_id, "b");
        assert!(program(&[], None).segment_at(0).is_none());
    }

    #[test]
    fn duration_marks_are_three_five_and_seven_seconds() {
        assert_eq!(duration_color_key(149, 50, 1), "under_3");
        assert_eq!(duration_color_key(150, 50, 1), "under_5");
        assert_eq!(duration_color_key(300, 60000, 1001), "under_7");
        assert_eq!(duration_color_key(350, 50, 1), "over_7");
        assert_eq!(duration_label(3000, 30000, 1001), "100:00");
        assert_eq!(duration_label(10, 0, 1), "0:00");
    }

    #[test]
    fn arrow_steps_select_neighbours_and_stop_at_the_ends() {
        let mut segments = ProgramSegments::new();
        segments.stored = vec![
            stored("a", "ton", (0, 10)),
            stored("b", "ton", (0, 10)),
            stored("c", "off", (0, 10)),
        ];
        let selected = |segments: &ProgramSegments| {
            segments.view().selected().map(|row| row.segment_id.clone())
        };
        segments.apply(SegmentCommand::Step { up: false });
        assert_eq!(selected(&segments).as_deref(), Some("a"));
        segments.apply(SegmentCommand::Step { up: true });
        assert_eq!(selected(&segments).as_deref(), Some("a"));
        segments.apply(SegmentCommand::Select("c".into()));
        segments.apply(SegmentCommand::Step { up: false });
        assert_eq!(selected(&segments).as_deref(), Some("c"));
        segments.apply(SegmentCommand::Step { up: true });
        assert_eq!(selected(&segments).as_deref(), Some("b"));
        let mut empty = ProgramSegments::new();
        empty.apply(SegmentCommand::Step { up: true });
        assert!(empty.view().selected().is_none());
    }

    #[test]
    fn move_and_delete_need_a_selection() {
        let mut segments = ProgramSegments::new();
        segments.stored = vec![stored("a", "ton", (0, 10))];
        segments.apply(SegmentCommand::DeleteSelected);
        assert_eq!(segments.view().message, "Odaberi segment.");
        segments.apply(SegmentCommand::Move { up: true });
        assert_eq!(segments.view().message, "Odaberi segment.");
    }

    #[test]
    fn unknown_kinds_in_the_database_are_not_shown() {
        let view = program(&[stored("a", "voice", (0, 10))], None);
        assert!(view.is_empty());
        assert_eq!(view.label(10), "--");
    }
}
