//! The pieces of the right panel of an editorial board (audit 2026-10-28: the focus of every
//! form held the transcript, the shots and the script, so a form without those panels read the
//! database for them and wrote a script profile into its project). A form makes only the
//! pieces of the panels its layout has: the transcript (AI Media Assist), the shots of the
//! Video AI tab, the script (Media Assist Audio, which also puts in the statements of the
//! transcript). Their views are shared snapshots, renewed only when a poll changed them.

use std::sync::Arc;

use qnc_clip_row::ClipRow;
use qnc_clip_shots::{ClipShots, ShotsView};
use qnc_script::{Script, ScriptView};
use qnc_source_preview::SourcePreview;
use qnc_source_timecode::SourceTimecode;
use qnc_timeline::TimelineProjection;
use qnc_transcript::{Transcript, TranscriptView};

pub const MODULE_ID: &str = "qnc.module.side-pieces";

/// The kinds of right panels (a group's layout contract names them).
pub const SEGMENT_PANEL: &str = "segment_panel";
pub const TRANSCRIPT_PANEL: &str = "transcript_panel";
/// The Video AI panel: the shots of the clip on the preview.
pub const VIDEO_AI_PANEL: &str = "video_ai_panel";
/// The script of the story (Media Assist Audio, user 2026-10-28).
pub const SCRIPT_PANEL: &str = "script_panel";

/// What the right panel pieces show; empty for a piece the form does not have.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SideViews {
    pub transcript: Arc<TranscriptView>,
    pub shots: Arc<ShotsView>,
    pub script: Arc<ScriptView>,
}

/// The pieces of the panels a form has.
#[derive(Debug, Default)]
pub struct SidePieces {
    transcript: Option<Transcript>,
    shots: Option<ClipShots>,
    script: Option<Script>,
    views: SideViews,
}

impl SidePieces {
    /// The pieces of the right panel kinds `kinds` (the panel and its tabs); the script puts in
    /// the transcript's statements, so it brings the transcript with it.
    pub fn for_panels<'a>(kinds: impl IntoIterator<Item = &'a str>) -> Self {
        let kinds: Vec<&str> = kinds.into_iter().collect();
        let has = |kind: &str| kinds.contains(&kind);
        Self {
            transcript: (has(TRANSCRIPT_PANEL) || has(SCRIPT_PANEL)).then(Transcript::new),
            shots: has(VIDEO_AI_PANEL).then(ClipShots::default),
            script: has(SCRIPT_PANEL).then(Script::new),
            views: SideViews::default(),
        }
    }

    pub fn views(&self) -> &SideViews {
        &self.views
    }

    /// Polls the pieces; a view is renewed only when its piece changed it.
    pub fn poll(&mut self, preview: &mut SourcePreview, timeline: &TimelineProjection, clips: &mut [ClipRow]) {
        // The pool's ticks are the transcript's.
        if let Some(transcript) = &mut self.transcript {
            if transcript.poll(preview, timeline, clips) {
                self.views.transcript = Arc::new(transcript.view().clone());
            }
        }
        if let Some(shots) = &mut self.shots {
            if shots.poll(preview) {
                self.views.shots = Arc::new(shots.view().clone());
            }
        }
        if let Some(script) = &mut self.script {
            if script.poll(preview, timeline, clips) {
                self.views.script = Arc::new(script.view().clone());
            }
            play_pieces(script, preview);
        }
    }

    /// The pieces' own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, value: Option<i64>, target: Option<&str>, preview: &mut SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>) -> Option<bool> {
        if let Some(transcript) = &mut self.transcript {
            if let Some(done) = transcript.act(action_id, value, target, preview, timeline, timecode) {
                return Some(done);
            }
        }
        if let Some(script) = &mut self.script {
            let empty = TranscriptView::default();
            let transcript = self.transcript.as_ref().map_or(&empty, Transcript::view);
            if let Some(done) = script.act(action_id, target, (preview, timeline, timecode, transcript)) {
                play_pieces(script, preview);
                // What the action changed is seen at once (the next poll compares from here).
                self.views.script = Arc::new(script.view().clone());
                return Some(done);
            }
        }
        if let Some(shots) = &mut self.shots {
            // A click on a shot puts the playhead on its start (user 2026-10-27).
            if action_id == qnc_clip_shots::CUE_SHOT {
                let frame = target.and_then(|frame| frame.parse::<u64>().ok());
                return Some(frame.is_some_and(|frame| preview.cue(frame)));
            }
            // A click on a usable part opens it with its IN/OUT, for Add SubClip.
            if action_id == qnc_clip_shots::OPEN_PART {
                return Some(shots.open_part(target));
            }
        }
        None
    }

    /// A clip and IN/OUT a chosen statement, usable part or script block opens (once).
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        let transcript = self.transcript.as_mut().and_then(Transcript::take_source_request);
        let shots = || self.shots.as_mut().and_then(ClipShots::take_source_request);
        transcript.or_else(shots).or_else(|| self.script.as_mut().and_then(Script::take_source_request))
    }

    /// Whether a read or write of a piece runs (the form repaints soon).
    pub fn busy(&self) -> bool {
        self.transcript.as_ref().is_some_and(Transcript::busy) || self.script.as_ref().is_some_and(Script::busy)
    }

    /// Whether a transcription of this process runs in the background.
    pub fn working(&self) -> bool {
        self.transcript.as_ref().is_some_and(Transcript::working)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_gets_only_the_pieces_of_its_panels() {
        let story = SidePieces::for_panels([SEGMENT_PANEL]);
        assert!(story.transcript.is_none() && story.shots.is_none() && story.script.is_none());
        let ai = SidePieces::for_panels([TRANSCRIPT_PANEL, VIDEO_AI_PANEL]);
        assert!(ai.transcript.is_some() && ai.shots.is_some() && ai.script.is_none());
        let audio = SidePieces::for_panels([SCRIPT_PANEL]);
        assert!(audio.transcript.is_some() && audio.shots.is_none() && audio.script.is_some());
    }
}

/// The parts of the SubSegment the script opened, to the one player (user 2026-10-29: its
/// program plays in the same Broadcast Player as the story).
fn play_pieces(script: &mut Script, preview: &mut SourcePreview) {
    if let Some((pieces, frame)) = script.take_pieces_request() {
        preview.show_pieces(&pieces, frame);
    }
}
