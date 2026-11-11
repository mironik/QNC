//! The pieces of the right panel of an editorial board (audit 2026-10-28: the focus of every
//! form held the transcript, the shots and the script, so a form without those panels read the
//! database for them and wrote a script profile into its project). A form makes only the
//! pieces of the panels its layout has: the transcript (AI Media Assist), the shots of the
//! Video AI tab, the script (Media Assist Audio, which also puts in the statements of the
//! transcript). Their views are shared snapshots, renewed only when a poll changed them.

use std::sync::Arc;

use qnc_ai_broll::{AiBroll, AiBrollView};
use qnc_clip_row::ClipRow;
use qnc_clip_shots::{ClipShots, ShotsView};
use qnc_script::{Script, ScriptView};
use qnc_skeleton_versions::{SkeletonVersions, VersionsView};
use qnc_source_preview::SourcePreview;
use qnc_source_timecode::SourceTimecode;
use qnc_timeline_model::TimelineProjection;
use qnc_transcript::{ClipTranscripts, Transcript, TranscriptView};

pub const MODULE_ID: &str = "qnc.module.side-pieces";

/// The kinds of right panels (a group's layout contract names them).
pub const SEGMENT_PANEL: &str = "segment_panel";
/// The segment panel of the S-Story (user 2026-10-31: "like the Story, without the lead-in"):
/// the same panel, its story the open version (a copy of the Story of its own).
pub const DRAFT_SEGMENT_PANEL: &str = "draft_segment_panel";
pub const TRANSCRIPT_PANEL: &str = "transcript_panel";
/// The transcript to read and search, without the statements (MA Script media overlay).
pub const TRANSCRIPT_SEARCH_PANEL: &str = "transcript_search_panel";
/// The Video AI panel: the shots of the clip on the preview.
pub const VIDEO_AI_PANEL: &str = "video_ai_panel";
/// The script of the story (Media Assist Audio, user 2026-10-28).
pub const SCRIPT_PANEL: &str = "script_panel";
/// Writing the story over the whole form (MA Script, user 2026-11-01): the same script.
pub const SCRIPT_WRITER_PANEL: &str = "script_writer_panel";
/// The bar in the media window of the story writer (user 2026-11-02): the marked IN-OUT goes into
/// the story from where it is marked. It shows the script of the writer panel.
pub const SCRIPT_CLIP_BAR: &str = "script_clip_bar";
/// Not a panel: the form has the pool tab AI B-roll (user 2026-11-10), its piece is made.
pub const AI_BROLL_TAB: &str = "ai_broll_tab";

/// What the right panel pieces show; empty for a piece the form does not have.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SideViews {
    pub transcript: Arc<TranscriptView>,
    pub shots: Arc<ShotsView>,
    pub script: Arc<ScriptView>,
    pub versions: Arc<VersionsView>,
    pub ai_broll: Arc<AiBrollView>,
}

/// The pieces of the panels a form has.
#[derive(Debug, Default)]
pub struct SidePieces {
    /// The one reader of the transcripts the pieces show (the transcript panel's clip, the
    /// script's statements): read from the database, given to both.
    transcripts: ClipTranscripts,
    transcript: Option<Transcript>,
    shots: Option<ClipShots>,
    script: Option<Script>,
    versions: Option<SkeletonVersions>,
    ai_broll: Option<AiBroll>,
    views: SideViews,
    /// The form's segment panel shows a version of the S-Story.
    draft_story: bool,
    /// The messages of the script and the transcript at the last look.
    said: [String; 2],
}

impl SidePieces {
    /// The pieces of the right panel kinds `kinds` (the panel and its tabs); the script puts in
    /// the transcript's statements, so it brings the transcript with it.
    pub fn for_panels<'a>(kinds: impl IntoIterator<Item = &'a str>) -> Self {
        let kinds: Vec<&str> = kinds.into_iter().collect();
        let has = |kind: &str| kinds.contains(&kind);
        Self {
            transcripts: ClipTranscripts::new(),
            transcript: (has(TRANSCRIPT_PANEL) || has(TRANSCRIPT_SEARCH_PANEL) || has(SCRIPT_PANEL) || has(SCRIPT_WRITER_PANEL)).then(Transcript::new),
            shots: has(VIDEO_AI_PANEL).then(ClipShots::default),
            script: (has(SCRIPT_PANEL) || has(SCRIPT_WRITER_PANEL)).then(Script::new),
            versions: has(DRAFT_SEGMENT_PANEL).then(SkeletonVersions::new),
            ai_broll: has(AI_BROLL_TAB).then(AiBroll::new),
            views: SideViews::default(),
            draft_story: has(DRAFT_SEGMENT_PANEL),
            said: Default::default(),
        }
    }

    /// The story of the form's segment panel in the project database `target`: the Story, or
    /// the open S-Story version when the form's panel is the S-Story's.
    pub fn story_of(&self, target: qnc_db_broker::ProjectDbTarget) -> qnc_db_broker::ProjectDbTarget {
        match self.draft_story() {
            Some(version) => target.draft_story(version),
            None => target,
        }
    }

    /// The S-Story version the form shows; `None` for a form of the Story or none.
    pub fn draft_story(&self) -> Option<u32> {
        self.draft_story.then(|| self.versions.as_ref().map_or(1, SkeletonVersions::open_number))
    }

    pub fn views(&self) -> &SideViews {
        &self.views
    }

    /// AI of the SubSegment open in place of the source timeline (user 2026-11-07); `None`
    /// while none is open (the AI of the story then).
    pub fn subsegment_assist(&self) -> Option<bool> {
        self.script.as_ref()?.subsegment_assist()
    }

    /// Polls the pieces; a view is renewed only when its piece changed it.
    pub fn poll(&mut self, preview: &mut SourcePreview, timeline: &TimelineProjection, clips: &mut [ClipRow]) {
        // The transcripts the pieces show, read once for all of them.
        if self.transcript.is_some() || self.script.is_some() {
            // No clip on the preview: the form was left or opened again; what a repass wrote is
            // read now (user 2026-11-08: the truth is in the database, shown then).
            if preview.view().clip_id.is_none() {
                self.transcripts.let_go();
            }
            let shown = self.transcript.as_ref().and(preview.view().clip_id.clone());
            let statements = self.script.as_ref().map(Script::transcript_clips).unwrap_or_default();
            self.transcripts.poll(preview.project_target(), shown.into_iter().chain(statements));
        }
        // The pool's ticks are the transcript's.
        if let Some(transcript) = &mut self.transcript {
            if transcript.poll(preview, timeline, clips, &mut self.transcripts) {
                self.views.transcript = Arc::new(transcript.view().clone());
            }
        }
        if let Some(shots) = &mut self.shots {
            if shots.poll(preview) {
                self.views.shots = Arc::new(shots.view().clone());
            }
        }
        if let Some(script) = &mut self.script {
            if script.poll(preview, timeline, clips, &self.transcripts) {
                self.views.script = Arc::new(script.view().clone());
            }
            play_pieces(script, preview);
        }
        if let Some(versions) = &mut self.versions {
            if versions.poll(preview) {
                self.views.versions = Arc::new(versions.view().clone());
            }
        }
        if let Some(ai_broll) = &mut self.ai_broll {
            if ai_broll.poll(preview) {
                self.views.ai_broll = Arc::new(ai_broll.view().clone());
            }
        }
        self.warn_of_refusals();
    }

    /// A message newly saying that a command could not be done is raised as a warning.
    fn warn_of_refusals(&mut self) {
        let script = self.script.as_ref().map_or("", |script| script.view().message.as_str());
        let transcript = self.transcript.as_ref().map_or("", |transcript| transcript.view().message.as_str());
        for (said, now) in self.said.iter_mut().zip([script, transcript]) {
            if said != now {
                if qnc_notice::is_refusal(now) {
                    qnc_notice::raise(now);
                }
                *said = now.to_string();
            }
        }
    }

    /// The pieces' own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, value: Option<i64>, target: Option<&str>, preview: &mut SourcePreview, timeline: &TimelineProjection, timecode: Option<&SourceTimecode>) -> Option<bool> {
        if let Some(transcript) = &mut self.transcript {
            if let Some(done) = transcript.act(action_id, value, target, preview, timeline, timecode) {
                // A statement sent to the repass (user 2026-11-08): it is transcribed again in the
                // background and written to the database only; this form keeps what it shows.
                if action_id == qnc_transcript::REPASS && done {
                    if let Some(clip) = preview.view().clip_id.clone() {
                        self.transcripts.hold(&clip);
                    }
                }
                // What the action changed (the search) is seen at once.
                self.views.transcript = Arc::new(transcript.view().clone());
                return Some(done);
            }
        }
        if let Some(script) = &mut self.script {
            let empty = TranscriptView::default();
            let transcript = self.transcript.as_ref().map_or(&empty, Transcript::view);
            script.set_value(value);
            if let Some(done) = script.act(action_id, target, (preview, timeline, timecode, transcript)) {
                play_pieces(script, preview);
                // What the action changed is seen at once (the next poll compares from here).
                self.views.script = Arc::new(script.view().clone());
                return Some(done);
            }
        }
        if let Some(versions) = &mut self.versions {
            if let Some(done) = versions.act(action_id, target) {
                self.views.versions = Arc::new(versions.view().clone());
                return Some(done);
            }
        }
        if let Some(ai_broll) = &mut self.ai_broll {
            if let Some(done) = ai_broll.act(action_id, target) {
                self.views.ai_broll = Arc::new(ai_broll.view().clone());
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

    /// The S-Story version newly opened (its number), once: the story machinery turns to it.
    pub fn take_version_switch(&mut self) -> Option<u32> {
        self.versions.as_mut().and_then(SkeletonVersions::take_switch)
    }

    /// A clip and IN/OUT a chosen statement, usable part or script block opens (once).
    /// The take of a voice record to hear (its clip and channel), once.
    pub fn take_voice_request(&mut self) -> Option<(String, u16, bool, u64)> {
        self.script.as_mut().and_then(Script::take_voice_request)
    }

    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        let transcript = self.transcript.as_mut().and_then(Transcript::take_source_request);
        let shots = || self.shots.as_mut().and_then(ClipShots::take_source_request);
        transcript
            .or_else(shots)
            .or_else(|| self.script.as_mut().and_then(Script::take_source_request))
            .or_else(|| self.ai_broll.as_mut().and_then(AiBroll::take_source_request))
    }

    /// Whether a read or write of a piece runs (the form repaints soon).
    /// The slot chosen in the story orders the AI B-roll by the words A1 says there (user
    /// 2026-11-10); the ask is made only where the form has that tab.
    pub fn follow_slot(&mut self, ask: impl FnOnce() -> Option<qnc_ai_broll::Ask>, preview: &SourcePreview) {
        if let Some(ai_broll) = &mut self.ai_broll {
            // The text of the script asks first where the form has one (user 2026-11-11: the
            // text editor gives the query), else the story's chosen slot.
            let written = self.script.as_ref().and_then(|script| script.view().broll_query()).and_then(|(key, slots)| qnc_ai_broll::ask_of_text(key, slots));
            if ai_broll.follow_slot(written.or_else(ask), preview) {
                self.views.ai_broll = Arc::new(ai_broll.view().clone());
            }
        }
    }

    pub fn busy(&self) -> bool {
        self.transcripts.busy() || self.transcript.as_ref().is_some_and(Transcript::busy) || self.script.as_ref().is_some_and(Script::busy) || self.versions.as_ref().is_some_and(SkeletonVersions::busy) || self.ai_broll.as_ref().is_some_and(AiBroll::busy)
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
        assert!(audio.transcript.is_some() && audio.shots.is_none() && audio.script.is_some() && audio.versions.is_none());
        assert_eq!(audio.draft_story(), None);
        let script = SidePieces::for_panels([SCRIPT_WRITER_PANEL]);
        assert!(script.transcript.is_some() && script.script.is_some() && script.versions.is_none());
        let s_story = SidePieces::for_panels([DRAFT_SEGMENT_PANEL]);
        assert!(s_story.versions.is_some() && s_story.script.is_none());
        assert_eq!(s_story.draft_story(), Some(1), "version 1 until the list of versions is read");
    }
}

/// The parts of the SubSegment the script opened, to the one player (user 2026-10-29: its
/// program plays in the same Broadcast Player as the story).
fn play_pieces(script: &mut Script, preview: &mut SourcePreview) {
    if let Some((pieces, frame)) = script.take_pieces_request() {
        preview.show_pieces(&pieces, frame);
    }
}
