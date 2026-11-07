//! The sound assistant on the source timeline (user 2026-11-07: "the AI sound assistant must be
//! universal, whatever the form"; "I do not see what the AI does"): for the shown clip it evens
//! out the speech on A1 within the clip (`qnc_sound_assist::level_speech`) and lowers A2 under
//! that speech when A2 is another channel (the story's ducking, `qnc_sound_assist::propose`),
//! from the loudness measured with the clip's wave. While it is on, a lane without a curve of
//! the user's plays, draws and shows the proposal as its curve; KEEP (Ctrl+Shift+N) keeps it as
//! the clip's own curves. The status line says what it did.

use qnc_program_waveform::Placement;
use qnc_source_preview::SourcePreview;
use qnc_timeline_model::TimelineProjection;

use crate::SourceTimeline;

/// What a proposal was made of: the clip, its span, the lanes' channels, levels and offs, the
/// waves read, and which lanes have a curve of the user's.
pub(crate) type AssistKey = (String, (u64, u64), [(u16, i8, bool); 2], u64, [bool; 2]);

impl SourceTimeline {
    /// AI on or off for the shown clip (`None` with no clip).
    pub fn sound_assist_on(&self) -> Option<bool> {
        self.clip.as_ref().map(|_| self.assist)
    }

    /// AI (Shift+N) and KEEP (Ctrl+Shift+N) on the source timeline; `None` for any other key.
    pub fn assist_key(&mut self, action_id: &str, preview: &mut SourcePreview) -> Option<bool> {
        match action_id {
            "toggle_sound_assist" => {
                self.assist = !self.assist;
                self.assisted = None;
                self.message = qnc_ui_text::text(if self.assist { "source.msg.assist_on" } else { "story.msg.sound_assist_off" }).into();
                self.hear_curves(preview);
                Some(true)
            }
            "sound_assist_keep" => {
                if !self.assist {
                    self.message = qnc_ui_text::text("story.msg.sound_assist_keep_off").into();
                    return Some(false);
                }
                let proposal = self.assisted.as_ref().map(|(_, curves)| curves.clone()).unwrap_or_default();
                let mut kept = 0;
                for (lane, curve) in proposal.into_iter().enumerate() {
                    if self.curves[lane].is_empty() && !curve.is_empty() {
                        self.curves[lane] = curve;
                        kept += 1;
                    }
                }
                if kept == 0 {
                    self.message = qnc_ui_text::text("story.msg.sound_assist_keep_none").into();
                    return Some(false);
                }
                self.assisted = None;
                self.keep_sound(preview);
                self.hear_curves(preview);
                self.message = qnc_ui_text::fill("story.msg.sound_assist_kept", &[("count", &kept.to_string())]).into();
                Some(true)
            }
            _ => None,
        }
    }

    /// Once a frame while AI is on: the clip's wave (with its loudness) is read and the proposal
    /// made again when anything it was made of changed.
    pub(crate) fn poll_assist(&mut self, preview: &SourcePreview, timeline: &TimelineProjection) {
        let Some(clip_id) = self.clip.clone().filter(|_| self.assist) else { return };
        if self.assist_clip.as_deref() != Some(clip_id.as_str()) {
            self.waves.0.set_reader(preview.artifact_reader());
            self.assist_clip = Some(clip_id.clone());
        }
        let (start, duration) = (timeline.range_start(), timeline.duration_frames().max(1));
        let end = start + duration;
        let lanes = self.kept_lanes();
        let placement = |lane: usize| Placement {
            clip_id: clip_id.clone(),
            channel: lanes[lane].channel,
            program_start: start,
            program_end: end,
            source_in: start,
            source_out: end,
            level_db: lanes[lane].level_db,
            off: lanes[lane].off,
            curve: Vec::new(),
        };
        let (a1, a2) = (placement(0), placement(1));
        let _ = self.waves.0.peaks(end, std::slice::from_ref(&a1), std::slice::from_ref(&a2), |_| Some(end));
        let own = [!self.curves[0].is_empty(), !self.curves[1].is_empty()];
        let lane_key = |lane: &qnc_lane_choice::Lane| (lane.channel, lane.level_db, lane.off);
        let key: AssistKey = (clip_id.clone(), (start, end), [lane_key(&lanes[0]), lane_key(&lanes[1])], self.waves.0.revision(), own);
        if self.assisted.as_ref().is_some_and(|(made, _)| *made == key) {
            return;
        }
        let Some(fps) = self.view.timecode.as_ref().map(|timecode| timecode.fps).filter(|fps| *fps > 0) else { return };
        let waves = &self.waves.0;
        let measure = |id: &str| waves.clip_loudness(id).map(|(channels, duration_frames)| qnc_program_loudness::ClipMeasure { channels, duration_frames });
        if waves.clip_loudness(&clip_id).is_none() {
            return;
        }
        let speech = if own[0] { Vec::new() } else { qnc_sound_assist::level_speech(end, (fps, 1), &a1, measure) };
        // A2 under the speech when it is another channel of the clip than A1.
        let duck = if own[1] || a2.off || a2.channel == a1.channel {
            Vec::new()
        } else {
            let items = [qnc_sound_assist::Item { lane: 0, at: a1.clone(), own_curve: true }, qnc_sound_assist::Item { lane: 1, at: a2.clone(), own_curve: false }];
            qnc_sound_assist::propose(end, (fps, 1), &items, measure).into_iter().find(|proposal| proposal.index == 1).map(|proposal| proposal.points).unwrap_or_default()
        };
        let stretches = speech.iter().filter(|(_, db10)| *db10 != 0).count() / 2;
        let ducks = duck.iter().filter(|(_, db10)| *db10 != 0).count() / 2;
        self.message = if stretches == 0 && ducks == 0 {
            qnc_ui_text::text("source.msg.assist_nothing").into()
        } else {
            qnc_ui_text::fill("source.msg.assist_made", &[("a1", &stretches.to_string()), ("a2", &ducks.to_string())]).into()
        };
        self.assisted = Some((key, [speech, duck]));
    }

    /// The assistant's curve of `lane` while it is on and the lane has none of the user's.
    pub(crate) fn assist_curve(&self, lane: usize) -> Option<&Vec<(u64, i16)>> {
        let (_, curves) = self.assisted.as_ref().filter(|_| self.assist && self.curves[lane].is_empty())?;
        curves.get(lane).filter(|curve| !curve.is_empty())
    }
}
