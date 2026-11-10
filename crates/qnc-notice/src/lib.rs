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

/// The warning a command raised, for the host of the form to draw: one per process, shown for
/// `SHOWN` (user 2026-11-09: a command that could not be done is told over the screen).
static RAISED: std::sync::Mutex<Option<Alert>> = std::sync::Mutex::new(None);

/// Raises the warning.
pub fn raise(text: impl Into<String>) {
    if let Ok(mut raised) = RAISED.lock() {
        *raised = Some(Alert::new(text));
    }
}

/// Raises the warning and gives its text back (for the status line).
pub fn raised(text: String) -> String {
    raise(text.clone());
    text
}

/// The warning raised last.
pub fn current() -> Option<Alert> {
    RAISED.lock().ok().and_then(|raised| raised.clone())
}

/// The messages of the side panels that say a command could not be done.
const REFUSALS: &[&str] = &[
    "script.write_first",
    "script.choose_text",
    "script.sot_marks",
    "script.no_project",
    "script.choose_to_record",
    "script.no_clip_kind",
    "script.nothing_to_undo",
    "script.nothing_to_redo",
    "script.draft_empty",
    "script.voice_local_only",
    "db.script_last_piece",
    "subsegment.slide_locked",
    "subsegment.no_join_sound",
    "subsegment.mark_in_pause",
    "story.msg.sound_assist_keep_off",
    "story.msg.sound_assist_keep_none",
    "transcript.quote_marks",
    "transcript.quote_empty",
    "transcribe.running",
    "transcribe.choose_clip",
    "transcribe.empty_range",
    "transcribe.off_in_project",
    "transcribe.repass_marks",
    "transcribe.repass_croatian",
    "transcribe.no_channel",
];

/// Whether a message of a side panel says a command could not be done.
pub fn is_refusal(message: &str) -> bool {
    !message.is_empty() && REFUSALS.iter().copied().any(|code| qnc_ui_text::text(code) == message)
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
    /// What the action is about (a group's code, an id), carried with it.
    pub target: Option<String>,
}

impl ChoiceOption {
    /// An option of `action_id` with `label`.
    pub fn new(action_id: impl Into<String>, label: impl Into<String>, strong: bool) -> Self {
        Self { action_id: action_id.into(), label: label.into(), strong, target: None }
    }

    /// The option with the target its action carries.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
}

/// The question a piece asks over the form, for the host of the form to draw (user 2026-11-10:
/// another group for an AI B-roll shot): one per process, until it is answered or withdrawn.
static ASKED: std::sync::Mutex<Option<Choice>> = std::sync::Mutex::new(None);

/// Asks `choice` over the form.
pub fn ask(choice: Choice) {
    if let Ok(mut asked) = ASKED.lock() {
        *asked = Some(choice);
    }
}

/// Withdraws the question asked.
pub fn withdraw() {
    if let Ok(mut asked) = ASKED.lock() {
        *asked = None;
    }
}

/// The question asked over the form.
pub fn asked() -> Option<Choice> {
    ASKED.lock().ok().and_then(|asked| asked.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_is_asked_until_it_is_withdrawn() {
        let choice = Choice { title: "t".into(), options: vec![ChoiceOption::new("a", "A", true).with_target("x")], ..Choice::default() };
        ask(choice.clone());
        assert_eq!(asked(), Some(choice));
        withdraw();
        assert_eq!(asked(), None);
    }
}
