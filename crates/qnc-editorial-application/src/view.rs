//! Data the editorial form paints and the intents it sends back. Neutral: no
//! player, database or store type leaks through it.

use std::sync::Arc;

use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_timeline_assets::SourceTimelineAssets;

/// Confirmed monitor picture (RGBA), as delivered by the player.
#[derive(Debug, Clone)]
pub struct MonitorFrame {
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub width: usize,
    pub height: usize,
    pub rgba: Arc<[u8]>,
}

/// One row of the clip list (summary only; the full record is read on demand).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorialClip {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
}

#[derive(Clone)]
pub struct EditorialView {
    pub clips: Vec<EditorialClip>,
    pub preview_clip_id: Option<String>,
    /// True while the project catalog is being read.
    pub loading: bool,
    /// Status or controlled error text; empty when there is nothing to say.
    pub message: String,
    pub monitor_frame: Option<MonitorFrame>,
    pub video_visible: bool,
    pub monitor_message: Option<String>,
    pub timeline: TimelineProjection,
    pub assets: SourceTimelineAssets,
    pub playing: bool,
}

impl Default for EditorialView {
    fn default() -> Self {
        Self {
            clips: Vec::new(),
            preview_clip_id: None,
            loading: false,
            message: String::new(),
            monitor_frame: None,
            video_visible: false,
            monitor_message: None,
            timeline: TimelineProjection::default(),
            assets: SourceTimelineAssets::empty(),
            playing: false,
        }
    }
}

impl EditorialView {
    /// Name of the clip in the monitor, if one is chosen.
    pub fn current_clip_label(&self) -> Option<&str> {
        let id = self.preview_clip_id.as_deref()?;
        self.clips
            .iter()
            .find(|clip| clip.clip_id == id)
            .map(|clip| clip.name.as_str())
    }
}

/// Neutral intent. `Action` carries an `action_id` from
/// `contracts/qnc-keyboard-shortcuts.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorialIntent {
    PreviewClip(String),
    Action(&'static str),
    Timeline(TimelineIntent),
}

pub mod action_ids {
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STEP_BACK_FRAME: &str = "step_back_frame";
    pub const STEP_FORWARD_FRAME: &str = "step_forward_frame";
}
