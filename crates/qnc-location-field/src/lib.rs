//! A location field and its browser (audit 2026-10-27: the Project form carried the same
//! code twice, for the folder of the projects and of the exports). The field holds what it
//! shows, whether its browser is open, the source tab and the last listing; an answer of
//! the browser becomes the step it asks of the background browser (`qnc-dir-browse-jobs`),
//! which the owner sends and whose listing it hands back. It knows no form and writes no
//! setting: the owner decides what a confirmed folder means.

use qnc_dir_browse_jobs::BrowseStep;
use qnc_dir_browser::BrowserState;
use qnc_dir_browser::{BrowserAction, BrowserAnswer};

pub use qnc_dir_browser::BrowserAnswer as Answer;

/// Where the browser looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceKind {
    #[default]
    Local,
    Lan,
    Internet,
}

/// The source tabs in order.
pub const KINDS: [SourceKind; 3] = [SourceKind::Local, SourceKind::Lan, SourceKind::Internet];

impl SourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => qnc_ui_text::text("source.computer"),
            Self::Lan => "LAN",
            Self::Internet => "Internet",
        }
    }
}

/// What an answer of the field asks of its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldStep {
    Nothing,
    /// Send this step to the background browser (the field is busy until its listing).
    Browse(BrowseStep),
    /// The folder of this URI was confirmed.
    Confirm(String),
}

#[derive(Debug, Clone)]
pub struct LocationField {
    /// The path the field shows.
    pub text: String,
    /// The path was typed or chosen since the settings were read.
    pub dirty: bool,
    pub open: bool,
    pub kind: SourceKind,
    pub browser: BrowserState,
    pub error: Option<String>,
    pub busy: bool,
}

impl LocationField {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            dirty: false,
            open: false,
            kind: SourceKind::Local,
            browser: BrowserState { roots: true, ..BrowserState::default() },
            error: None,
            busy: false,
        }
    }

    /// Opens or closes the browser; only one of the fields of a form is open. Opened on
    /// the computer, it starts at the folder the field shows (or the disks).
    pub fn toggle(&mut self, other: &mut LocationField) -> FieldStep {
        let opened = toggle_exclusive_panel(&mut self.open, &mut other.open);
        if !opened || self.kind != SourceKind::Local {
            return FieldStep::Nothing;
        }
        let start = std::path::PathBuf::from(self.text.trim());
        self.browse(if start.is_absolute() { BrowseStep::Path(start) } else { BrowseStep::Roots })
    }

    /// The step an answer of the browser asks for.
    pub fn answer(&mut self, answer: &BrowserAnswer) -> FieldStep {
        let local = self.kind == SourceKind::Local;
        match answer {
            BrowserAnswer::Browser(BrowserAction::Kind(index)) => {
                let Some(kind) = KINDS.get(*index).copied() else { return FieldStep::Nothing };
                self.kind = kind;
                let unread = self.browser.entries.is_empty() && self.browser.path_label.trim().is_empty();
                if kind == SourceKind::Local && unread {
                    return self.browse(BrowseStep::Roots);
                }
                FieldStep::Nothing
            }
            BrowserAnswer::Browser(BrowserAction::Open(uri)) if local => self.browse(BrowseStep::Uri(uri.clone())),
            BrowserAnswer::Browser(BrowserAction::Parent) if local => self.browse(BrowseStep::Parent),
            BrowserAnswer::Browser(BrowserAction::Roots) if local => self.browse(BrowseStep::Roots),
            BrowserAnswer::Browser(_) => FieldStep::Nothing,
            BrowserAnswer::Confirm => self.browser.current_uri.clone().map_or(FieldStep::Nothing, FieldStep::Confirm),
            BrowserAnswer::Cancel => {
                self.open = false;
                FieldStep::Nothing
            }
        }
    }

    fn browse(&mut self, step: BrowseStep) -> FieldStep {
        self.busy = true;
        self.error = None;
        FieldStep::Browse(step)
    }

    /// The listing of the step the owner sent.
    pub fn apply_listing(&mut self, listing: Result<BrowserState, String>) {
        match listing {
            Ok(browser) => {
                self.browser = browser;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
                self.browser.entries.clear();
            }
        }
        self.busy = false;
    }

    /// A folder the owner took: shown, marked as chosen, the browser closed.
    pub fn chosen(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.dirty = true;
        self.open = false;
    }

    /// Whether Select can be pressed: a folder of the computer is shown.
    pub fn can_confirm(&self) -> bool {
        self.kind == SourceKind::Local && !self.browser.roots && self.browser.current_uri.is_some() && !self.busy
    }
}

/// Opens or closes one of two panels of a form; opening it closes the other (one browser open
/// at a time, AGENTS 9). The form owns the two fields; the browser does not know there are two.
fn toggle_exclusive_panel(current_open: &mut bool, other_open: &mut bool) -> bool {
    let opened = !*current_open;
    *current_open = opened;
    if opened {
        *other_open = false;
    }
    opened
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_dir_browser::BrowserState;

    #[test]
    fn one_browser_opens_at_a_time_and_starts_at_the_folder_shown() {
        let mut projects = LocationField::new("C:/Projects");
        let mut exports = LocationField::new("exports/projekti");
        let absolute = if cfg!(windows) { "C:/Projects" } else { "/Projects" };
        projects.text = absolute.into();
        assert_eq!(projects.toggle(&mut exports), FieldStep::Browse(BrowseStep::Path(absolute.into())));
        assert!(projects.open && projects.busy);
        assert_eq!(exports.toggle(&mut projects), FieldStep::Browse(BrowseStep::Roots));
        assert!(exports.open && !projects.open);
    }

    #[test]
    fn answers_become_steps_and_a_lan_tab_does_not_browse() {
        let mut field = LocationField::new("");
        field.browser = BrowserState { roots: false, current_uri: Some("qnc://local/dir/a".into()), ..BrowserState::default() };
        assert_eq!(field.answer(&BrowserAnswer::Browser(BrowserAction::Parent)), FieldStep::Browse(BrowseStep::Parent));
        field.apply_listing(Ok(BrowserState { roots: false, current_uri: Some("qnc://local/dir/b".into()), ..BrowserState::default() }));
        assert!(field.can_confirm());
        assert_eq!(field.answer(&BrowserAnswer::Confirm), FieldStep::Confirm("qnc://local/dir/b".into()));
        assert_eq!(field.answer(&BrowserAnswer::Browser(BrowserAction::Kind(1))), FieldStep::Nothing);
        assert_eq!(field.answer(&BrowserAnswer::Browser(BrowserAction::Roots)), FieldStep::Nothing);
        field.open = true;
        field.answer(&BrowserAnswer::Cancel);
        assert!(!field.open);
    }


    #[test]
    fn opening_one_panel_closes_the_other() {
        let (mut current_open, mut other_open) = (false, true);
        assert!(toggle_exclusive_panel(&mut current_open, &mut other_open));
        assert!(current_open && !other_open);
    }

    #[test]
    fn closing_current_panel_does_not_open_the_other() {
        let (mut current_open, mut other_open) = (true, false);
        assert!(!toggle_exclusive_panel(&mut current_open, &mut other_open));
        assert!(!current_open && !other_open);
    }
}
