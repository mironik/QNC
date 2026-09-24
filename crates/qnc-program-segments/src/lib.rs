//! Program of an edited story: Ton (picture and sound) and Off (sound only)
//! segments in order (docs/93 R8-R14), their M markers and M-M slots (docs/93
//! R15-R23, docs/94).
//!
//! Everything lives in the project database (`story_parts`, `story_markers`,
//! `story_marker_slots`, `story_state`, owner `qnc-content-store`), by the v5 rule
//! `qnc-story-segment-timeline.mdc`. This component reads it, writes
//! through the serialized content write transport without blocking the caller, and
//! turns it into a program model on one frame axis. The program playhead comes only
//! from the player's confirmed frame; navigation only asks for a cue. It knows no
//! form and no application, and never plays, probes or opens media.

mod markers;

pub use markers::{
    check_move, first_empty_slot, neighbour_marker, neighbour_segment, neighbour_slot,
    program_frame, resolve, slot_at, slots, source_at, MarkerPin, Slot,
};

use qnc_content_store::{
    Access, ContentTarget, ContentWriteData, ContentWriteTransport, Operation, ProgramMarker,
    ProgramSegment, ProgramSlot,
};

pub const MODULE_ID: &str = "qnc.module.program-segments";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ton carries its picture and sound; Off only its sound (the picture comes later
/// from covers, black until then). Both are heard on A1; covers on A2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Ton,
    Off,
}

impl SegmentKind {
    fn db(self) -> &'static str {
        match self {
            Self::Ton => "tonovi",
            Self::Off => "offovi",
        }
    }

    fn from_db(value: &str) -> Option<Self> {
        match value {
            "tonovi" => Some(Self::Ton),
            "offovi" => Some(Self::Off),
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

/// One row of the Segment tab (v5 `segment_parts`): every segment in stored
/// order, deleted ones greyed (`active` false) and not clickable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPart {
    pub segment_id: String,
    pub kind: SegmentKind,
    pub duration_label: String,
    pub active: bool,
    pub selected: bool,
}

/// What a form shows: the program in order, its markers and slots, timebase,
/// length and the program playhead confirmed by the player.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SegmentsView {
    pub rows: Vec<SegmentRow>,
    /// The Segment tab list, deleted segments included.
    pub parts: Vec<SegmentPart>,
    pub markers: Vec<MarkerPin>,
    pub slots: Vec<Slot>,
    /// Story timebase (`num`, `den`); `None` while the program is empty.
    pub timebase: Option<(u32, u32)>,
    pub total_frames: u64,
    /// Program frame of the confirmed player picture in the Wrap view.
    pub playhead: Option<u64>,
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

    pub fn selected_marker(&self) -> Option<&MarkerPin> {
        self.markers.iter().find(|pin| pin.selected)
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
    let mut parts = Vec::with_capacity(segments.len());
    for segment in segments {
        let Some(kind) = SegmentKind::from_db(&segment.kind) else {
            continue;
        };
        let frames = segment.out_frame.saturating_sub(segment.in_frame);
        parts.push(SegmentPart {
            segment_id: segment.segment_id.clone(),
            kind,
            duration_label: duration_label(frames, segment.fps_num, segment.fps_den),
            active: segment.active,
            selected: segment.active && selected == Some(segment.segment_id.as_str()),
        });
        if !segment.active {
            continue;
        }
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
        parts,
        timebase: segments
            .iter()
            .find(|segment| segment.active && SegmentKind::from_db(&segment.kind).is_some())
            .map(|first| (first.fps_num, first.fps_den)),
        total_frames: start,
        rows,
        ..SegmentsView::default()
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

/// What a form, the navigation bar or the keyboard asks of the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentCommand {
    Select(String),
    /// Previous (`up`) or next segment: select it and cue its start (arrows, ⏮ ⏭).
    Step {
        up: bool,
    },
    /// Swap the selected segment with its neighbour (Up / Down).
    Move {
        up: bool,
    },
    /// The selected marker first, otherwise the selected segment (docs/94 section 2).
    DeleteSelected,
    SelectMarker(String),
    /// A click on a slot: select it, playhead at the clicked program frame (v5
    /// `SelectMarkerSlot { frame }`).
    SelectSlot {
        slot_id: String,
        frame: u64,
    },
    /// Previous or next M marker from the playhead.
    StepMarker {
        up: bool,
    },
    /// Previous or next M-M slot.
    StepSlot {
        up: bool,
    },
    /// Cue the start of the program (🏠).
    ProgramStart,
    /// A program frame the user pointed at.
    Cue(u64),
    /// M: a new marker at the playhead, or the selected marker moved there (docs/94 7a).
    Marker,
}

impl SegmentCommand {
    /// The command behind a keyboard catalog `action_id`.
    pub fn from_action(action_id: &str) -> Option<Self> {
        Some(match action_id {
            "add_marker" | "add_marker_continue" => Self::Marker,
            "delete_marker" | "delete_part" | "delete_segment" => Self::DeleteSelected,
            "playlist_input_start" => Self::ProgramStart,
            "step_prev_part" => Self::Step { up: true },
            "step_next_part" => Self::Step { up: false },
            _ => return None,
        })
    }
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

/// What the caller asks of the player for a Wrap cue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CueStep {
    /// Open this clip with this source frame as its first picture.
    Open(String, u64),
    Cue(u64),
}

/// Where the player must go: this clip at this source frame (Wrap view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub segment_id: String,
    pub clip_id: String,
    pub source_frame: u64,
}

/// The program of the active project: read, written and turned into a view.
#[derive(Default)]
pub struct ProgramSegments {
    target: Option<ContentTarget>,
    project_id: String,
    stored: Vec<ProgramSegment>,
    stored_markers: Vec<ProgramMarker>,
    stored_slots: Vec<ProgramSlot>,
    /// Take the stored selection on the next read (on open, and after writes that
    /// may move it, v5 `story_state`).
    adopt_selection: bool,
    selected: Option<String>,
    selected_marker: Option<String>,
    selected_slot: Option<String>,
    /// Segment the Wrap view shows in the player.
    wrap_segment: Option<String>,
    playhead: Option<u64>,
    cue: Option<Cue>,
    /// Source frame asked of the player: older confirmed frames do not move the
    /// playhead back.
    awaiting: Option<u64>,
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
            *self = Self::default();
        }
        self.target = Some(target);
        self.project_id = project_id.to_string();
        self.adopt_selection = !same;
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

    /// Where the player must go, once; the caller opens the clip and cues it.
    pub fn take_cue(&mut self) -> Option<Cue> {
        self.cue.take()
    }

    /// One step of the Wrap view per repaint, from what the player shows: another
    /// clip is opened straight at the cue frame; the shown clip is cued once the
    /// player confirms a picture of it. The confirmed picture also becomes the
    /// program playhead.
    pub fn drive_player(
        &mut self,
        shown_clip: Option<&str>,
        confirmed_frame: Option<u64>,
    ) -> Option<CueStep> {
        self.follow_player(shown_clip, confirmed_frame);
        let cue = self.cue.as_ref()?;
        if shown_clip != Some(cue.clip_id.as_str()) {
            return self
                .cue
                .take()
                .map(|cue| CueStep::Open(cue.clip_id, cue.source_frame));
        }
        confirmed_frame?;
        self.cue.take().map(|cue| CueStep::Cue(cue.source_frame))
    }

    /// Whether the Wrap view (a segment in the player) is on.
    pub fn in_wrap(&self) -> bool {
        self.wrap_segment.is_some()
    }

    /// Leaves the Wrap view: the player shows a source clip again.
    pub fn leave_wrap(&mut self) {
        self.wrap_segment = None;
        self.cue = None;
        self.awaiting = None;
        self.set_playhead(None);
    }

    /// The player's confirmed picture: in the Wrap view it becomes the program
    /// playhead when it lies inside the shown segment. Nothing is interpolated.
    pub fn follow_player(&mut self, clip_id: Option<&str>, confirmed_frame: Option<u64>) {
        if let Some(awaiting) = self.awaiting {
            if confirmed_frame != Some(awaiting) {
                return;
            }
            self.awaiting = None;
        }
        let playhead = self
            .wrap_segment
            .as_deref()
            .and_then(|id| self.view.rows.iter().find(|row| row.segment_id == id))
            .filter(|row| Some(row.clip_id.as_str()) == clip_id)
            .zip(confirmed_frame)
            .and_then(|(row, frame)| program_frame(row, frame));
        if playhead.is_some() || clip_id.is_none() {
            self.set_playhead(playhead);
        }
    }

    fn set_playhead(&mut self, playhead: Option<u64>) {
        if self.playhead != playhead {
            self.playhead = playhead;
            self.view.playhead = playhead;
        }
    }

    /// Rereads the program from the database.
    pub fn reload(&mut self) {
        let Some(target) = &self.target else {
            return;
        };
        let read = target.open(Access::ReadOnly).and_then(|mut client| {
            Ok((
                client.list_segments()?,
                client.list_markers()?,
                client.list_slots()?,
                client.read_story_selection()?,
            ))
        });
        match read {
            Ok((stored, markers, slots, selection)) => {
                self.stored = stored;
                self.stored_markers = markers;
                self.stored_slots = slots;
                if std::mem::take(&mut self.adopt_selection) {
                    let some = |id: String| (!id.is_empty()).then_some(id);
                    self.selected = some(selection.selected_part_id);
                    self.selected_slot = some(selection.selected_slot_id);
                }
                self.refresh_view(String::new());
            }
            Err(error) => self.refresh_view(error),
        }
    }

    /// Whether a keyboard catalog action belongs to the program.
    pub fn handles(&self, action_id: &str) -> bool {
        SegmentCommand::from_action(action_id).is_some()
    }

    /// Applies a keyboard catalog action of the program; false if it is not one.
    pub fn apply_action(&mut self, action_id: &str) -> bool {
        SegmentCommand::from_action(action_id).is_some_and(|command| self.apply(command))
    }

    /// Applies a command; returns true so the caller repaints.
    pub fn apply(&mut self, command: SegmentCommand) -> bool {
        let playhead = self.playhead.unwrap_or(0);
        match command {
            SegmentCommand::Select(segment_id) => self.select(&segment_id),
            SegmentCommand::Step { up } => self.step(up),
            SegmentCommand::Move { up } => self.move_selected(up),
            SegmentCommand::DeleteSelected => self.delete_selected(),
            SegmentCommand::SelectMarker(marker_id) => self.select_marker(&marker_id),
            SegmentCommand::SelectSlot { slot_id, frame } => {
                self.select_slot(&slot_id, Some(frame))
            }
            SegmentCommand::StepMarker { up } => {
                match neighbour_marker(&self.view.markers, playhead, up).cloned() {
                    Some(pin) => self.select_marker(&pin.marker_id),
                    None => self.refresh_view("Nema prethodnog/sljedeceg M markera.".into()),
                }
            }
            SegmentCommand::StepSlot { up } => {
                match neighbour_slot(&self.view.slots, playhead, up).cloned() {
                    Some(slot) => self.select_slot(&slot.slot_id, None),
                    None => self.refresh_view("Nema prethodnog/sljedeceg M-M slota.".into()),
                }
            }
            SegmentCommand::ProgramStart => self.cue_program(0),
            SegmentCommand::Cue(frame) => self.cue_at(frame),
            SegmentCommand::Marker => self.marker_at_playhead(),
        }
        true
    }

    fn step(&mut self, up: bool) {
        let from = self
            .selected
            .as_deref()
            .and_then(|id| self.view.rows.iter().find(|row| row.segment_id == id));
        let target = match from {
            Some(row) => {
                let index = self.view.rows.iter().position(|r| r == row).unwrap_or(0);
                let next = if up {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(self.view.rows.len() - 1)
                };
                self.view.rows.get(next)
            }
            None if up => self.view.rows.last(),
            None => self.view.rows.first(),
        };
        if let Some(segment_id) = target.map(|row| row.segment_id.clone()) {
            self.select(&segment_id);
        }
    }

    /// Selects a segment (clears the selected marker and slot, docs/94 3.1) and
    /// cues its start in the Wrap view.
    pub fn select(&mut self, segment_id: &str) {
        let Some(start) = self
            .view
            .rows
            .iter()
            .find(|row| row.segment_id == segment_id)
            .map(|row| row.start_frame)
        else {
            return;
        };
        self.selected = Some(segment_id.to_string());
        self.selected_marker = None;
        self.selected_slot = None;
        self.write(Operation::SelectPart {
            part_id: segment_id.to_string(),
        });
        self.refresh_view(String::new());
        self.cue_program(start);
    }

    fn select_marker(&mut self, marker_id: &str) {
        let Some(frame) = self
            .view
            .markers
            .iter()
            .find(|pin| pin.marker_id == marker_id)
            .map(|pin| pin.frame)
        else {
            return;
        };
        // v5 `select_marker`: the start marker is locked and is not selected.
        if frame == 0 {
            self.selected_marker = None;
            self.refresh_view("Početni M marker je zaključan.".into());
        } else {
            self.selected_marker = Some(marker_id.to_string());
            self.selected_slot = None;
            self.refresh_view(String::new());
        }
        self.cue_program(frame);
    }

    /// Selects a slot; the playhead goes to the clicked frame, else to the slot
    /// start (v5 `SelectMarkerSlot` and `select_adjacent_marker_slot`).
    fn select_slot(&mut self, slot_id: &str, frame: Option<u64>) {
        let Some(start) = self
            .view
            .slots
            .iter()
            .find(|slot| slot.slot_id == slot_id)
            .map(|slot| slot.start_frame)
        else {
            return;
        };
        self.selected_slot = Some(slot_id.to_string());
        self.selected_marker = None;
        self.write(Operation::SelectSlot {
            slot_id: slot_id.to_string(),
        });
        self.refresh_view(String::new());
        self.cue_at(frame.unwrap_or(start));
    }

    /// A program frame the user pointed at on a Wrap segment: the segment under it
    /// becomes the selected one (v5 keeps `selected_part_id` on the playhead
    /// segment) and the player is asked for that frame.
    fn cue_at(&mut self, frame: u64) {
        if let Some(segment) = self.view.segment_at(frame) {
            if self.selected.as_deref() != Some(segment.segment_id.as_str()) {
                self.selected = Some(segment.segment_id.clone());
                self.refresh_view(String::new());
            }
        }
        self.cue_program(frame);
    }

    /// Wrap view: the program playhead goes to the frame at once (v5
    /// `set_wrap_playhead_frame`) and the player is asked for that picture; its
    /// confirmed frames take over once it reaches the cue.
    fn cue_program(&mut self, frame: u64) {
        let Some(cue) = source_at(&self.view, frame).map(|(segment, source_frame)| Cue {
            segment_id: segment.segment_id.clone(),
            clip_id: segment.clip_id.clone(),
            source_frame,
        }) else {
            return;
        };
        self.set_playhead(Some(frame.min(self.view.total_frames)));
        self.awaiting = Some(cue.source_frame);
        self.wrap_segment = Some(cue.segment_id.clone());
        self.cue = Some(cue);
    }

    /// M on the Wrap segment under the playhead: the marker is placed on that
    /// segment at the frame inside it (v5 `create_marker_from_part_frame`); the
    /// program only shows it. A selected marker moves there (docs/94 7a).
    fn marker_at_playhead(&mut self) {
        let Some(frame) = self.playhead else {
            return self.refresh_view("M marker se stavlja na Wrap segment (playhead).".into());
        };
        let operation = match self.selected_marker.clone() {
            Some(marker_id) => {
                if let Err(error) = check_move(&self.view.markers, &marker_id, frame) {
                    return self.refresh_view(error);
                }
                Operation::MoveMarker {
                    marker_id,
                    program_frame: frame,
                }
            }
            None => {
                let Some(segment) = self.view.segment_at(frame) else {
                    return self.refresh_view("Nema Wrap segmenta pod playheadom.".into());
                };
                Operation::CreateMarker {
                    part_id: segment.segment_id.clone(),
                    local_frame: frame - segment.start_frame,
                }
            }
        };
        self.write(operation);
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
            active: true,
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
        if let Some(marker_id) = self.selected_marker.clone() {
            self.selected_marker = None;
            return self.write(Operation::DeleteMarker { marker_id });
        }
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

    fn write(&mut self, operation: Operation) {
        let key = self.next_key("write");
        self.send(|writes| writes.write_program(key, operation));
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
        let mut created = None;
        for completion in completions {
            match completion.result {
                Ok(result) => {
                    if let ContentWriteData::Created(segment_id) = result.data {
                        if self.pending_create.as_deref() == Some(completion.key.as_str()) {
                            created = Some(segment_id);
                        }
                    }
                }
                Err(error) => message = error,
            }
        }
        if let Some(segment_id) = created {
            // A new segment becomes the selection, in the database too.
            self.selected = Some(segment_id.clone());
            self.write(Operation::SelectPart {
                part_id: segment_id,
            });
        }
        // The store may have moved the selection (v5 `delete_part`): take it once
        // every write of this component has landed.
        self.adopt_selection = !self.has_pending_work();
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
        if !known(
            &self.selected,
            self.stored.iter().map(|row| row.segment_id.as_str()),
        ) {
            self.selected = None;
        }
        if !known(
            &self.wrap_segment,
            self.stored.iter().map(|row| row.segment_id.as_str()),
        ) {
            self.wrap_segment = None;
        }
        let mut view = program(&self.stored, self.selected.as_deref());
        view.markers = resolve(&view, &self.stored_markers, self.selected_marker.as_deref());
        if !known(
            &self.selected_marker,
            view.markers.iter().map(|pin| pin.marker_id.as_str()),
        ) {
            self.selected_marker = None;
        }
        view.slots = slots(&self.stored_slots, self.selected_slot.as_deref());
        if !known(
            &self.selected_slot,
            view.slots.iter().map(|slot| slot.slot_id.as_str()),
        ) {
            self.selected_slot = None;
        }
        view.playhead = self.playhead;
        view.message = message;
        self.view = view;
    }
}

/// Whether a remembered id is still among the ids.
fn known<'a>(id: &Option<String>, mut ids: impl Iterator<Item = &'a str>) -> bool {
    id.as_deref().is_some_and(|id| ids.any(|known| known == id))
}

#[cfg(test)]
mod tests;
