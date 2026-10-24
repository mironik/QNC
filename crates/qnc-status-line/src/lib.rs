//! The status line of a surface (user 2026-10-24: a finished export says so with a link to
//! its file): the text shown and, when the text names a file, that file as a link. Opening
//! the link shows the file in the file browser of the OS (Windows Explorer, macOS Finder,
//! the folder on Linux); nothing is written or run besides that browser.

use std::path::Path;

pub const MODULE_ID: &str = "qnc.module.status-line";

/// How the status line colours its text (user 2026-10-24: an export running is red, a
/// finished one green).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Plain,
    /// Work going on, or a failure: red.
    Alert,
    /// Work done: green.
    Done,
}

/// What the status line shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Status<'a> {
    pub text: &'a str,
    /// A local file the text speaks of.
    pub link: Option<&'a str>,
    pub tone: Tone,
}

impl<'a> Status<'a> {
    /// A text without a link, in the line's own colour.
    pub fn plain(text: &'a str) -> Self {
        Self { text, ..Default::default() }
    }
}

/// The first message that says something, with its link and tone.
pub fn first<'a>(messages: &[Status<'a>], otherwise: &'a str) -> Status<'a> {
    messages
        .iter()
        .find(|status| !status.text.is_empty())
        .copied()
        .unwrap_or(Status { text: otherwise, link: None, tone: Tone::Plain })
}

/// Shows `file` in the file browser of this OS, chosen in its folder.
pub fn open(file: &str) -> Result<(), String> {
    let path = Path::new(file);
    if !path.exists() {
        return Err(format!("{file}: not found"));
    }
    let mut command = browser(path);
    command.spawn().map(|_| ()).map_err(|error| format!("{file}: {error}"))
}

#[cfg(windows)]
fn browser(path: &Path) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut command = std::process::Command::new("explorer.exe");
    command.raw_arg(format!("/select,\"{}\"", path.display()));
    command
}

#[cfg(target_os = "macos")]
fn browser(path: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    command.arg("-R").arg(path);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn browser(path: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(path.parent().unwrap_or(path));
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_message_that_says_something_keeps_its_link() {
        let done = Status { text: "Exported: x.mxf", link: Some("x.mxf"), tone: Tone::Done };
        let status = first(&[Status { text: "", link: Some("a"), tone: Tone::Alert }, done, Status { text: "player", ..Default::default() }], "catalog");
        assert_eq!(status, done);
        assert_eq!(first(&[Status::default()], "catalog"), Status { text: "catalog", link: None, tone: Tone::Plain });
        assert!(open("Z:/no/such/file.mxf").is_err());
    }
}
