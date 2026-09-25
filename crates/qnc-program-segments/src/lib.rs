//! Program of an edited story: Ton (picture and sound) and Off (sound only)
//! segments in order (docs/93 R8-R14), their M markers and M-M slots (docs/93
//! R15-R23, docs/94).
//!
//! Everything lives in the project database (`story_parts`, `story_markers`,
//! `story_marker_slots`, `story_state`, owner `qnc-content-store`), by the v5 rule
//! `qnc-story-segment-timeline.mdc`. This component reads it, writes
//! through the serialized content write transport without blocking the caller, and
//! turns it into a program model on one frame axis. The program playhead belongs to
//! the Wrap view (`qnc-wrap-session`); navigation only asks for a program frame. It knows no
//! form and no application, and never plays, probes or opens media.

mod marker_edit;
mod markers;
mod sync;

pub use markers::{
    check_move, first_empty_slot, neighbour_marker, neighbour_segment, neighbour_slot,
    program_frame, resolve, slot_at, slots, source_at, MarkerPin, Slot,
};

pub use qnc_sync_cover::SyncPreview;
pub use sync::SyncSpace;

use qnc_content_store::{
    Access, ContentTarget, ContentWriteData, ContentWriteTransport, Operation, ProgramCover,
    ProgramMarker, ProgramSegment, ProgramSlot,
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

/// A cover on the program axis: drawn over its slot (v5 `segment_timeline_covers`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverSpan {
    pub cover_id: String,
    pub slot_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub selected: bool,
}

/// The source the user marked for Talking Head, Voice over and covers: the chosen
/// clip, its confirmed IN/OUT and the timebase the player confirmed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourcePick {
    pub clip_id: Option<String>,
    pub clip_name: String,
    pub marks: Option<(u64, u64)>,
    /// The IN mark itself, set by the user (Sync arms on it).
    pub in_mark: Option<u64>,
    pub duration_frames: u64,
    pub timebase: Option<(i64, i64)>,
}

impl SourcePick {
    pub fn new(
        clip_id: Option<&str>,
        clip_name: Option<&str>,
        marks: Option<(u64, u64)>,
        (in_mark, duration_frames): (Option<u64>, u64),
        timebase: Option<(i64, i64)>,
    ) -> Self {
        Self {
            clip_id: clip_id.map(str::to_string),
            clip_name: clip_name.unwrap_or_default().to_string(),
            marks,
            in_mark,
            duration_frames,
            timebase,
        }
    }
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
    /// Covers by program frame.
    pub covers: Vec<CoverSpan>,
    /// Sync/B-roll is on.
    pub sync_enabled: bool,
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

    pub fn selected_slot(&self) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.selected)
    }

    pub fn selected_cover(&self) -> Option<&CoverSpan> {
        self.covers.iter().find(|cover| cover.selected)
    }

    /// Whether a program action can run now: B needs an empty selected slot,
    /// Overwrite a selected slot or cover; the others are always offered.
    pub fn action_enabled(&self, action_id: &str) -> bool {
        match SegmentCommand::from_action(action_id) {
            Some(SegmentCommand::Cover { overwrite: false }) => self.quick_cover_slot().is_ok(),
            Some(SegmentCommand::Cover { overwrite: true }) => self.overwrite_cover_slot().is_ok(),
            _ => true,
        }
    }

    /// Whether an action belongs to the program.
    pub fn handles(&self, action_id: &str) -> bool {
        SegmentCommand::from_action(action_id).is_some()
    }

    /// v5 `quick_cover_target`: the selected slot, while it has no cover.
    pub fn quick_cover_slot(&self) -> Result<&Slot, String> {
        let slot = self
            .selected_slot()
            .ok_or("Odaberi marker slot za pokrivalicu")?;
        if slot.has_cover {
            return Err("Odabrani marker slot već ima pokrivalicu".into());
        }
        Ok(slot)
    }

    /// v5 `overwrite_cover_target`: the selected slot, else the selected cover's slot.
    pub fn overwrite_cover_slot(&self) -> Result<&str, String> {
        self.selected_slot()
            .map(|slot| slot.slot_id.as_str())
            .or_else(|| self.selected_cover().map(|cover| cover.slot_id.as_str()))
            .ok_or_else(|| "Odaberi marker slot za pokrivalicu".into())
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
    /// Isključi: the segment leaves the program and stays listed where it was.
    Exclude(String),
    /// Uključi: an excluded segment comes back where it was.
    Include(String),
    /// Izbriši: an excluded segment is removed for good.
    Purge(String),
    /// Replace: the segment takes the marked source (clip, IN/OUT) as its base
    /// layer and its length; its kind stays (user rule 2026-09-25).
    Replace(String),
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
    /// Talking Head (Ton) or Voice over (Off) from the marked source.
    AddSegment(SegmentKind),
    /// Cover slot / B (empty selected slot) or Overwrite / Shift+B from the marked source.
    Cover {
        overwrite: bool,
    },
    /// Ctrl+M: the selected (or nearest) marker into editing.
    EditMarker,
    /// Shift+M: always a new marker at the playhead, whatever is selected.
    AddMarker,
    /// Arrows while a marker is edited: its draft by frames.
    NudgeMarker(i64),
    /// A mouse drag of a marker: its draft on this program frame.
    DragMarker {
        marker_id: String,
        frame: u64,
    },
    /// Escape while a marker is edited.
    CancelMarkerEdit,
    /// The Delete key (user rule 2026-09-25): only what was taken with Ctrl+
    /// before (Ctrl+M: the marker) is deleted; a click never arms Delete.
    DeleteFocused,
    /// Sync/B-roll on or off.
    ToggleSync,
    /// Enter: the cover of a closed Sync slot (v5 `sync_cover_enter_or_activate_focused_item`).
    CommitSync,
    /// A click on a cover: select it, playhead at the clicked program frame.
    SelectCover {
        cover_id: String,
        frame: u64,
    },
}

impl SegmentCommand {
    /// The command behind a keyboard catalog `action_id`.
    pub fn from_action(action_id: &str) -> Option<Self> {
        Some(match action_id {
            "add_marker" => Self::Marker,
            // User rule (2026-09-25): Ctrl+key selects, Shift+key adds.
            "add_marker_continue" => Self::AddMarker,
            // One Delete press sends delete_part, delete_marker and delete_segment (both
            // scopes); delete_marker holds every delete key in every preset, so only it
            // deletes: one press, one delete (the marker, else the cover, else the segment).
            "delete_marker" => Self::DeleteFocused,
            "playlist_input_start" => Self::ProgramStart,
            "step_prev_part" => Self::Step { up: true },
            "step_next_part" => Self::Step { up: false },
            "add_ton_segment" => Self::AddSegment(SegmentKind::Ton),
            "add_off_segment" => Self::AddSegment(SegmentKind::Off),
            "quick_overwrite_cover" => Self::Cover { overwrite: false },
            "overwrite_cover" => Self::Cover { overwrite: true },
            "activate_focused_item" => Self::CommitSync,
            "select_marker" => Self::EditMarker,
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

/// The program of the active project: read, written and turned into a view.
#[derive(Default)]
pub struct ProgramSegments {
    target: Option<ContentTarget>,
    project_id: String,
    stored: Vec<ProgramSegment>,
    stored_markers: Vec<ProgramMarker>,
    stored_slots: Vec<ProgramSlot>,
    stored_covers: Vec<ProgramCover>,
    /// The segments or covers changed since the program player last took them.
    program_changed: bool,
    /// Take the stored selection on the next read (on open, and after writes that
    /// may move it, v5 `story_state`).
    adopt_selection: bool,
    selected: Option<String>,
    selected_marker: Option<String>,
    selected_slot: Option<String>,
    selected_cover: Option<String>,
    /// What Talking Head, Voice over and covers take from the source view.
    source: SourcePick,
    sync: qnc_sync_cover::SyncCover,
    /// IN was pressed since the last source (arms Sync).
    sync_in_pressed: bool,
    /// The marker being moved and its draft program frame (Enter writes it).
    marker_edit: Option<(String, u64)>,
    /// A marker move was confirmed; Enter stays taken until it lands.
    marker_committing: bool,
    /// Program playhead of the Wrap view (`qnc-wrap-session`), given by the caller.
    playhead: Option<u64>,
    /// Program frame the user pointed at, for the Wrap view to take once.
    seek: Option<u64>,
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

    /// The program frame the user pointed at, once; the Wrap view takes it.
    pub fn take_seek(&mut self) -> Option<u64> {
        self.seek.take()
    }

    /// The Wrap playhead on the program axis, given by the Wrap view; markers
    /// and navigation start from it.
    pub fn set_playhead(&mut self, playhead: Option<u64>) {
        if self.playhead != playhead {
            self.playhead = playhead;
            self.view.playhead = playhead;
        }
    }

    /// The source the user marked, given by the caller each repaint.
    pub fn set_source(&mut self, source: SourcePick) {
        let previous_in = (self.source.clip_id == source.clip_id)
            .then_some(self.source.in_mark)
            .flatten();
        self.source = source;
        self.arm_sync_on_new_in(previous_in);
    }

    /// Whether the playable program (segments or covers) changed since the last
    /// call; the Wrap view then opens it again.
    pub fn take_program_changed(&mut self) -> bool {
        std::mem::take(&mut self.program_changed)
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
                client.list_covers()?,
                client.read_story_selection()?,
            ))
        });
        match read {
            Ok((stored, markers, slots, covers, selection)) => {
                self.program_changed |= stored != self.stored || covers != self.stored_covers;
                self.stored = stored;
                self.stored_covers = covers;
                self.stored_markers = markers;
                self.stored_slots = slots;
                if std::mem::take(&mut self.adopt_selection) {
                    let some = |id: String| (!id.is_empty()).then_some(id);
                    self.selected = some(selection.selected_part_id);
                    self.selected_slot = some(selection.selected_slot_id);
                    self.selected_cover = some(selection.selected_cover_id);
                }
                self.refresh_view(String::new());
                self.resolve_sync();
            }
            Err(error) => self.refresh_view(error),
        }
    }

    /// Whether a keyboard catalog action belongs to the program.
    pub fn handles(&self, action_id: &str) -> bool {
        self.command_for(action_id).is_some()
    }

    /// Applies a keyboard catalog action of the program; false if it is not one.
    pub fn apply_action(&mut self, action_id: &str) -> bool {
        self.command_for(action_id)
            .is_some_and(|command| self.apply(command))
    }

    /// While a marker is edited, the arrows and Escape belong to it.
    fn command_for(&self, action_id: &str) -> Option<SegmentCommand> {
        if self.marker_edit.is_some() {
            match action_id {
                "step_back_frame" => return Some(SegmentCommand::NudgeMarker(-1)),
                "step_forward_frame" => return Some(SegmentCommand::NudgeMarker(1)),
                "clear_focus" | "close_player" => return Some(SegmentCommand::CancelMarkerEdit),
                _ => {}
            }
        }
        SegmentCommand::from_action(action_id)
    }

    /// Applies a command; returns true so the caller repaints.
    pub fn apply(&mut self, command: SegmentCommand) -> bool {
        let playhead = self.playhead.unwrap_or(0);
        match command {
            SegmentCommand::Select(segment_id) => self.select(&segment_id),
            SegmentCommand::Step { up } => self.step(up),
            SegmentCommand::Move { up } => self.move_selected(up),
            SegmentCommand::Exclude(segment_id) => {
                let key = self.next_key("exclude");
                self.send(|writes| writes.delete_segment(key, segment_id));
            }
            SegmentCommand::Include(segment_id) => {
                self.adopt_selection = true;
                self.write(Operation::IncludeSegment { segment_id });
            }
            SegmentCommand::Purge(segment_id) => self.write(Operation::PurgeSegment { segment_id }),
            SegmentCommand::Replace(segment_id) => {
                if let Some((clip_id, (in_frame, out_frame), (fps_num, fps_den))) =
                    self.marked_source()
                {
                    self.write(Operation::ReplaceSegment {
                        segment_id,
                        clip_id,
                        in_frame,
                        out_frame,
                        fps_num,
                        fps_den,
                    });
                }
            }
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
            SegmentCommand::AddSegment(kind) => self.create_from_source(kind),
            SegmentCommand::Cover { overwrite } => self.cover_from_source(overwrite),
            SegmentCommand::SelectCover { cover_id, frame } => self.select_cover(cover_id, frame),
            SegmentCommand::ToggleSync => self.toggle_sync(),
            SegmentCommand::CommitSync => {
                if !self.commit_marker_edit() {
                    self.commit_sync(true);
                }
            }
            SegmentCommand::EditMarker => self.edit_marker(),
            SegmentCommand::AddMarker => {
                self.marker_edit = None;
                self.selected_marker = None;
                self.marker_at_playhead();
            }
            SegmentCommand::NudgeMarker(frames) => self.nudge_marker(frames),
            SegmentCommand::DragMarker { marker_id, frame } => {
                // User rule: a marker moves only after Ctrl+M took it.
                if self.marker_edit.as_ref().map(|(id, _)| id) == Some(&marker_id) {
                    self.set_marker_draft(frame);
                }
            }
            SegmentCommand::CancelMarkerEdit => self.cancel_marker_edit(),
            SegmentCommand::DeleteFocused => match self.marker_edit.take() {
                Some((marker_id, _)) => {
                    self.selected_marker = None;
                    self.write(Operation::DeleteMarker { marker_id });
                }
                None => {
                    self.refresh_view("Brisanje traži marker prethodno uzet za uređivanje.".into())
                }
            },
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
        self.marker_edit = None;
        self.selected_slot = None;
        self.write(Operation::SelectPart {
            part_id: segment_id.to_string(),
        });
        if self.selected_cover.take().is_some() {
            self.write(Operation::SelectCover {
                cover_id: String::new(),
            });
        }
        self.refresh_view(String::new());
        self.cue_program(start);
    }

    fn select_marker(&mut self, marker_id: &str) {
        self.marker_edit = None;
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
        self.marker_edit = None;
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
    /// `set_wrap_playhead_frame`) and the Wrap view is asked for that program
    /// frame. No source clip is chosen here: Wrap plays the program (v5).
    fn cue_program(&mut self, frame: u64) {
        if self.view.is_empty() {
            return;
        }
        let frame = frame.min(self.view.total_frames);
        // Pointing elsewhere stops a Sync play: the whole program opens again.
        self.program_changed |= self.sync.cancel();
        self.set_playhead(Some(frame));
        self.seek = Some(frame);
    }

    /// M on the Wrap segment under the playhead: the marker is placed on that
    /// segment at the frame inside it (v5 `create_marker_from_part_frame`); the
    /// program only shows it. A selected marker moves there (docs/94 7a).
    fn marker_at_playhead(&mut self) {
        let Some(frame) = self.playhead else {
            return self.refresh_view("M marker se stavlja na Wrap segment (playhead).".into());
        };
        // User rule: only a marker taken with Ctrl+M goes to the playhead (a draft,
        // Enter confirms); otherwise M sets a new marker.
        let operation = match self.marker_edit.clone() {
            Some((marker_id, _)) => return self.marker_to_playhead(marker_id),
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
            a1_source_channel: 0,
        };
        let project_id = self.project_id.clone();
        if self.send(|writes| writes.create_segment(key.clone(), project_id, row)) {
            self.pending_create = Some(key);
        }
    }

    /// Talking Head / Voice over from the marked source: the chosen clip between
    /// the confirmed IN/OUT, in the timebase the player confirmed. Missing parts end
    /// in a message, never in a guessed range or rate.
    pub fn create_from_source(&mut self, kind: SegmentKind) {
        let Some((clip_id, (in_frame, out_frame), (fps_num, fps_den))) = self.marked_source()
        else {
            return;
        };
        self.create(NewSegment {
            kind,
            clip_id,
            in_frame,
            out_frame,
            fps_num,
            fps_den,
        });
    }

    /// v5 `quick_cover` / `overwrite_cover`: the marked source becomes the cover
    /// of the target slot (a B-roll virtual shot; the store replaces a cover there).
    pub fn cover_from_source(&mut self, overwrite: bool) {
        let slot = if overwrite {
            self.view.overwrite_cover_slot().map(str::to_string)
        } else {
            self.view
                .quick_cover_slot()
                .map(|slot| slot.slot_id.clone())
        };
        let slot_id = match slot {
            Ok(slot_id) => slot_id,
            Err(error) => return self.refresh_view(error),
        };
        let Some((clip_id, (in_frame, out_frame), (fps_num, fps_den))) = self.marked_source()
        else {
            return;
        };
        self.write(Operation::CreateCover {
            project_id: self.project_id.clone(),
            slot_id,
            clip_id,
            clip_name: self.source.clip_name.clone(),
            in_frame,
            out_frame,
            fps_num,
            fps_den,
        });
    }

    fn marked_source(&mut self) -> Option<(String, (u64, u64), (u32, u32))> {
        let source = &self.source;
        let rate = |value: i64| u32::try_from(value).unwrap_or(0);
        let picked = match (&source.clip_id, source.marks, source.timebase) {
            (Some(clip_id), Some(marks), Some((num, den))) => {
                Some((clip_id.clone(), marks, (rate(num), rate(den))))
            }
            _ => None,
        };
        if picked.is_none() {
            self.refresh_view("Odaberi klip i potvrdi IN i OUT na playeru.".into());
        }
        picked
    }

    /// v5 `select_cover`: clears the selected marker; the playhead goes to the click.
    fn select_cover(&mut self, cover_id: String, frame: u64) {
        if !self
            .view
            .covers
            .iter()
            .any(|cover| cover.cover_id == cover_id)
        {
            return;
        }
        self.selected_marker = None;
        self.marker_edit = None;
        self.selected_cover = Some(cover_id.clone());
        self.write(Operation::SelectCover { cover_id });
        self.refresh_view(String::new());
        self.cue_at(frame);
    }

    pub fn delete_selected(&mut self) {
        // Ctrl+M then Delete: the marker being edited goes (it is the selected one).
        self.marker_edit = None;
        if let Some(marker_id) = self.selected_marker.clone() {
            self.selected_marker = None;
            return self.write(Operation::DeleteMarker { marker_id });
        }
        // v5 `delete_selected_timeline_item`: the marker, then the cover, then the segment.
        if let Some(cover_id) = self.selected_cover.take() {
            return self.write(Operation::DeleteCover { cover_id });
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
        if self.adopt_selection {
            self.sync.landed();
            self.marker_committing = false;
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
        if !known(
            &self.selected,
            self.stored.iter().map(|row| row.segment_id.as_str()),
        ) {
            self.selected = None;
        }
        let mut view = program(&self.stored, self.selected.as_deref());
        view.markers = resolve(&view, &self.stored_markers, self.selected_marker.as_deref());
        if let Some((marker_id, draft)) = &self.marker_edit {
            // The draft is drawn where the marker is going.
            for pin in view
                .markers
                .iter_mut()
                .filter(|pin| &pin.marker_id == marker_id)
            {
                pin.frame = *draft;
            }
        }
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
        if !known(
            &self.selected_cover,
            self.stored_covers
                .iter()
                .map(|cover| cover.cover_id.as_str()),
        ) {
            self.selected_cover = None;
        }
        view.covers = self
            .stored_covers
            .iter()
            .map(|cover| CoverSpan {
                cover_id: cover.cover_id.clone(),
                slot_id: cover.slot_id.clone(),
                start_frame: cover.program_start_frame,
                end_frame: cover.program_end_frame,
                selected: self.selected_cover.as_deref() == Some(cover.cover_id.as_str()),
            })
            .collect();
        view.sync_enabled = self.sync.enabled();
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

#[cfg(test)]
mod db_tests;
