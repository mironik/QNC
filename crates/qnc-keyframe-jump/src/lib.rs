//! Up/Down on a taken edit (user 2026-10-11): to the key picture before or after it, half a
//! second when every picture is one (intra). The search runs in the background (piece
//! `qnc-keyframe-nav`); its answer moves what it was asked for: the taken source IN/OUT (the
//! playhead with it) or the taken slot or marker of the story. It owns no database or form.

use qnc_program_segments::ProgramSegments;
use qnc_source_preview::SourcePreview;
use qnc_timeline::TimelineProjection;

pub const MODULE_ID: &str = "qnc.module.keyframe-jump";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the answer of a key picture search moves.
#[derive(Debug, Clone, Copy)]
enum Jump {
    SourceMark,
    Program,
}

#[derive(Debug, Default)]
pub struct KeyframeJump {
    nav: qnc_keyframe_nav::KeyframeNav,
    jump: Option<Jump>,
}

impl KeyframeJump {
    /// The taken source mark (of the clip on the preview) to the key picture before or after it.
    pub fn source_mark(&mut self, forward: bool, preview: &SourcePreview, timeline: &TimelineProjection) -> bool {
        let (clip, frame) = (preview.view().clip_id.clone(), qnc_source_mark_focus::taken_picture(timeline));
        self.jump = Some(Jump::SourceMark);
        clip.zip(frame).is_some_and(|(clip, frame)| self.nav.request(&clip, frame, forward, preview.still_locator()))
    }

    /// The taken slot or marker of the story to the key picture before or after it; `None` when
    /// the story has nothing taken that a key picture moves.
    pub fn program(&mut self, forward: bool, preview: &SourcePreview, segments: &ProgramSegments) -> Option<bool> {
        let (clip, frame) = segments.keyframe_reference()?;
        self.jump = Some(Jump::Program);
        Some(self.nav.request(&clip, frame, forward, preview.still_locator()))
    }

    /// The answer, once it came: applied to the edit it was asked for.
    pub fn poll(&mut self, preview: &mut SourcePreview, segments: &mut ProgramSegments, timeline: &mut TimelineProjection) {
        let Some(answer) = self.nav.take() else { return };
        match (self.jump.take(), answer) {
            (Some(Jump::SourceMark), Ok(frames)) => {
                qnc_source_panel::mark_cue(preview, qnc_source_mark_focus::nudge(timeline, frames));
            }
            (Some(Jump::Program), Ok(frames)) => {
                segments.jump_edit(frames);
            }
            (_, Err(message)) => segments.say(message),
            (None, Ok(_)) => {}
        }
    }

    /// Whether a search runs (the caller repaints soon).
    pub fn busy(&self) -> bool {
        self.nav.busy()
    }
}
