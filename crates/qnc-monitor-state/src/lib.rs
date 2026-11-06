//! What a monitor shows and its Auto/On switch (docs/99 step 2, a core piece without egui): the
//! confirmed picture of the player, a message, and the one choice of this application whether
//! the monitor leaves its picture to the program output. `qnc-monitor` draws it; the preview
//! (`qnc-source-preview`) keeps it without a window.

use std::sync::Arc;

pub const MODULE_ID: &str = "qnc.module.monitor-state";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Confirmed monitor picture (RGBA), as delivered by the player.
#[derive(Debug, Clone)]
pub struct MonitorFrame {
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub width: usize,
    pub height: usize,
    pub rgba: Arc<[u8]>,
    /// Where the player writes the pictures of this session (a program output on an
    /// external screen reads the same map at its own refresh).
    pub frame_map: Option<Arc<std::path::Path>>,
}

/// What the monitor shows, the same for every form (the monitor panel's input).
#[derive(Debug, Clone)]
pub struct MonitorState {
    pub frame: Option<MonitorFrame>,
    pub video_visible: bool,
    pub message: Option<String>,
}

/// The same state: the same picture (session, generation, sequence), not its pixels.
impl PartialEq for MonitorState {
    fn eq(&self, other: &Self) -> bool {
        let picture = |state: &Self| state.frame.as_ref().map(|frame| (frame.session_id.clone(), frame.generation, frame.sequence));
        picture(self) == picture(other) && (self.video_visible, &self.message) == (other.video_visible, &other.message)
    }
}

impl Default for MonitorState {
    fn default() -> Self {
        Self { frame: None, video_visible: false, message: None }
    }
}

/// The monitor's switch (catalog actions without a key), placed by a form's layout in the
/// row under the monitor: Auto leaves the picture to the program output on an external
/// screen while it runs, On keeps drawing it here (user 2026-10-08, 2026-10-19).
pub const MONITOR_AUTO: &str = "monitor_auto";
pub const MONITOR_ON: &str = "monitor_on";

/// Auto (the default) or On: one choice for every monitor of this application, whichever
/// form shows it (user 2026-10-19: "set once, not a hundred times within one project");
/// a choice of this laptop while the application runs, not of a form, clip or project.
static AUTO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Whether Auto is chosen.
pub fn auto() -> bool {
    AUTO.load(std::sync::atomic::Ordering::Relaxed)
}

/// Chooses Auto or On by the switch action; false for any other action.
pub fn choose(action_id: &str) -> bool {
    let auto = match action_id {
        MONITOR_AUTO => true,
        MONITOR_ON => false,
        _ => return false,
    };
    AUTO.store(auto, std::sync::atomic::Ordering::Relaxed);
    true
}

/// Whether the switch button `action_id` is the one on (`auto`: Auto chosen).
pub fn switch_on(action_id: &str, auto: bool) -> bool {
    match action_id {
        MONITOR_AUTO => auto,
        MONITOR_ON => !auto,
        _ => false,
    }
}
