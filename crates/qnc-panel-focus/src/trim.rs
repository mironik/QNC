//! Dynamic trim (`qnc-trim-play`, docs/98 1.4, user 2026-11-05): with a cut taken, the play
//! around it goes round, and K stops the playhead and brings the taken edge there.

use super::*;

impl PanelFocus {
    /// The cut the trim zoom shows (the SubSegment's first: it stands instead of the source
    /// timeline) and the playhead on the zoom's axis while it follows the player.
    fn taken_cut(&self, segments: &ProgramSegments) -> Option<(u64, Option<u64>)> {
        let script = &self.side.views().script;
        let subsegment = script.subsegment.as_ref().and_then(|view| view.zoom.as_deref()).and_then(|zoom| zoom.zoom_sound.as_ref());
        let view = segments.view();
        let sound = subsegment.or(view.zoom_sound.as_ref())?;
        Some((sound.cut, sound.playhead))
    }

    /// The play around the taken cut: back to two seconds before it, play to two after it,
    /// again and again until something else stops the player. False without a taken cut.
    pub(crate) fn trim_around(&mut self, preview: &mut SourcePreview, segments: &ProgramSegments) -> bool {
        let Some((cut, playhead)) = self.taken_cut(segments) else { return false };
        let Some((_, now, _)) = preview.playing_at() else { return false };
        let Some(interval) = preview.player_view().source_frame_interval() else { return false };
        // The zoom's axis may not be the player's (a clip edge of the S-Story): the cut is
        // where the player is, plus how far the zoom's playhead is from it.
        let cut = playhead.map_or(cut, |playhead| now.saturating_add_signed(cut as i64 - playhead as i64));
        self.shuttle.stop();
        preview.set_scrubbing(false);
        let around = qnc_trim_play::TrimLoop::around(cut, 1.0 / interval.as_secs_f64());
        preview.cue(around.from());
        self.trim_loop = Some(around);
        trim_log(format!("trim-play around cut={cut} from={} to={}", around.from(), around.to()));
        true
    }

    /// One look at the player while the play goes round the cut.
    pub(crate) fn trim_tick(&mut self, preview: &mut SourcePreview, segments: &ProgramSegments) {
        if self.trim_loop.is_none() {
            return;
        }
        // Nothing taken any more (Escape, Enter): the play goes on as a play.
        if self.taken_cut(segments).is_none() {
            self.trim_loop = None;
            return;
        }
        let (Some(around), Some((_, frame, _))) = (self.trim_loop.as_mut(), preview.playing_at()) else { return };
        match around.step(frame, preview.player_view().playing()) {
            qnc_trim_play::Step::Rewind(from) => _ = preview.cue(from),
            qnc_trim_play::Step::Play => _ = preview.toggle_play(),
            qnc_trim_play::Step::Over => self.trim_loop = None,
            qnc_trim_play::Step::Nothing => {}
        }
    }

    /// K with a cut taken: the player stops and the taken edge goes where the playhead is, by
    /// the same steps the arrows give it, so every kind of taken cut keeps its own rules.
    pub(crate) fn trim_to_stop(
        &mut self,
        pieces: (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection),
    ) -> bool {
        self.trim_loop = None;
        let Some((cut, Some(stopped))) = self.taken_cut(pieces.2) else { return false };
        let by = qnc_trim_play::moved_by(cut, stopped);
        let action = if by > 0 { "step_forward_frame" } else { "step_back_frame" };
        let mut done = false;
        for _ in 0..by.unsigned_abs().min(500) {
            done |= self.route(action, (&mut *pieces.0, &mut *pieces.1, &mut *pieces.2, &mut *pieces.3)).unwrap_or(false);
        }
        trim_log(format!("trim-play stop cut={cut} stopped={stopped} moved={by}"));
        done
    }
}

fn trim_log(line: String) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
    }
}
