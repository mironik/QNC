//! The moves of the playhead that are not the player's own play: the JKL shuttle (docs/98 1.1,
//! piece `qnc-shuttle`) and the play round a taken cut, dynamic trim (docs/98 1.4, piece
//! `qnc-trim-play`). It drives the one preview (Broadcast Player): it plays, pauses and cues;
//! the player keeps the clock. Which cut is taken the caller says; K with a cut taken gives the
//! caller the frames the taken edge goes, which it moves by the same steps the arrows give it.
//! It owns no database, form or story.

use qnc_source_preview::SourcePreview;

pub const MODULE_ID: &str = "qnc.module.playhead-moves";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shift+K: the play round the taken cut.
pub use qnc_trim_play::AROUND;
/// K: the shuttle stops (and a taken cut goes where the playhead stopped).
pub use qnc_shuttle::STOP;

/// The cut a trim zoom shows and, while the zoom follows the player, the playhead on the
/// zoom's axis.
pub type TakenCut = Option<(u64, Option<u64>)>;

#[derive(Debug, Default)]
pub struct PlayheadMoves {
    shuttle: qnc_shuttle::Shuttle,
    /// The frame the shuttle last cued.
    cued: Option<u64>,
    /// The play round a taken cut while it goes round.
    around: Option<qnc_trim_play::TrimLoop>,
}

impl PlayheadMoves {
    /// J, K or L: the shuttle's speed; at 1x forward the player plays, at any other speed the
    /// shuttle moves the playhead (`tick`). `None` for another action.
    pub fn key(&mut self, action_id: &str, preview: &mut SourcePreview) -> Option<bool> {
        let (_, frame, _) = preview.playing_at().unwrap_or((false, 0, false));
        let playing = preview.player_view().playing();
        let frame = preview.clip_frame().or(preview.program_frame()).unwrap_or(frame);
        let done = self.shuttle.press(action_id, std::time::Instant::now(), frame)?;
        self.cued = None;
        self.around = None;
        preview.set_scrubbing(self.shuttle.running());
        Some(match done {
            qnc_shuttle::Do::Play if !playing => preview.toggle_play(),
            qnc_shuttle::Do::Play => true,
            qnc_shuttle::Do::Pause => {
                preview.pause();
                true
            }
        })
    }

    /// Any other transport (Space, a click on a timeline) ends both moves.
    pub fn stop(&mut self, preview: &mut SourcePreview) {
        self.shuttle.stop();
        self.around = None;
        preview.set_scrubbing(false);
    }

    /// Whether either move runs (the caller repaints soon).
    pub fn running(&self) -> bool {
        self.shuttle.running() || self.around.is_some()
    }

    /// One look at the player: the shuttle's speed moves the playhead, the play round a taken
    /// cut goes round (it ends when nothing is taken any more).
    pub fn tick(&mut self, preview: &mut SourcePreview, cut: TakenCut) {
        if let Some(interval) = preview.player_view().source_frame_interval() {
            if let Some(frame) = self.shuttle.frame_at(std::time::Instant::now(), interval) {
                if self.cued != Some(frame) {
                    self.cued = Some(frame);
                    preview.cue(frame);
                }
            }
        }
        if self.around.is_none() {
            return;
        }
        // Nothing taken any more (Escape, Enter): the play goes on as a play.
        if cut.is_none() {
            self.around = None;
            return;
        }
        let (Some(around), Some((_, frame, _))) = (self.around.as_mut(), preview.playing_at()) else { return };
        match around.step(frame, preview.player_view().playing()) {
            qnc_trim_play::Step::Rewind(from) => _ = preview.cue(from),
            qnc_trim_play::Step::Play => _ = preview.toggle_play(),
            qnc_trim_play::Step::Over => self.around = None,
            qnc_trim_play::Step::Nothing => {}
        }
    }

    /// Shift+K: the play round the taken cut, back to two seconds before it, play to two after
    /// it, again and again until something else stops the player. False without a taken cut.
    pub fn around(&mut self, cut: TakenCut, preview: &mut SourcePreview) -> bool {
        let Some((cut, playhead)) = cut else { return false };
        let Some((_, now, _)) = preview.playing_at() else { return false };
        let Some(interval) = preview.player_view().source_frame_interval() else { return false };
        // The zoom's axis may not be the player's (a clip edge of the S-Story): the cut is
        // where the player is, plus how far the zoom's playhead is from it.
        let cut = playhead.map_or(cut, |playhead| now.saturating_add_signed(cut as i64 - playhead as i64));
        self.shuttle.stop();
        preview.set_scrubbing(false);
        let around = qnc_trim_play::TrimLoop::around(cut, 1.0 / interval.as_secs_f64());
        preview.cue(around.from());
        self.around = Some(around);
        log(format!("trim-play around cut={cut} from={} to={}", around.from(), around.to()));
        true
    }

    /// K: the shuttle stops; with a cut taken, the frames its edge goes to reach where the
    /// playhead stopped (the caller moves it by the arrows' steps). `0` with nothing taken.
    pub fn stopped(&mut self, cut: TakenCut, preview: &mut SourcePreview) -> i64 {
        self.key(STOP, preview);
        self.around = None;
        let Some((cut, Some(stopped))) = cut else { return 0 };
        let by = qnc_trim_play::moved_by(cut, stopped);
        log(format!("trim-play stop cut={cut} stopped={stopped} moved={by}"));
        by
    }
}

fn log(line: String) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
    }
}
