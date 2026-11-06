//! What the user is told or asked (docs/99 step 2, a core piece without egui): a warning that
//! must not be missed (`Alert`, user 2026-10-21) and a question with options (`Choice`, user
//! 2026-10-23). The model only: `qnc-alert` and `qnc-choice-box` draw them.

use std::time::{Duration, Instant};

pub const MODULE_ID: &str = "qnc.module.notice";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long a warning stays on the screen.
pub const SHOWN: Duration = Duration::from_millis(3500);

/// A warning: its text and when it was raised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub text: String,
    pub raised: Instant,
}

impl Alert {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), raised: Instant::now() }
    }

    /// Whether it is still on the screen at `now`.
    pub fn shown_at(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.raised) < SHOWN
    }
}

/// What the box shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choice {
    pub title: String,
    /// What is in the way.
    pub message: String,
    /// What it is about (clip names), one per line.
    pub items: Vec<String>,
    /// How it can be solved.
    pub solution: String,
    pub options: Vec<ChoiceOption>,
}

/// One option: a catalog action and its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOption {
    pub action_id: String,
    pub label: String,
    /// The proposed one.
    pub strong: bool,
}
