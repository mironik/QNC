//! Program of an edited story: Ton (picture and sound) and Off (sound only)
//! segments in order (docs/93 R8-R14), their M markers and M-M slots (docs/93
//! R15-R23, docs/94).
//!
//! Everything lives in the project database (`story_parts`, `story_markers`,
//! `story_marker_slots`, `story_state`, table module `qnc-program-db`), by the v5 rule
//! `qnc-story-segment-timeline.mdc`. This component reads it, writes
//! through the one intermediary of the project database without blocking the caller, and
//! turns it into a program model on one frame axis. The program playhead belongs to
//! the Wrap view (`qnc-wrap-session`); navigation only asks for a program frame. It knows no
//! form and no application, and never plays, probes or opens media.

mod confirm;
mod edit;
mod lanes;
mod nav;
mod slot_move;
mod slot_trim;
mod marker_edit;
mod sync;

// The view and the commands are their own pieces (audit 2026-10-10); this model keeps
// the state, reads and writes the story and composes them.
pub use qnc_program_commands::{SegmentCommand, SegmentKind};
pub use qnc_program_view::{
    check_move, cover_spans, duration_color_key, duration_label, first_empty_slot,
    neighbour_marker, neighbour_segment, neighbour_slot, program, program_frame, resolve,
    slot_at, slots, source_at, CoverSpan, EditStill, MarkerPin, SegmentPart, SegmentRow, SegmentsView, Slot,
};

pub use qnc_sync_cover::SyncPreview;
pub use sync::SyncSpace;

use qnc_db_broker::ProjectDbTarget;
use qnc_program_db::{
    Data as StoryData, Operation, ProgramCover, ProgramMarker, ProgramSegment, ProgramSlot,
    StoryPending, StoryReader, StoryWriter,
};
use qnc_virtual_shots::{
    Data as ShotData, Operation as ShotOperation, VirtualShotsPending, VirtualShotsWriter,
};

/// A cover waiting for its B-roll virtual shot (v5 `create_cover`: the virtual shot
/// of the source IN/OUT first, then the cover of the slot).
#[derive(Debug, Clone)]
struct NewCover {
    slot_id: String,
    clip_id: String,
    in_frame: u64,
    out_frame: u64,
    fps_num: u32,
    fps_den: u32,
    /// Source channel (zero based) heard on A2.
    a2_source_channel: u16,
}

pub const MODULE_ID: &str = "qnc.module.program-segments";
/// How long a Cover slot or Overwrite is named in the status line.
const COVER_NOTICE: std::time::Duration = std::time::Duration::from_secs(3);
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

/// A segment to append: the source IN/OUT the player confirmed and their timebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSegment {
    pub kind: SegmentKind,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub fps_num: u32,
    pub fps_den: u32,
    /// Source channel (zero based) heard on A1.
    pub a1_source_channel: u16,
}

/// The program of the active project: read, written and turned into a view.
#[derive(Default)]
pub struct ProgramSegments {
    /// The last Cover slot / Overwrite asked for (see `SegmentsView::cover_pressed`).
    cover_pressed: Option<(bool, std::time::Instant)>,
    /// The project database of the active project (its story and virtual shots).
    target: Option<ProjectDbTarget>,
    /// Where the lease of the story's prerender is read, and its waker.
    prerender: Option<qnc_content_store::ContentTarget>,
    waker: qnc_prerender_launch::PrerenderWaker,
    /// The stored waves of the clips the program plays (v5 program waveform).
    waves: qnc_program_waveform::ProgramWaves,
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
    /// Undo and redo steps kept in the database, from the last read.
    history: (u64, u64),
    /// The rate of the story from the last read (set by its first segment).
    story_rate: Option<(u32, u32)>,
    selected: Option<String>,
    selected_marker: Option<String>,
    selected_slot: Option<String>,
    selected_cover: Option<String>,
    /// What Talking Head, Voice over and covers take from the source view.
    source: SourcePick,
    /// The channels heard on A1 and A2, a lane taken with the keyboard and the large
    /// wave (piece `qnc-lane-choice`).
    lanes: qnc_lane_choice::LaneChoice,
    /// Clip lengths in their own frames, as the form last gave them (`view_with_waves`):
    /// the edit pictures know where their clip ends.
    clip_frames: std::collections::HashMap<String, u64>,
    channels: qnc_source_channels::SourceChannels,
    sync: qnc_sync_cover::SyncCover,
    /// IN was pressed since the last source (arms Sync).
    sync_in_pressed: bool,
    /// The closed Sync slot writes its cover as soon as it is stored (Enter during the
    /// play, or the play reached an M marker or the source OUT).
    sync_commit: bool,
    /// Pokrivalice was chosen; at first neither way of making covers is (user 2026-10-10).
    covers_chosen: bool,
    /// The segment being edited and the clip with IN/OUT the source view should open.
    editing: Option<String>,
    /// The clip, its IN/OUT and the mark to take (`Some(true)` IN, `Some(false)` OUT).
    source_request: Option<(String, u64, u64, Option<bool>)>,
    /// A segment edge taken with Ctrl+I/O on the segment panel (`edit.rs`).
    edge: Option<edit::EdgeTrim>,
    /// The Wrap frame to go back to once that edge is written or dropped.
    wrap_return: Option<u64>,
    /// The marker being moved and its draft program frame (Enter writes it).
    marker_edit: Option<(String, u64)>,
    /// The slot taken with Ctrl+S and its draft shift (`slot_move.rs`).
    slot_move: Option<slot_move::SlotMove>,
    /// The slot edge taken with Ctrl+I/O on the segment panel (`slot_trim.rs`).
    slot_trim: Option<slot_trim::SlotTrim>,
    /// A cover taken with Ctrl+click; Delete removes it.
    cover_taken: Option<String>,
    /// A marker move was confirmed; Enter stays taken until it lands.
    marker_committing: bool,
    /// Program playhead of the Wrap view (`qnc-wrap-session`), given by the caller.
    playhead: Option<u64>,
    /// Program frame the user pointed at, for the Wrap view to take once.
    seek: Option<u64>,
    writer: Option<StoryWriter>,
    /// Story writes sent and not answered yet, by key.
    pending: Vec<(String, StoryPending)>,
    /// The B-roll virtual shots of covers, and the cover each one waits to place.
    shots: Option<VirtualShotsWriter>,
    covers_waiting: Vec<(VirtualShotsPending, NewCover)>,
    /// B-roll shots saved for covers: (shot, clip, IN frame), for their posters.
    created_cover_shots: Vec<(String, String, u64)>,
    /// The source IN (clip, frame) a cover was made from: used, it makes no short.
    cover_marks: Option<(String, u64)>,
    /// Write whose completion selects the new segment.
    pending_create: Option<String>,
    sequence: u64,
    view: SegmentsView,
    /// The read of the story running on its own thread, and whether another is asked
    /// meanwhile: the form's thread never waits on the database (a reader waits while a
    /// background job writes the project database).
    reading: Option<std::sync::mpsc::Receiver<qnc_program_db::Result<StoryRead>>>,
    read_again: bool,
}

/// One read of the story: segments, markers, slots, covers and the stored selection.
type StoryRead = (
    Vec<ProgramSegment>,
    Vec<ProgramMarker>,
    Vec<ProgramSlot>,
    Vec<ProgramCover>,
    qnc_program_db::StorySelection,
);

impl ProgramSegments {
    pub fn new() -> Self {
        Self::default()
    }

    /// Points to the project database of the active project (its story and the
    /// virtual shots of covers) and reads it.
    /// `waves` reads the stored clip waves of that project (the program wave).
    pub fn configure(
        &mut self,
        story: ProjectDbTarget,
        project_id: &str,
        waves: Option<std::sync::Arc<dyn qnc_program_waveform::TimelineArtifactRead>>,
        prerender: Option<qnc_content_store::ContentTarget>,
    ) {
        let same = self.target.as_ref().map(ProjectDbTarget::uri) == Some(story.uri());
        if !same {
            *self = Self::default();
            self.waves.set_reader(waves);
        }
        self.target = Some(story);
        self.project_id = project_id.to_string();
        self.adopt_selection = !same;
        // The story's prerender is always on (user 2026-10-12): woken when the project
        // opens and whenever the stored story changes.
        self.prerender = prerender;
        self.waker.wake(self.prerender.as_ref());
        self.reload();
    }

    pub fn view(&self) -> &SegmentsView {
        &self.view
    }

    /// The view with its program wave; `clips` gives each clip's length in the frames
    /// its IN/OUT use (a clip wave spans the whole clip).
    pub fn view_with_waves<'a>(&mut self, clips: impl Iterator<Item = (&'a str, u64)>) -> SegmentsView {
        let durations: std::collections::HashMap<&str, u64> = clips.collect();
        if durations.len() != self.clip_frames.len() || durations.iter().any(|(id, frames)| self.clip_frames.get(*id) != Some(frames)) {
            self.clip_frames = durations.iter().map(|(id, frames)| (id.to_string(), *frames)).collect();
            let message = self.view.message.clone();
            self.refresh_view(message);
        }
        let (segments, covers) = self.view.wave_placements();
        let mut view = self.view.clone();
        view.peaks = self.waves.peaks(view.total_frames, &segments, &covers, |id| durations.get(id).copied());
        // A zoom picture at its clip's first or last frame (user 2026-10-13).
        qnc_program_view::mark_clip_edges(&mut view.edit_stills, |id| durations.get(id).copied());
        qnc_program_view::mark_clip_edges(&mut view.source_edit_stills, |id| durations.get(id).copied());
        view
    }

    pub fn has_pending_work(&self) -> bool {
        !self.pending.is_empty() || !self.covers_waiting.is_empty()
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
    /// `content` is the project content of the clip: its stored record gives the audio
    /// channels A1 is chosen from.
    pub fn set_source(&mut self, source: SourcePick, content: Option<&qnc_source_channels::ContentTarget>) {
        let count = self.channels.for_clip(content, source.clip_id.as_deref());
        let previous_in = (self.source.clip_id == source.clip_id)
            .then_some(self.source.in_mark)
            .flatten();
        if self.source.clip_id != source.clip_id {
            self.lanes.new_clip(); // another clip: A1 on channel 1, A2 on channel 2 (user rule)
            self.view.lane_taken = None;
            self.view.a1_choice = self.lanes.choice(0);
            self.view.a2_choice = self.lanes.choice(1);
        }
        self.source = source;
        self.view.source_edit_stills = self.edge_stills();
        self.arm_sync_on_new_in(previous_in);
        self.set_source_channels(count);
    }

    /// Whether the playable program (segments or covers) changed since the last
    /// call; the Wrap view then opens it again.
    pub fn take_program_changed(&mut self) -> bool {
        std::mem::take(&mut self.program_changed)
    }

    /// Rereads the program from the database, on a thread of its own; `poll` takes the
    /// result. A reload asked while one runs reads once more after it.
    pub fn reload(&mut self) {
        let Some(target) = self.target.clone() else {
            return;
        };
        if self.reading.is_some() {
            self.read_again = true;
            return;
        }
        let (send, receive) = std::sync::mpsc::channel();
        let started = std::thread::Builder::new().name("qnc-story-read".into()).spawn(move || {
            let read = StoryReader::open(&target).and_then(|mut client| {
                Ok((
                    client.list_segments()?,
                    client.list_markers()?,
                    client.list_slots()?,
                    client.list_covers()?,
                    client.read_story_selection()?,
                ))
            });
            let _ = send.send(read);
        });
        match started {
            Ok(_) => self.reading = Some(receive),
            Err(error) => self.refresh_view(error.to_string()),
        }
    }

    /// Whether a read of the story or of its waves is still running.
    pub fn reading(&self) -> bool {
        self.reading.is_some() || self.waves.reading()
    }

    /// Takes a finished read of the story; true if one was applied.
    fn take_read(&mut self) -> bool {
        let Some(receive) = &self.reading else {
            return false;
        };
        let read = match receive.try_recv() {
            Ok(read) => read,
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("story read stopped".into()),
        };
        self.reading = None;
        self.apply_read(read);
        if std::mem::take(&mut self.read_again) {
            self.reload();
        }
        true
    }

    fn apply_read(&mut self, read: qnc_program_db::Result<StoryRead>) {
        match read {
            Ok((stored, markers, slots, covers, selection)) => {
                if stored != self.stored || covers != self.stored_covers {
                    self.program_changed = true;
                    self.waker.wake(self.prerender.as_ref());
                }
                self.stored = stored;
                self.stored_covers = covers;
                self.stored_markers = markers;
                self.stored_slots = slots;
                self.history = (selection.undo_depth, selection.redo_depth);
                self.story_rate = selection.story_fps;
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
            || ((self.slot_move.is_some() || self.slot_trim.is_some())
                && matches!(action_id, "step_back_frame" | "step_forward_frame" | "clear_focus" | "close_player"))
    }

    /// Applies a keyboard catalog action of the program; false if it is not one.
    pub fn apply_action(&mut self, action_id: &str) -> bool {
        if let Some(done) = self.trim_key(action_id) {
            return done; // a taken slot edge (segment panel)
        }
        if let Some(done) = self.slot_key(action_id) {
            return done; // a taken slot (select_current_marker_slot)
        }
        self.command_for(action_id)
            .is_some_and(|command| self.apply(command))
    }

    /// While a marker is edited, the arrows and Escape belong to it.
    fn command_for(&self, action_id: &str) -> Option<SegmentCommand> {
        if self.cover_taken.is_some() && matches!(action_id, "clear_focus" | "close_player") {
            return Some(SegmentCommand::CancelMarkerEdit);
        }
        if self.editing.is_some() && matches!(action_id, "clear_focus" | "close_player") {
            return Some(SegmentCommand::CancelEdit);
        }
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
                self.write(Operation::DeleteSegment { segment_id });
            }
            SegmentCommand::Include(segment_id) => {
                self.adopt_selection = true;
                self.write(Operation::IncludeSegment { segment_id });
            }
            SegmentCommand::Purge(segment_id) => self.write(Operation::PurgeSegment { segment_id }),
            SegmentCommand::Undo | SegmentCommand::Redo => {
                self.marker_edit = None;
                self.slot_trim = None;
                self.adopt_selection = true;
                self.write(if matches!(command, SegmentCommand::Undo) {
                    Operation::UndoStory
                } else {
                    Operation::RedoStory
                });
            }
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
                    None => self.refresh_view(qnc_ui_text::text("story.msg.no_adjacent_marker").into()),
                }
            }
            SegmentCommand::StepSlot { up } => {
                match neighbour_slot(&self.view.slots, playhead, up).cloned() {
                    Some(slot) => self.select_slot(&slot.slot_id, None),
                    None => self.refresh_view(qnc_ui_text::text("story.msg.no_adjacent_slot").into()),
                }
            }
            SegmentCommand::ProgramStart => self.cue_program(0),
            SegmentCommand::Cue(frame) => self.cue_at(frame),
            SegmentCommand::Marker => self.marker_at_playhead(),
            SegmentCommand::AddSegment(kind) => self.create_from_source(kind),
            SegmentCommand::Cover { overwrite } => self.cover_from_source(overwrite),
            SegmentCommand::SelectCover { cover_id, frame } => self.select_cover(cover_id, frame),
            SegmentCommand::TakeCover { cover_id, frame } => {
                self.select_cover(cover_id.clone(), frame);
                if self.selected_cover.as_deref() == Some(cover_id.as_str()) {
                    self.cover_taken = Some(cover_id);
                    self.refresh_view(qnc_ui_text::text("story.msg.cover_taken").into());
                }
            }
            SegmentCommand::ToggleSync => self.toggle_sync(),
            SegmentCommand::CoverMode { sync } => {
                self.covers_chosen = !sync && !self.view.rows.is_empty();
                if self.sync.enabled() != sync {
                    self.toggle_sync();
                } else {
                    self.refresh_view(String::new());
                }
            }
            SegmentCommand::CommitSync => {
                if !self.commit_marker_edit() && !self.commit_edit() {
                    if !self.finish_sync_with_cover() {
                        self.commit_sync(true);
                    }
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
            SegmentCommand::SelectSlotAtPlayhead => self.select_slot_at_playhead(),
            SegmentCommand::FocusEmptySlot => self.focus_empty_slot(),
            SegmentCommand::NavigateObject { up } => self.navigate_object(up),
            SegmentCommand::Edit(segment_id) => self.edit(segment_id),
            SegmentCommand::CancelEdit => self.cancel_edit(),
            SegmentCommand::CancelMarkerEdit => {
                if self.cover_taken.take().is_some() {
                    self.refresh_view(qnc_ui_text::text("story.msg.cover_delete_cancelled").into());
                }
                self.cancel_marker_edit();
            }
            SegmentCommand::DeleteFocused => match (self.marker_edit.take(), self.cover_taken.take()) {
                (Some((marker_id, _)), _) => {
                    self.selected_marker = None;
                    self.write(Operation::DeleteMarker { marker_id });
                }
                (None, Some(cover_id)) => {
                    self.selected_cover = None;
                    self.write(Operation::DeleteCover { cover_id });
                }
                (None, None) => self.refresh_view(
                    qnc_ui_text::text("story.msg.take_before_delete").into(),
                ),
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
            self.refresh_view(qnc_ui_text::text("story.msg.start_marker_locked").into());
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
            return self.refresh_view(qnc_ui_text::text("story.msg.marker_on_wrap").into());
        };
        // User rule: only a marker taken with Ctrl+M goes to the playhead (a draft,
        // Enter confirms); otherwise M sets a new marker.
        let operation = match self.marker_edit.clone() {
            Some((marker_id, _)) => return self.marker_to_playhead(marker_id),
            None => {
                let Some(segment) = self.view.segment_at(frame) else {
                    return self.refresh_view(qnc_ui_text::text("story.msg.no_wrap_segment").into());
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
        let key = self.write_keyed(Operation::CreateSegment {
            project_id: self.project_id.clone(),
            kind: segment.kind.db().into(),
            clip_id: segment.clip_id,
            in_frame: segment.in_frame,
            out_frame: segment.out_frame,
            fps_num: segment.fps_num,
            fps_den: segment.fps_den,
            a1_source_channel: segment.a1_source_channel,
        });
        if key.is_some() {
            self.pending_create = key;
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
            a1_source_channel: self.lanes.heard().0,
        });
    }

    /// v5 `quick_cover` / `overwrite_cover`: the marked source becomes the cover
    /// of the target slot (a B-roll virtual shot; the store replaces a cover there).
    pub fn cover_from_source(&mut self, overwrite: bool) {
        self.cover_pressed = Some((overwrite, std::time::Instant::now()));
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
        let clip_name = self.source.clip_name.clone();
        self.cover(
            NewCover {
                slot_id,
                clip_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
                a2_source_channel: self.lanes.heard().1,
            },
            clip_name,
        );
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
            self.refresh_view(qnc_ui_text::text("story.msg.pick_clip_and_marks").into());
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
        self.cover_taken = None;
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
            return self.refresh_view(qnc_ui_text::text("story.msg.pick_segment").into());
        };
        self.write(Operation::DeleteSegment { segment_id });
    }

    pub fn move_selected(&mut self, up: bool) {
        let Some(segment_id) = self.selected.clone() else {
            return self.refresh_view(qnc_ui_text::text("story.msg.pick_segment").into());
        };
        self.write(Operation::MoveSegment { segment_id, up });
    }

    fn write(&mut self, operation: Operation) {
        self.write_keyed(operation);
    }

    /// Sends a story write without waiting; its key comes back.
    fn write_keyed(&mut self, operation: Operation) -> Option<String> {
        let Some(target) = self.target.clone() else {
            self.refresh_view(qnc_ui_text::text("story.msg.db_unavailable").into());
            return None;
        };
        if self.writer.is_none() {
            match StoryWriter::spawn(target) { // never waits on the form thread
                Ok(writer) => self.writer = Some(writer),
                Err(error) => {
                    self.refresh_view(error);
                    return None;
                }
            }
        }
        let key = self.next_key("write");
        match self.writer.as_ref()?.submit(&operation) {
            Ok(pending) => {
                self.pending.push((key.clone(), pending));
                Some(key)
            }
            Err(error) => {
                self.refresh_view(error);
                None
            }
        }
    }

    /// v5 `create_cover` from source frames: the B-roll virtual shot of the source
    /// IN/OUT is made first (`qnc-virtual-shots`); the cover of the slot follows once
    /// it is saved.
    pub(crate) fn cover(&mut self, cover: NewCover, clip_name: String) {
        let Some(target) = self.target.clone() else {
            return self.refresh_view(qnc_ui_text::text("story.msg.db_unavailable").into());
        };
        if self.shots.is_none() {
            match VirtualShotsWriter::spawn(target) { // never waits on the form thread
                Ok(shots) => self.shots = Some(shots),
                Err(error) => return self.refresh_view(error),
            }
        }
        let sent = self.shots.as_ref().map(|shots| {
            shots.submit(&ShotOperation::CreateCoverShot {
                project_id: self.project_id.clone(),
                clip_id: cover.clip_id.clone(),
                clip_name,
                in_frame: cover.in_frame,
                out_frame: cover.out_frame,
            })
        });
        match sent {
            Some(Ok(pending)) => {
                self.cover_marks = Some((cover.clip_id.clone(), cover.in_frame));
                self.covers_waiting.push((pending, cover));
            }
            Some(Err(error)) => self.refresh_view(error),
            None => {}
        }
    }

    /// Applies finished writes and rereads the program. Returns whether it changed.
    pub fn poll(&mut self) -> bool {
        let read = self.take_read();
        let mut message = String::new();
        let mut landed = false;
        // Covers whose B-roll virtual shot is saved go into their slot.
        let mut place = Vec::new();
        for (pending, cover) in std::mem::take(&mut self.covers_waiting) {
            match pending.try_take() {
                None => self.covers_waiting.push((pending, cover)),
                Some(result) => {
                    landed = true;
                    match result {
                        Ok(ShotData::Created(shot_id)) => place.push((cover, shot_id)),
                        Ok(_) => message = qnc_ui_text::text("story.msg.bad_broll_reply").into(),
                        Err(error) => message = error,
                    }
                }
            }
        }
        for (cover, virtual_shot_id) in place {
            self.created_cover_shots.push((virtual_shot_id.clone(), cover.clip_id.clone(), cover.in_frame));
            self.write(Operation::CreateCover {
                project_id: self.project_id.clone(),
                slot_id: cover.slot_id,
                clip_id: cover.clip_id,
                virtual_shot_id,
                in_frame: cover.in_frame,
                out_frame: cover.out_frame,
                fps_num: cover.fps_num,
                fps_den: cover.fps_den,
                a2_source_channel: cover.a2_source_channel,
            });
        }
        let mut created = None;
        let mut waiting = Vec::new();
        for (key, pending) in std::mem::take(&mut self.pending) {
            match pending.try_take() {
                None => waiting.push((key, pending)),
                Some(result) => {
                    landed = true;
                    match result {
                        Ok(StoryData::Created(segment_id))
                            if self.pending_create.as_deref() == Some(key.as_str()) =>
                        {
                            created = Some(segment_id);
                        }
                        Ok(_) => {}
                        Err(error) => message = error,
                    }
                }
            }
        }
        self.pending = waiting;
        if !landed {
            return read;
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
        let mut view = program(&self.stored, self.selected.as_deref(), self.story_rate);
        (view.undo_depth, view.redo_depth) = self.history;
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
        if self.slot_move.as_ref().map(|slot| &slot.slot_id) != self.selected_slot.as_ref() {
            self.slot_move = None; // another selection lets the slot go
        }
        if self.slot_trim.as_ref().map(|trim| &trim.slot_id) != self.selected_slot.as_ref() {
            self.slot_trim = None; // another selection lets the slot edge go
        }
        if let Some(trim) = &self.slot_trim {
            // The draft: the markers from the slot's end on move with the trim.
            let total = view.total_frames;
            view.markers.iter_mut().filter(|pin| !pin.locked).for_each(|pin| pin.frame = trim.moved(pin.frame, total));
            for drawn in &mut view.slots {
                (drawn.start_frame, drawn.end_frame) = (trim.moved(drawn.start_frame, total), trim.moved(drawn.end_frame, total));
            }
        }
        if let Some(slot) = &self.slot_move {
            // The draft is drawn where the slot and its two markers are going.
            view.markers.iter_mut().for_each(|pin| pin.frame = slot.moved(pin.frame));
            for drawn in &mut view.slots {
                (drawn.start_frame, drawn.end_frame) = (slot.moved(drawn.start_frame), slot.moved(drawn.end_frame));
            }
        }
        if !known(
            &self.selected_cover,
            self.stored_covers
                .iter()
                .map(|cover| cover.cover_id.as_str()),
        ) {
            self.selected_cover = None;
        }
        view.covers = cover_spans(&self.stored_covers, self.selected_cover.as_deref(), view.timebase);
        if view.rows.is_empty() {
            // No segments: neither way of making covers is chosen (user 2026-10-10).
            self.covers_chosen = false;
            if self.sync.enabled() {
                self.program_changed |= self.sync.toggle();
            }
        }
        view.sync_enabled = self.sync.enabled();
        view.covers_chosen = self.covers_chosen;
        view.playhead = self.playhead;
        view.cover_pressed = self.cover_pressed;
        view.message = match self.cover_pressed {
            // A Cover slot or Overwrite asked for in the last moments says so (user
            // 2026-10-13: Overwrite gave no sign that it was taken).
            Some((overwrite, at)) if message.is_empty() && at.elapsed() < COVER_NOTICE => {
                if overwrite { qnc_ui_text::text("story.msg.overwrite_done").into() } else { qnc_ui_text::text("story.msg.cover_slot_done").into() }
            }
            _ => message,
        };
        view.editing = self.editing.clone();
        view.edit_stills = match &self.slot_trim {
            Some(trim) => qnc_program_view::trim_stills(&view, &slots(&self.stored_slots, None), &trim.slot_id, trim.start, trim.frames),
            None => qnc_program_view::edit_stills(&view, &slots(&self.stored_slots, None), &self.edit_moves()),
        };
        view.source_edit_stills = self.edge_stills();
        view.trim_frames = self.edit_moves().into_iter().map(|(_, new)| new).collect();
        qnc_program_view::mark_clip_edges(&mut view.edit_stills, |id| self.clip_frames.get(id).copied());
        self.lane_view(&mut view);
        self.view = view;
    }

    /// What the view shows of the A1/A2 lanes.
    fn lane_view(&self, view: &mut SegmentsView) {
        view.a1_choice = self.lanes.choice(0);
        view.a2_choice = self.lanes.choice(1);
        (view.lane_taken, view.wave_zoom) = (self.lanes.taken(), self.lanes.wave_zoom());
        let (a1, a2) = (self.lanes.lane(0), self.lanes.lane(1));
        (view.lane_off, view.lane_level_db) = ([a1.off, a2.off], [a1.level_db, a2.level_db]);
    }

    /// How many audio channels the marked clip has (its stored record); a chosen
    /// channel the clip does not have goes back to channel 1.
    pub fn set_source_channels(&mut self, count: Option<u16>) {
        if self.lanes.set_channels(count) {
            let mut view = std::mem::take(&mut self.view);
            self.lane_view(&mut view);
            self.view = view;
        }
    }

    /// The channel picker of the source timeline chose the channel heard on A1.
    pub fn choose_a1_channel(&mut self, channel: u16) -> bool {
        self.choose_channel(0, channel)
    }

    /// The source channels kept on A1 and A2 (zero based): what is written to the story.
    pub fn heard_channels(&self) -> (u16, u16) {
        self.lanes.heard()
    }

    /// The source channels the preview plays on A1 and A2 now and their gains (0 when a
    /// lane is off): a taken lane's draft is heard before Enter keeps it; Escape goes back
    /// to what is kept.
    pub fn audible_channels(&self) -> ((u16, u16), (f32, f32)) {
        self.lanes.audible()
    }

    /// The channel picker of the source timeline chose the channel heard on A2.
    pub fn choose_a2_channel(&mut self, channel: u16) -> bool {
        self.choose_channel(1, channel)
    }

    /// On the taken lane a click only drafts the channel (Enter keeps it, Escape drops it;
    /// user 2026-10-14); otherwise the chosen channel is kept at once.
    fn choose_channel(&mut self, lane: u8, channel: u16) -> bool {
        if self.lanes.taken().is_some_and(|(taken, _)| taken == lane) {
            let drafted = self.lanes.set_draft(lane, channel);
            self.view.lane_taken = self.lanes.taken();
            return drafted;
        }
        if !self.lanes.choose(lane, channel) {
            return false;
        }
        let mut view = std::mem::take(&mut self.view);
        self.lane_view(&mut view);
        self.view = view;
        true
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

impl ProgramSegments {
    /// The B-roll shots saved for covers since the last call: (shot, clip, IN frame).
    /// The caller gives each its poster (user rule 2026-10-01).
    pub fn take_created_cover_shots(&mut self) -> Vec<(String, String, u64)> {
        std::mem::take(&mut self.created_cover_shots)
    }
}

impl ProgramSegments {
    /// Whether the source IN shown now is the one a cover was just made from (user rule
    /// 2026-10-01: a cover goes to the B-roll tab and its IN/OUT makes no virtual short;
    /// an Enter after a Sync that ended on a marker is that Sync's confirmation).
    pub fn marks_used_by_cover(&self, clip_id: Option<&str>, in_frame: Option<u64>) -> bool {
        match (&self.cover_marks, clip_id, in_frame) {
            (Some((clip, used)), Some(shown), Some(frame)) => clip == shown && *used == frame,
            _ => false,
        }
    }
}
