//! The shots of the clip on the preview (user 2026-10-27: Video AI in AI Media Assist): the
//! analysis the background worker kept in the project database, read outside the UI thread
//! and read again now and then while the clip has none yet, so the shots show when they are
//! made. A click on a shot puts the playhead on its start. It analyses nothing itself.

use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use qnc_db_broker::ProjectDbTarget;
use qnc_shot_db::ClipShots as Analysis;
use qnc_source_preview::SourcePreview;

pub const MODULE_ID: &str = "qnc.module.clip-shots";
/// The panel the shot actions come from, and its action: the playhead on a shot (target: frame).
pub const PANEL: &str = "video_ai";
pub const CUE_SHOT: &str = "shot_cue";
/// A usable part opens on the source timeline with its IN/OUT (target: the part's index), so
/// Add SubClip (Shift+S) keeps it as a SubClip.
pub const OPEN_PART: &str = "shot_open_part";

/// A clip without an analysis is asked again after this.
const ASK_AGAIN: Duration = Duration::from_secs(3);

/// What the Video AI panel shows of the clip on the preview.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShotsView {
    pub clip_id: Option<String>,
    /// The clip's analysis, once read; `None` while it has none (or it is read).
    pub analysis: Option<Arc<Analysis>>,
    pub playhead: Option<u64>,
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

type Read = (String, String, mpsc::Receiver<Result<Option<Analysis>, String>>);

#[derive(Debug, Default)]
pub struct ClipShots {
    view: ShotsView,
    /// The project and clip of the shown analysis, and when it was read.
    read: Option<(String, String, Instant)>,
    reading: Option<Read>,
    /// A clip and IN/OUT a chosen part opens on the source timeline.
    source_request: Option<(String, u64, u64)>,
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
            self.take_read(project, clip_id);
            self.ask(project, clip_id);
        }
        self.view != before
    }

    fn take_read(&mut self, project: &ProjectDbTarget, clip_id: &str) {
        let Some((uri, clip, done)) = &self.reading else { return };
        match done.try_recv() {
            Ok(answer) => {
                if uri == project.uri() && clip == clip_id {
                    match answer {
                        Ok(analysis) => {
                            self.view.analysis = analysis.map(Arc::new);
                            self.view.message.clear();
                        }
                        Err(error) => self.view.message = error,
                    }
                }
                self.read = Some((uri.clone(), clip.clone(), Instant::now()));
                self.reading = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => self.reading = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    /// Reads the clip's analysis when it was not read, or again a while after it had none.
    fn ask(&mut self, project: &ProjectDbTarget, clip_id: &str) {
        if self.reading.is_some() {
            return;
        }
        let fresh = self.read.as_ref().is_some_and(|(uri, clip, at)| {
            uri == project.uri() && clip == clip_id && (self.view.analysis.is_some() || at.elapsed() < ASK_AGAIN)
        });
        if fresh {
            return;
        }
        let (send, done) = mpsc::channel();
        let (target, clip) = (project.clone(), clip_id.to_string());
        if std::thread::Builder::new().name("qnc-clip-shots-read".into()).spawn(move || _ = send.send(qnc_shot_db::read_clip(&target, &clip))).is_ok() {
            self.reading = Some((project.uri().to_string(), clip_id.to_string(), done));
        }
    }
}
