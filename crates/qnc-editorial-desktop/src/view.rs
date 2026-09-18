//! Data the form paints and the intents it sends back. Neutral: the form has no
//! application, database or player; whoever hosts it fills the view.

use std::sync::Arc;

use qnc_filmstrip::FilmstripBackground;
use qnc_timeline::{TimelineIntent, TimelineProjection};

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

/// Poster shown in the monitor until the first real frame arrives.
#[derive(Debug, Clone)]
pub struct Poster {
    pub uri: String,
    pub content_key: u64,
    pub size: [usize; 2],
    pub pixels: Arc<[u8]>,
}

#[derive(Clone)]
pub struct EditorialView {
    pub monitor_frame: Option<MonitorFrame>,
    pub video_visible: bool,
    pub monitor_message: Option<String>,
    pub poster: Option<Poster>,
    /// Label of the current clip; `None` shows the contract placeholder.
    pub clip_label: Option<String>,
    pub timeline: TimelineProjection,
    pub filmstrip: Option<FilmstripBackground>,
    pub a1_peaks: Vec<f32>,
    pub a2_peaks: Vec<f32>,
    pub a3_peaks: Vec<f32>,
    pub a4_peaks: Vec<f32>,
}

impl Default for EditorialView {
    fn default() -> Self {
        Self {
            monitor_frame: None,
            video_visible: false,
            monitor_message: None,
            poster: None,
            clip_label: None,
            timeline: TimelineProjection::default(),
            filmstrip: None,
            a1_peaks: Vec::new(),
            a2_peaks: Vec::new(),
            a3_peaks: Vec::new(),
            a4_peaks: Vec::new(),
        }
    }
}

/// Neutral intent. `Action` carries an `action_id` from
/// `contracts/qnc-keyboard-shortcuts.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorialIntent {
    Action(&'static str),
    Timeline(TimelineIntent),
}

pub mod action_ids {
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STEP_BACK_FRAME: &str = "step_back_frame";
    pub const STEP_FORWARD_FRAME: &str = "step_forward_frame";
}
