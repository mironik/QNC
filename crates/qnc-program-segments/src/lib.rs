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
mod delete;
mod zoom;
mod edit;
mod nav;
mod slot_move;
mod slot_trim;
mod overlap;
mod cut_part;
pub use cut_part::CutPart;
pub use overlap::OverlapDraft;
mod marker_edit;
mod part_sound;
mod sync;
mod commands;
mod create;
mod refresh;
mod select;
mod story_read;
mod writes;

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
    /// Source channel (zero based) heard on A2, its level in dB and whether it is off.
    a2_source_channel: u16,
    a2_level_db: i8,
    a2_off: bool,
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
    /// A1 and A2 as the source timeline keeps them (piece `qnc-source-timeline`): what a
    /// segment, cover or Sync cover takes with it.
    pub lanes: [qnc_lane_choice::Lane; 2],
    /// The source timeline holds the current Enter (a lane taken or just kept).
    pub holds_enter: bool,
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
            lanes: [qnc_lane_choice::Lane::default(), qnc_lane_choice::Lane { channel: 1, ..Default::default() }],
            holds_enter: false,
        }
    }

    /// The lanes the source timeline keeps and whether it holds the Enter.
    pub fn with_lanes(mut self, lanes: [qnc_lane_choice::Lane; 2], holds_enter: bool) -> Self {
        (self.lanes, self.holds_enter) = (lanes, holds_enter);
        self
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
    /// Source channel (zero based) heard on A1, its level in dB and whether it is off.
    pub a1_source_channel: u16,
    pub a1_level_db: i8,
    pub a1_off: bool,
}

/// What a measured loudness belongs to: program length, rate, waves read, A1 and A2 parts.
type LoudnessKey = (u64, (u32, u32), u64, Vec<qnc_program_waveform::Placement>, Vec<qnc_program_waveform::Placement>);

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
    /// The stored filmstrips of the clips the program shows (Wrap row pictures).
    filmstrips: qnc_program_filmstrip::ProgramFilmstrips,
    /// The preview normalises the program's A1 and A2 (user 2026-10-24); off when the
    /// application starts.
    normalize: bool,
    /// The program's loudness and what it was measured for (placements, waves, rate).
    loudness: Option<(LoudnessKey, qnc_program_loudness::ProgramLoudness)>,
    /// The Wrap rows show a narrow video row instead of the filmstrip.
    video_compact: bool,
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
    /// A taken sound let go by another take (`let_go_edits`), until the panel asks.
    sound_dropped: bool,
    /// The last refused take (`confirm.rs` `refused`), shown by `qnc-alert`.
    alert: Option<qnc_alert::Alert>,
    /// The slot shown enlarged in the place of the source timeline (`zoom.rs`).
    zoom_slot: Option<zoom::ZoomOn>,
    /// What Talking Head, Voice over and covers take from the source view.
    source: SourcePick,
    /// The sound of a stored segment or cover taken on the Wrap row (`part_sound.rs`).
    part_sound: qnc_part_sound::PartSound,
    /// Clip lengths in their own frames, as the form last gave them (`view_with_waves`):
    /// the edit pictures know where their clip ends.
    clip_frames: std::collections::HashMap<String, u64>,
    sync: qnc_sync_cover::SyncCover,
    /// IN was pressed since the last source (arms Sync).
    sync_in_pressed: bool,
    /// The last IN was pressed while the source played (Sync starts once it is armed).
    sync_flight: bool,
    /// Armed on the fly: Sync starts with the next poll.
    sync_flight_ready: bool,
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
    /// The J or L of a cover taken with Ctrl+J / Ctrl+L (`overlap.rs`).
    overlap: Option<overlap::OverlapEdit>,
    /// What of the taken marker's cut moves (`cut_part.rs`).
    cut_part: cut_part::CutPart,
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
    /// The cover writes waiting for the store, with the B-roll shot made for each: a
    /// refused cover takes its shot away again.
    cover_writes: Vec<(String, String)>,
    /// The source IN (clip, frame) a cover was made from: used, it makes no short.
    cover_marks: Option<(String, u64)>,
    /// Where the cover being written ends on the program: once it is in, the Wrap playhead
    /// goes there, so the next B fills the next empty part (user 2026-10-18: subclips laid
    /// one after another over the offs).
    cover_end: Option<u64>,
    /// Write whose completion selects the new segment.
    pending_create: Option<String>,
    sequence: u64,
    view: SegmentsView,
    /// The read of the story running on its own thread, and whether another is asked
    /// meanwhile: the form's thread never waits on the database (a reader waits while a
    /// background job writes the project database).
    reading: Option<std::sync::mpsc::Receiver<qnc_program_db::Result<StoryRead>>>,
    /// The audio channels of each clip of the story, from its stored media record (external
    /// audit 2026-10-17: A1/A2 of a segment hang on the wave no more; the wave only draws).
    clip_channels: ClipChannels,
    read_again: bool,
}

/// One read of the story: segments, markers, slots, covers and the stored selection.
type StoryRead = (
    Vec<ProgramSegment>,
    Vec<ProgramMarker>,
    Vec<ProgramSlot>,
    Vec<ProgramCover>,
    qnc_program_db::StorySelection,
    ClipChannels,
);

/// The audio channel count of clips of the story, from their stored media records.
type ClipChannels = std::collections::HashMap<String, Option<u16>>;

/// The clips whose channel count was read; any other is read again.
fn counted(channels: &ClipChannels) -> std::collections::HashSet<String> {
    channels.iter().filter(|(_, count)| count.is_some()).map(|(clip, _)| clip.clone()).collect()
}

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
            self.filmstrips.set_reader(waves.clone());
            self.waves.set_reader(waves);
        }
        if !same {
            // B-roll shots whose cover never came (the program stopped between the two
            // writes) go; a minute old, so a cover another process is writing stays.
            if let Ok(shots) = VirtualShotsWriter::spawn(story.clone()) {
                let _ = shots.submit(&ShotOperation::ForgetUnconfirmedCoverShots { older_than_seconds: 60 });
                self.shots = Some(shots);
            }
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
    /// `clips`: each clip's id, length and poster (shown in a Wrap row until its filmstrip is
    /// made).
    pub fn view_with_waves<'a>(
        &mut self,
        clips: impl Iterator<Item = (&'a str, u64, Option<&'a std::sync::Arc<qnc_image_assets::RgbaImage>>)>,
    ) -> SegmentsView {
        let clips: Vec<_> = clips.collect();
        let posters: std::collections::HashMap<&str, std::sync::Arc<qnc_image_assets::RgbaImage>> =
            clips.iter().filter_map(|(id, _, poster)| Some((*id, (*poster)?.clone()))).collect();
        let durations: std::collections::HashMap<&str, u64> = clips.iter().map(|(id, frames, _)| (*id, *frames)).collect();
        if durations.len() != self.clip_frames.len() || durations.iter().any(|(id, frames)| self.clip_frames.get(*id) != Some(frames)) {
            self.clip_frames = durations.iter().map(|(id, frames)| (id.to_string(), *frames)).collect();
            let message = self.view.message.clone();
            self.refresh_view(message);
        }
        let (mut segments, mut covers) = self.view.wave_placements();
        // A taken part's wave is its drafted channel, level and off, that part alone (the log
        // 2026-10-22: the zoom of A2 was empty; a drafted level scaled the whole row).
        if let Some((lane, id, draft)) = self.part_sound_draft_now() {
            let index = if lane == 0 { self.view.rows.iter().position(|row| row.segment_id == id) } else { self.view.covers.iter().position(|cover| cover.cover_id == id) };
            let placements = if lane == 0 { &mut segments } else { &mut covers };
            if let Some(placement) = index.and_then(|index| placements.get_mut(index)) {
                (placement.channel, placement.level_db, placement.off) = (draft.channel, draft.level_db, draft.off);
            }
        }
        let mut view = self.view.clone();
        view.peaks = self.waves.peaks(view.total_frames, &segments, &covers, |id| durations.get(id).copied());
        view.loudness = self.loudness_of(&view, segments, covers);
        view.normalize = self.normalize;
        view.video_compact = self.video_compact;
        if !self.video_compact {
            view.pictures = self.filmstrips.pictures(&view.picture_placements(), |id| durations.get(id).copied(), |id| posters.get(id).cloned());
        }
        // A zoom picture at its clip's first or last frame (user 2026-10-13).
        qnc_program_view::mark_clip_edges(&mut view.edit_stills, |id| durations.get(id).copied());
        qnc_program_view::mark_clip_edges(&mut view.source_edit_stills, |id| durations.get(id).copied());
        view
    }

    /// The loudness of A1 and A2 as heard (drafts too), measured again only when the program,
    /// the waves read or the rate changed.
    fn loudness_of(
        &mut self,
        view: &SegmentsView,
        segments: Vec<qnc_program_waveform::Placement>,
        covers: Vec<qnc_program_waveform::Placement>,
    ) -> Option<qnc_program_loudness::ProgramLoudness> {
        let rate = view.timebase?;
        let key = (view.total_frames, rate, self.waves.revision(), segments, covers);
        if let Some((kept, measured)) = &self.loudness {
            if *kept == key {
                return Some(*measured);
            }
        }
        let waves = &self.waves;
        let measured = qnc_program_loudness::measure(key.0, rate, &key.3, &key.4, |id| {
            waves.clip_loudness(id).map(|(channels, duration_frames)| qnc_program_loudness::ClipMeasure { channels, duration_frames })
        });
        self.loudness = Some((key, measured));
        Some(measured)
    }

    /// What the preview normalises to the project's targets: the measured A1 and A2, while
    /// normalising is on.
    pub fn normalization(&self) -> Option<[Option<f64>; 2]> {
        let measured = self.loudness.as_ref().map(|(_, measured)| *measured).unwrap_or_default();
        self.normalize.then_some([measured.a1_lufs, measured.a2_lufs])
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

    /// The source the user marked, given by the caller each repaint, with the A1/A2
    /// lanes its source timeline keeps.
    pub fn set_source(&mut self, source: SourcePick) {
        let previous_in = (self.source.clip_id == source.clip_id)
            .then_some(self.source.in_mark)
            .flatten();
        self.source = source;
        self.view.source_edit_stills = self.edge_stills();
        self.arm_sync_on_new_in(previous_in);
    }

    /// Whether the playable program (segments or covers) changed since the last
    /// call; the Wrap view then opens it again.
    pub fn take_program_changed(&mut self) -> bool {
        std::mem::take(&mut self.program_changed)
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
