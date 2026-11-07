//! The model of every timeline (docs/99 step 2: a core piece, no egui): the projection of a
//! timeline (its range, playhead, IN/OUT and the mark in focus), its audio lanes and the
//! intents a timeline reports. `qnc-timeline` draws it and reads the pointer; the logic that
//! keeps or changes it (preview, panels, story) needs no window.

pub const MODULE_ID: &str = "qnc.module.timeline-model";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use qnc_edit_step::Wheel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioLane {
    #[default]
    None,
    A1,
    A2,
    A3,
    A4,
}

impl AudioLane {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::A1 => "A1",
            Self::A2 => "A2",
            Self::A3 => "A3",
            Self::A4 => "A4",
        }
    }

    pub fn toggle(self, lane: Self) -> Self {
        if lane == Self::None {
            Self::None
        } else if self == lane {
            Self::None
        } else {
            lane
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimelineSourceMarkFocus {
    #[default]
    Playhead,
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineProjection {
    pub range_start_frame: u64,
    pub duration_frames: u64,
    pub playhead_frame: Option<u64>,
    pub cue_enabled: bool,
    pub source_in_frame: Option<u64>,
    pub source_out_frame: Option<u64>,
    pub source_mark_focus: TimelineSourceMarkFocus,
}

impl Default for TimelineProjection {
    fn default() -> Self {
        Self {
            range_start_frame: 0,
            duration_frames: 1,
            playhead_frame: None,
            cue_enabled: false,
            source_in_frame: None,
            source_out_frame: None,
            source_mark_focus: TimelineSourceMarkFocus::Playhead,
        }
    }
}

impl TimelineProjection {
    pub fn new(range_start_frame: u64, duration_frames: u64) -> Self {
        Self {
            range_start_frame,
            duration_frames: duration_frames.max(1),
            playhead_frame: None,
            cue_enabled: false,
            source_in_frame: None,
            source_out_frame: None,
            source_mark_focus: TimelineSourceMarkFocus::Playhead,
        }
    }

    pub fn with_playhead(mut self, playhead_frame: u64) -> Self {
        self.playhead_frame = Some(playhead_frame);
        self
    }

    pub fn with_cue_enabled(mut self, cue_enabled: bool) -> Self {
        self.cue_enabled = cue_enabled;
        self
    }

    pub fn range_start(&self) -> u64 {
        self.range_start_frame
    }

    pub fn confirmed_frame(&self) -> Option<u64> {
        self.playhead_frame
    }

    pub fn duration_frames(&self) -> u64 {
        self.duration_frames.max(1)
    }

    pub fn can_cue(&self) -> bool {
        self.cue_enabled
    }

    pub fn preserving_source_marks_from(mut self, previous: &Self) -> Self {
        self.source_in_frame = previous.source_in_frame;
        self.source_out_frame = previous.source_out_frame;
        self.source_mark_focus = previous.source_mark_focus;
        // While the player prepares the clip again (another A1/A2 channel, live 2026-10-16)
        // it has confirmed no position and its length is not known yet: the marks stay as
        // they were, not clamped to that unknown length.
        if self.playhead_frame.is_none() {
            return self;
        }
        self.normalize_source_marks()
    }

    /// IN at the confirmed frame. Marking does not take the mark (user 2026-10-20: a mark
    /// left taken made the next click on the timeline only let it go, so the playhead moved
    /// on the second click); Ctrl+I/O, a right click or Ctrl+wheel take it.
    pub fn with_source_in_at_confirmed(mut self) -> Self {
        if let Some(frame) = self.confirmed_frame() {
            self.source_in_frame = Some(self.local_frame(frame));
            if let Some(out) = self.source_out_frame {
                if out <= self.source_in_frame.unwrap_or(0) {
                    self.source_out_frame = Some(self.duration_frames());
                }
            }
        }
        self.normalize_source_marks()
    }

    pub fn with_source_out_at_confirmed(mut self) -> Self {
        if let Some(frame) = self.confirmed_frame() {
            self.source_out_frame = Some(
                self.local_frame(frame).max(
                    self.source_in_frame
                        .unwrap_or(0)
                        .saturating_add(1)
                        .min(self.duration_frames()),
                ),
            );
        }
        self.normalize_source_marks()
    }

    /// The first cut of a pair at the playhead: IN there, no OUT yet (user 2026-10-18).
    pub fn with_first_cut_at_confirmed(self) -> Self {
        match self.confirmed_frame() {
            Some(frame) => self.with_first_cut_at(frame),
            None => self.normalize_source_marks(),
        }
    }

    /// The first cut at `frame` (an absolute frame): IN there, no OUT.
    pub fn with_first_cut_at(mut self, frame: u64) -> Self {
        self.source_in_frame = Some(self.local_frame(frame));
        self.source_out_frame = None;
        self.normalize_source_marks()
    }

    /// The second cut at `frame`, the first at `first` (absolute frames): the earlier is IN,
    /// the later OUT.
    pub fn with_second_cut(mut self, first: u64, frame: u64) -> Self {
        let (earlier, later) = (self.local_frame(first.min(frame)), self.local_frame(first.max(frame)));
        self.source_in_frame = Some(earlier);
        self.source_out_frame = Some(later.max(earlier.saturating_add(1)).min(self.duration_frames()));
        self.normalize_source_marks()
    }

    /// The second cut at the playhead, the first at `first` (an absolute frame): the distance
    /// between them is marked, the earlier is IN, the later OUT, whatever their order.
    pub fn with_second_cut_at_confirmed(self, first: u64) -> Self {
        match self.confirmed_frame() {
            Some(frame) => self.with_second_cut(first, frame),
            None => self.normalize_source_marks(),
        }
    }

    pub fn focus_source_in(mut self) -> Self {
        self.source_mark_focus = TimelineSourceMarkFocus::In;
        self
    }

    pub fn focus_source_out(mut self) -> Self {
        self.source_mark_focus = TimelineSourceMarkFocus::Out;
        self
    }

    fn local_frame(&self, absolute_frame: u64) -> u64 {
        absolute_frame
            .saturating_sub(self.range_start_frame)
            .min(self.duration_frames())
    }

    /// The source mark taken (Ctrl+I/O, a right click): `Some(true)` IN, `Some(false)` OUT.
    pub fn taken_source_mark(&self) -> Option<bool> {
        match self.source_mark_focus {
            TimelineSourceMarkFocus::In => Some(true),
            TimelineSourceMarkFocus::Out => Some(false),
            TimelineSourceMarkFocus::Playhead => None,
        }
    }

    /// IN/OUT the source timeline paints. A confirmed player source shows the
    /// whole clip at once, the same way v5 does, until a mark replaces that end.
    pub fn visible_source_marks(&self) -> Option<(u64, u64)> {
        let duration = self.duration_frames;
        if self.playhead_frame.is_some() && duration > 0 {
            let start = self.source_in_frame.unwrap_or(0).min(duration);
            let mut end = self.source_out_frame.unwrap_or(duration).min(duration);
            if end <= start {
                end = start.saturating_add(1).min(duration);
            }
            return (end > start).then_some((start, end));
        }
        match (self.source_in_frame, self.source_out_frame) {
            (Some(start), Some(end)) if end > start => Some((start, end)),
            _ => None,
        }
    }

    /// The length the header shows for the marks `(start, end)`: while OUT is not set, from
    /// IN to the playhead, following it (user 2026-10-16: how far the playhead is from IN);
    /// with OUT set, IN to OUT.
    pub fn shown_source_length(&self, (start, end): (u64, u64)) -> u64 {
        match (self.source_out_frame, self.playhead_frame) {
            (None, Some(playhead)) => self.local_frame(playhead).saturating_sub(start),
            _ => end.saturating_sub(start),
        }
    }

    fn normalize_source_marks(mut self) -> Self {
        let duration = self.duration_frames();
        self.source_in_frame = self.source_in_frame.map(|frame| frame.min(duration));
        self.source_out_frame = self.source_out_frame.map(|frame| frame.min(duration));
        if let (Some(in_frame), Some(out_frame)) = (self.source_in_frame, self.source_out_frame) {
            if out_frame <= in_frame {
                self.source_out_frame = Some(in_frame.saturating_add(1).min(duration));
            }
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineIntent {
    None,
    CueFrame(u64),
    ToggleAudioExpand(AudioLane),
    SelectVirtual { id: String, frame: u64 },
    /// A double click on a segment of a row: it opens on the source timeline (Edit; user
    /// 2026-10-21).
    OpenVirtual { id: String, frame: u64 },
    /// A double click on the picture or A2 of a row: its slot is shown enlarged in the place of
    /// the source timeline (user 2026-10-21: "a zoom of a part of the Wrap row").
    OpenSlot { id: String, frame: u64 },
    SelectCover { id: String, frame: u64 },
    SelectMarkerSlot { id: String, frame: u64 },
    /// The source channel (zero based) chosen for A1 in the channel picker.
    ChooseA1Channel(u16),
    /// The source channel (zero based) chosen for A2 in its channel picker.
    ChooseA2Channel(u16),
    /// A right click on A1 or A2 of a source timeline: the lane is taken (as Ctrl+1, Ctrl+2).
    TakeAudioLane(AudioLane),
    /// A click on the A1 or A2 label (a button): the lane is taken (as Ctrl+1, Ctrl+2); on a
    /// row with markers the sound under the playhead in the row, else at its start (`frame`).
    LaneLabel { lane: AudioLane, frame: Option<u64> },
    /// A click on the V label: Home of the panel (start of the clip or of the program).
    GoHome,
    /// Ctrl+click on the V label of a banded row: filmstrip or narrow video (user
    /// 2026-10-15).
    ToggleVideoMode,
    /// The mouse wheel over the timeline (user 2026-10-16: the timeline behaves the same
    /// in every form): one step a frame, Ctrl held or not.
    Wheel(Wheel),
    /// A left click while something is taken (IN/OUT, A1/A2, a marker, a slot, a sound): it is
    /// kept, as Enter, and then the playhead goes to `Some` frame (user rule 2026-10-20); `None`
    /// keeps only (the end of a drag, a click on a label).
    Keep(Option<u64>),
    /// Shift + left click on a row with markers: an M marker added there (as Shift+M).
    AddMarkerAt(u64),
    /// A right click is Ctrl+ (user 2026-10-17: the right mouse button takes what it points
    /// at): IN (`true`) or OUT, whichever is nearer.
    TakeMark(bool),
    /// IN (`in_mark`) or OUT dragged with the left button to `frame` of the clip (user
    /// 2026-10-17: as in editing programs); letting the button go keeps it (`KeepMark`).
    DragMark { in_mark: bool, frame: u64 },
    /// A right click on an M marker of a row (as Ctrl+M).
    TakeMarker { id: String, frame: u64 },
    /// A right click on a cover's colour strip: its slot (as Ctrl+S).
    TakeSlot { id: String, frame: u64 },
    /// A right click on a cover outside any slot: the cover taken (for Delete).
    TakeCover { id: String, frame: u64 },
    /// Ctrl + left click on a cover's colour strip: the cover taken (for Delete).
    TakeCoverOnStrip { id: String, frame: u64 },
    /// A right click on A1 or A2 of a row with markers (a Wrap row): the sound of what lies
    /// there, at that frame (as Ctrl+1/Ctrl+2 there).
    TakeSound { lane: AudioLane, frame: u64 },
    /// A right click on a level curve (docs/98 2.1): taken as Ctrl+Shift+1/2, at the frame.
    TakeLevelLine { lane: AudioLane, frame: u64 },
    /// A second right click over A2 near the taken M marker: only its sound moves (user
    /// 2026-10-22: a right click takes the whole marker, a second one its part). The row's
    /// owner decides whether it has parts (only the enlarged slot has).
    TakeMarkerSound { id: String },
    /// A second right click over the picture near the taken M marker: only its picture moves.
    TakeMarkerPicture { id: String },
    /// A left click over A2 on a side of the taken marker's sound (user 2026-10-29): that side
    /// (the nearer to `frame`) moves alone.
    TakeCutEdge { id: String, frame: u64 },
    /// An M marker of a row dragged with the left button to `frame` of the row (a draft; the
    /// owner moves nothing until it keeps it). Returned by the row, not passed through egui
    /// memory (docs/99 step 4).
    DragMarker { id: String, frame: u64 },
    /// A side of the taken cut's sound dragged to `frame` of the row (user 2026-10-29).
    DragCutEdge { id: String, frame: u64 },
    /// A drag along the length strips of the parts that started on the strip of `id` and has
    /// reached `frame` (user 2026-11-07: several slots are chosen by dragging over their strips).
    DragStrip { id: String, frame: u64 },
    /// A cut (scissors) at the playhead of a source timeline: the middle mouse button, a
    /// touchpad's three-finger tap set to it, a two-finger tap on a touch screen (user
    /// 2026-10-18: two cuts mark IN and OUT, the earlier IN, the later OUT).
    Cut,
    /// A double click on a source timeline: a cut at the frame clicked (user 2026-10-26:
    /// "double click for IN/OUT; which is IN and which OUT the timecode decides, IN is the
    /// smaller"), as two cuts do.
    CutAt(u64),
}

impl Default for TimelineIntent {
    fn default() -> Self {
        Self::None
    }
}
mod events;
pub use events::event;

/// The scope of the keyboard catalog that holds the keys of the source timeline, the same in
/// every form (user 2026-10-17); the scope of the panel in focus is asked first.
pub const SOURCE_TIMELINE_SCOPE: &str = "source_timeline";
