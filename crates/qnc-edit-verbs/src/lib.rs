//! The one vocabulary of editing on a timeline and on its zoom (AGENTS 0.12, user 2026-11-04:
//! "a timeline and its zoom have the same tools, only the drawing differs"; docs/97).
//!
//! A verb names what the editor does to a thing on the axis (a cut, its picture, its A1 or A2
//! sound, a side of it, a piece, a lane) and turns into the catalog system action every view
//! already sends (contracts/qnc-keyboard-shortcuts.json). The timeline and the zoom build their
//! events here, so the same gesture is the same action in both. The block holds no state, no
//! rules of an owner (story, clip, script) and no drawing: an owner decides what a verb does.

use qnc_action_event::ActionEvent;

pub const MODULE_ID: &str = "qnc.module.edit-verbs";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A side of a cut: before it (the piece that ends there, its OUT) or after it (the piece that
/// starts there, its IN). On the timeline a click left or right of the cut, in the zoom the
/// picture left or right of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Before,
    After,
}

impl Side {
    pub fn after(self) -> bool {
        self == Side::After
    }
    pub fn of(after: bool) -> Self {
        if after {
            Side::After
        } else {
            Side::Before
        }
    }
}

/// A sound lane of a cut or a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    A1,
    A2,
}

/// What a view asks of its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verb {
    /// The playhead to a frame of the view's axis.
    Cue(u64),
    /// Keep what is taken (Enter, a left click while something is held).
    Keep,
    /// Give up what is taken (Esc).
    Drop,
    /// Take back the last change (Backspace).
    Undo,
    /// Write what is taken and keep it taken (the zoom's Save).
    Save,
    /// Take the picture of a cut only (V, Ctrl+I on a taken cut).
    TakePicture { cut: String },
    /// Take a sound of a cut (A1/A2 label, a right click on its lane, Ctrl+1/Ctrl+2).
    TakeSound { cut: String, lane: Lane },
    /// Take a lane (its label clicked, no cut taken): on a program's row the sound of what lies
    /// at `frame` (the playhead), on a source timeline the clip's lane.
    TakeLane { lane: Lane, frame: Option<u64> },
    /// A channel picked in a taken lane's column.
    PickChannel { lane: Lane, channel: u16 },
    /// Pick a side of a taken cut (a click left or right of it; in the zoom, a picture).
    PickSide { cut: String, side: Side },
    /// Take the J or L of a cut's sound nearest a frame (a click near its red line).
    TakeSoundSide { cut: String, frame: u64 },
    /// Move the taken J or L to a frame (a drag of its red line).
    DragSoundSide { cut: String, frame: u64 },
    /// A word said from `from` to `to` (frames of the view's axis): with something taken it goes
    /// to the word, otherwise it is a cut there.
    Word { from: u64, to: u64 },
}

/// Catalog actions of the verbs (contracts/qnc-keyboard-shortcuts.json).
pub const CUE: &str = "timeline_cue_frame";
pub const KEEP: &str = "activate_focused_item";
pub const DROP: &str = "clear_focus";
pub const UNDO: &str = "undo_object";
/// Write what is taken and keep it taken (the zoom's Save, user 2026-11-04).
pub const SAVE: &str = "zoom_save";
pub const TAKE_PICTURE: &str = "select_mark_in";
pub const TAKE_A1: &str = "select_audio_a1";
pub const TAKE_A2: &str = "select_audio_a2";
pub const PICK_SIDE: &str = "zoom_pick_side";
pub const PICK_A1_CHANNEL: &str = "source_choose_a1_channel";
pub const PICK_A2_CHANNEL: &str = "source_choose_a2_channel";
pub const TAKE_SOUND_SIDE: &str = "timeline_take_cut_edge";
pub const DRAG_SOUND_SIDE: &str = "timeline_drag_cut_edge";
pub const WORD: &str = "transcript_cut_word";
/// A left click on a timeline while something is held there (contracts/input/mouse.json).
pub const KEEP_ON_SEGMENTS: &str = "segments_keep_edit";
pub const KEEP_ON_SOURCE: &str = "source_keep_mark";

impl Verb {
    /// The system action of the verb on the panel `at` (the panel of the view's owner).
    pub fn event(&self, at: &str) -> ActionEvent {
        let event = match self {
            Verb::Cue(frame) => ActionEvent::key(CUE).with_value(*frame),
            Verb::Keep => ActionEvent::key(KEEP),
            Verb::Drop => ActionEvent::key(DROP),
            Verb::Undo => ActionEvent::key(UNDO),
            Verb::Save => ActionEvent::key(SAVE),
            Verb::TakePicture { cut } => ActionEvent::key(TAKE_PICTURE).with_target(cut.clone()),
            Verb::TakeSound { cut, lane } => ActionEvent::key(match lane {
                Lane::A1 => TAKE_A1,
                Lane::A2 => TAKE_A2,
            })
            .with_target(cut.clone()),
            Verb::TakeLane { lane, frame } => {
                let event = ActionEvent::key(match lane {
                    Lane::A1 => TAKE_A1,
                    Lane::A2 => TAKE_A2,
                });
                match frame {
                    Some(frame) => event.with_value(*frame),
                    None => event,
                }
            }
            Verb::PickChannel { lane, channel } => ActionEvent::key(match lane {
                Lane::A1 => PICK_A1_CHANNEL,
                Lane::A2 => PICK_A2_CHANNEL,
            })
            .with_value(*channel),
            Verb::PickSide { cut, side } => ActionEvent::key(PICK_SIDE).with_target(cut.clone()).with_value(i64::from(side.after())),
            Verb::TakeSoundSide { cut, frame } => ActionEvent::key(TAKE_SOUND_SIDE).with_target(cut.clone()).with_value(*frame),
            Verb::DragSoundSide { cut, frame } => ActionEvent::key(DRAG_SOUND_SIDE).with_target(cut.clone()).with_value(*frame),
            Verb::Word { from, to } => ActionEvent::key(WORD).with_value(*from).with_target(to.to_string()),
        };
        event.at(at)
    }

    /// The verb of a system action, if it is one (the way back, for an owner and for tests).
    pub fn of(event: &ActionEvent) -> Option<Self> {
        let cut = || event.target.clone();
        let frame = || event.value.and_then(|value| u64::try_from(value).ok());
        Some(match event.action_id.as_str() {
            CUE => Verb::Cue(frame()?),
            // A left click while something is held keeps it as Enter does (the timeline's own
            // ids, then it cues: the frame is where it was clicked).
            KEEP | KEEP_ON_SEGMENTS | KEEP_ON_SOURCE => Verb::Keep,
            DROP => Verb::Drop,
            UNDO => Verb::Undo,
            SAVE => Verb::Save,
            TAKE_PICTURE => Verb::TakePicture { cut: cut()? },
            TAKE_A1 | TAKE_A2 => {
                let lane = if event.action_id == TAKE_A1 { Lane::A1 } else { Lane::A2 };
                match cut() {
                    Some(cut) => Verb::TakeSound { cut, lane },
                    None => Verb::TakeLane { lane, frame: frame() },
                }
            }
            PICK_A1_CHANNEL => Verb::PickChannel { lane: Lane::A1, channel: u16::try_from(event.value?).ok()? },
            PICK_A2_CHANNEL => Verb::PickChannel { lane: Lane::A2, channel: u16::try_from(event.value?).ok()? },
            PICK_SIDE => Verb::PickSide { cut: cut()?, side: Side::of(event.value? != 0) },
            TAKE_SOUND_SIDE => Verb::TakeSoundSide { cut: cut()?, frame: frame()? },
            DRAG_SOUND_SIDE => Verb::DragSoundSide { cut: cut()?, frame: frame()? },
            WORD => Verb::Word { from: frame()?, to: cut()?.parse().ok()? },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every verb goes to its catalog action and back unchanged, on the owner's panel.
    #[test]
    fn every_verb_is_one_system_action_and_back() {
        let cut = "marker-7".to_string();
        let verbs = [
            Verb::Cue(12),
            Verb::Keep,
            Verb::Drop,
            Verb::Undo,
            Verb::Save,
            Verb::TakePicture { cut: cut.clone() },
            Verb::TakeSound { cut: cut.clone(), lane: Lane::A1 },
            Verb::TakeSound { cut: cut.clone(), lane: Lane::A2 },
            Verb::TakeLane { lane: Lane::A1, frame: Some(30) },
            Verb::TakeLane { lane: Lane::A2, frame: None },
            Verb::PickChannel { lane: Lane::A1, channel: 3 },
            Verb::PickSide { cut: cut.clone(), side: Side::Before },
            Verb::PickSide { cut: cut.clone(), side: Side::After },
            Verb::TakeSoundSide { cut: cut.clone(), frame: 40 },
            Verb::DragSoundSide { cut: cut.clone(), frame: 41 },
            Verb::Word { from: 100, to: 112 },
        ];
        for verb in verbs {
            let event = verb.event("segments");
            assert_eq!(event.at.as_deref(), Some("segments"));
            assert_eq!(Verb::of(&event), Some(verb.clone()), "{verb:?} -> {event:?}");
        }
    }

    #[test]
    fn an_action_that_is_not_a_verb_is_none() {
        assert_eq!(Verb::of(&ActionEvent::key("export_open")), None);
    }
}
