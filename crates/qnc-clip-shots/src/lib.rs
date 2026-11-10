//! The shots of the clip on the preview (user 2026-10-27: Video AI in AI Media Assist): the
//! analysis the background worker kept in the project database, read outside the UI thread
//! and read again every few seconds while the clip is shown (piece `qnc-clip-follow`), so the
//! shots show when they are made or made again. A click on a shot puts the playhead on its
//! start. It analyses nothing itself.

use std::{sync::Arc, time::Duration};

use qnc_shot_db::ClipShots as Analysis;
use qnc_source_preview::SourcePreview;

pub const MODULE_ID: &str = "qnc.module.clip-shots";
/// The panel the shot actions come from, and its action: the playhead on a shot (target: frame).
pub const PANEL: &str = "video_ai";
pub const CUE_SHOT: &str = "shot_cue";
/// A usable part opens on the source timeline with its IN/OUT (target: the part's index), so
/// Add SubClip (Shift+S) keeps it as a SubClip.
pub const OPEN_PART: &str = "shot_open_part";

/// The shown clip's analysis is read again after this.
const ASK_AGAIN: Duration = Duration::from_secs(3);

/// What the Video AI panel shows of the clip on the preview.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShotsView {
    pub clip_id: Option<String>,
    /// The clip's analysis, once read; `None` while it has none (or it is read).
    pub analysis: Option<Arc<Analysis>>,
    pub playhead: Option<u64>,
    /// The clip is a statement (user 2026-11-10): kept whole, its parts are not cut by its
    /// pictures, so none are shown.
    pub statement: bool,
    /// Why the analysis could not be read.
    pub message: String,
}

impl ShotsView {
    /// The shot the playhead is in.
    pub fn shot_at_playhead(&self) -> Option<usize> {
        let (analysis, frame) = (self.analysis.as_ref()?, self.playhead?);
        analysis.shots.iter().position(|shot| shot.from <= frame && frame < shot.to)
    }

    /// The usable part the playhead is in.
    pub fn part_at_playhead(&self) -> Option<usize> {
        let (analysis, frame) = (self.analysis.as_ref()?, self.playhead?);
        analysis.parts.iter().position(|part| part.from <= frame && frame < part.to)
    }
}

#[derive(Debug)]
pub struct ClipShots {
    view: ShotsView,
    /// The shown clip's analysis as the database holds it, read again in a rhythm.
    follow: qnc_clip_follow::ClipFollow<Option<(Arc<Analysis>, bool)>>,
    /// A clip and IN/OUT a chosen part opens on the source timeline.
    source_request: Option<(String, u64, u64)>,
}

impl Default for ClipShots {
    fn default() -> Self {
        Self { view: ShotsView::default(), follow: qnc_clip_follow::ClipFollow::new("clip-shots", ASK_AGAIN), source_request: None }
    }
}

impl ClipShots {
    pub fn view(&self) -> &ShotsView {
        &self.view
    }

    /// The part `target` (its index) of the shown clip opens with its IN/OUT; false when there
    /// is no such part.
    pub fn open_part(&mut self, target: Option<&str>) -> bool {
        let part = target.and_then(|index| index.parse::<usize>().ok()).and_then(|index| self.view.analysis.as_ref()?.parts.get(index).cloned());
        let (Some(part), Some(clip)) = (part, self.view.clip_id.clone()) else { return false };
        self.source_request = Some((clip, part.from, part.to));
        true
    }

    /// A clip and IN/OUT a chosen part opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }

    /// Follows the clip on the preview; true when the view changed.
    pub fn poll(&mut self, preview: &SourcePreview) -> bool {
        let before = self.view.clone();
        let project = preview.project_target().cloned();
        let clip_id = preview.view().clip_id.clone();
        self.view.playhead = preview.view().timeline.playhead_frame;
        if self.view.clip_id != clip_id {
            self.view.analysis = None;
            self.view.message.clear();
            self.view.clip_id = clip_id.clone();
        }
        if let (Some(project), Some(clip_id)) = (project.as_ref(), clip_id.as_deref()) {
            let analysis = self.follow.get(project.uri(), clip_id, true, || {
                let (target, clip) = (project.clone(), clip_id.to_string());
                move || {
                    // A statement the look kept whole (`statement` shots) shows no parts.
                    let statement = qnc_shot_content_db::read_clip(&target, &clip).ok().flatten().is_some_and(|content| content.parts.iter().any(|part| part.kind == "statement"));
                    qnc_shot_db::read_clip(&target, &clip).map(|analysis| analysis.map(|mut analysis| {
                        if statement {
                            analysis.parts.clear();
                        }
                        (Arc::new(analysis), statement)
                    }))
                }
            });
            let analysis = analysis.flatten();
            self.view.statement = analysis.as_ref().is_some_and(|(_, statement)| *statement);
            self.view.analysis = analysis.map(|(analysis, _)| analysis);
            self.view.message = self.follow.error(project.uri(), clip_id).unwrap_or_default().to_string();
        }
        self.view != before
    }
}
