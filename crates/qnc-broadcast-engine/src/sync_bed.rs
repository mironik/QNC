//! The story's A1 under a Sync/B-roll, in the clip session (user 2026-10-20: "the story's A1
//! must be heard, so the user knows how long to hold the clip"; a second player was not
//! broadcast quality: its own clock and device stream left A1 3-4 frames off the picture).
//!
//! The A1 of the Sync window is decoded ahead (`ProgramSound` of a sound-only program) and
//! handed to the clip's own device as a bed; IN only tells the device at which sample of the
//! clip's playout bed sample 0 plays. One clock, one device stream, sample exact. It writes
//! nothing and opens no player; the window comes with the session's launch.

use std::{cell::RefCell, rc::Rc};

use qnc_audio_output::AudioOutput;
use qnc_broadcast_player::{BroadcastEngineErrorKind, SourceRuntime};

use crate::{
    DecodeInput, Result, error,
    program::{ProgramPlan, ProgramSound},
    sample_boundary,
};

/// Bed samples kept ahead at least (a frame at most rates).
const FRAME_ROOM: usize = 4_000;

pub(crate) struct SyncBed {
    sound: ProgramSound,
    device: Rc<RefCell<AudioOutput>>,
    clip: SourceRuntime,
    /// The bed frame the handed content starts at, and the next frame to hand.
    first: u64,
    next: u64,
    /// Samples of a rendered frame the device had no room for yet.
    held: Vec<f32>,
    /// The bed plays (started and not stopped).
    playing: bool,
}

impl SyncBed {
    /// The bed of `plan` (a sound-only window of the story) over the clip's device; its
    /// media are read with the clip's opener.
    pub fn open(plan: &ProgramPlan, clip_input: &DecodeInput, clip: &SourceRuntime, device: Rc<RefCell<AudioOutput>>) -> Result<Self> {
        let inputs: Vec<Rc<DecodeInput>> =
            plan.clips.iter().map(|plan| Rc::new(clip_input.for_media(plan.media.clone()))).collect();
        let sound = ProgramSound::open(plan, &inputs)?;
        let clip_rate = clip.audio_format.as_ref().map(|format| format.sample_rate_hz);
        if clip_rate != Some(sound.rate()) {
            return Err(error("the Sync bed and the clip play at different sample rates"));
        }
        device.borrow_mut().bed_reset();
        Ok(Self { sound, device, clip: clip.clone(), first: 0, next: 0, held: Vec::new(), playing: false })
    }

    /// Hands the next frames of A1 while the device has room (a few seconds ahead).
    pub fn fill(&mut self) {
        let channels = self.sound.channels().max(1);
        loop {
            if !self.held.is_empty() {
                let handed = self.device.borrow_mut().bed_push(&self.held);
                self.held.drain(..handed);
                if !self.held.is_empty() {
                    return;
                }
            }
            if self.next >= self.sound.duration_frames() || self.device.borrow().bed_room() < FRAME_ROOM {
                return;
            }
            match self.sound.render(self.next) {
                Ok(samples) => {
                    // A1 is the program's output 1.
                    self.held = samples.iter().step_by(channels).copied().collect();
                    self.next += 1;
                }
                Err(e) if e.kind == BroadcastEngineErrorKind::NotReady => return,
                Err(e) => {
                    diag(format!("sync-bed stopped frame={} why={e}", self.next));
                    self.next = self.sound.duration_frames();
                    return;
                }
            }
        }
    }

    /// Bed frame `bed_frame` plays from the sample of clip frame `source_frame` on (output 1).
    pub fn start(&mut self, source_frame: u64, bed_frame: u64) -> Result<()> {
        if bed_frame != self.first || self.playing {
            self.reset(bed_frame);
        }
        let rate = self.sound.rate();
        let position = sample_boundary(source_frame, self.clip.timebase, rate)?;
        self.device.borrow().bed_start(position, 0);
        self.playing = true;
        diag(format!("sync-bed start source_frame={source_frame} bed_frame={bed_frame} sample={position}"));
        self.fill();
        Ok(())
    }

    /// The bed stops at once; it is handed again from the window start for the next IN.
    pub fn stop(&mut self) {
        self.device.borrow().bed_stop();
        if self.playing {
            self.playing = false;
            self.reset(0);
        }
    }

    fn reset(&mut self, from: u64) {
        self.device.borrow_mut().bed_reset();
        (self.first, self.next) = (from, from);
        self.held.clear();
    }
}

fn diag(text: String) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, text);
    }
}
