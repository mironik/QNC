use std::path::PathBuf;

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopNavigation {
    NextGroup,
}

impl DesktopNavigation {
    pub fn action_id(self) -> &'static str {
        match self {
            Self::NextGroup => "shell_next_group",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApplicationRef {
    pub application_id: String,
    pub tab_id: String,
    pub priority_group: String,
}

pub trait ShellDesktopApp {
    fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui);

    /// Display-only text from the active surface, never a workflow payload.
    fn footer_status(&self) -> Option<&str> {
        None
    }

    fn on_activated(&mut self) {}

    fn take_navigation_request(&mut self) -> Option<DesktopNavigation> {
        None
    }

    fn navigation_sequence(&self) -> Result<Vec<DesktopApplicationRef>, String> {
        Err("Aplikacija nema navigacijski DB prikaz.".into())
    }
}

pub fn next_group_tab(
    current: &str,
    sequence: &[DesktopApplicationRef],
    available: &[DesktopApplicationRef],
) -> Result<Option<String>, String> {
    let current_index = sequence
        .iter()
        .position(|entry| entry.application_id == current)
        .ok_or("Trenutna aplikacija nije u spremljenom slijedu.")?;
    let registered = |entry: &DesktopApplicationRef| {
        available
            .iter()
            .find(|app| app.application_id == entry.application_id && app.tab_id == entry.tab_id)
            .ok_or_else(|| {
                format!(
                    "Odabrana aplikacija nije dostupna: {}",
                    entry.application_id
                )
            })
    };
    let group = |entry: &DesktopApplicationRef| -> Result<String, String> {
        let value = entry.priority_group.as_str();
        if value.len() != 1 || !value.as_bytes()[0].is_ascii_lowercase() {
            return Err("Neispravna prioritetna grupa.".into());
        }
        Ok(value.to_string())
    };
    let source = &sequence[current_index];
    let source_group = group(registered(source)?)?;
    let mut previous = None;
    for entry in sequence {
        let entry_group = group(entry)?;
        if previous.as_ref().is_some_and(|value| value >= &entry_group) {
            return Err("Spremljeni slijed nema jedinstvene rastuce grupe.".into());
        }
        previous = Some(entry_group);
    }
    if group(source)? != source_group {
        return Err("Grupa trenutne aplikacije ne odgovara spremljenom slijedu.".into());
    }
    let target = sequence
        .iter()
        .find(|entry| entry.priority_group > source_group);
    let Some(target) = target else {
        return Ok(None);
    };
    let installed = registered(target)?;
    let target_group = group(installed)?;
    if target_group <= source_group || target.priority_group != target_group {
        return Err("Ciljna aplikacija nije u sljedecoj odabranoj grupi.".into());
    }
    Ok(Some(installed.tab_id.clone()))
}

#[derive(Clone, Copy)]
pub struct EmbeddedAppFactory {
    pub desktop_entry: &'static str,
    pub create: fn(PathBuf) -> Result<Box<dyn ShellDesktopApp>, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, group: &str) -> DesktopApplicationRef {
        DesktopApplicationRef {
            application_id: format!("qnc.{id}"),
            tab_id: id.into(),
            priority_group: group.into(),
        }
    }

    #[test]
    fn follows_selected_groups_including_gaps_and_alternative_apps() {
        let sequence = vec![app("alternative", "a"), app("short", "c"), app("last", "z")];
        let mut available = sequence.clone();
        available.push(app("standard", "c"));
        assert_eq!(
            next_group_tab("qnc.alternative", &sequence, &available).unwrap(),
            Some("short".into())
        );
        assert_eq!(
            next_group_tab("qnc.short", &sequence, &available).unwrap(),
            Some("last".into())
        );
        assert_eq!(
            next_group_tab("qnc.last", &sequence, &available).unwrap(),
            None
        );
    }

    #[test]
    fn rejects_old_duplicate_reversed_or_changed_groups() {
        let available = vec![app("first", "a"), app("next", "b")];
        for groups in [
            ["", ""],
            ["a", "a"],
            ["b", "a"],
            ["a", "C"],
            ["a", "c"],
            ["0", "1"],
        ] {
            let sequence = vec![app("first", groups[0]), app("next", groups[1])];
            assert!(
                next_group_tab("qnc.first", &sequence, &available).is_err(),
                "{groups:?}"
            );
        }
    }

    #[test]
    fn never_substitutes_missing_selected_app_or_guesses_sequence() {
        let sequence = vec![app("first", "a"), app("missing", "b"), app("last", "c")];
        let available = vec![app("first", "a"), app("other", "b"), app("last", "c")];
        assert!(next_group_tab("qnc.first", &sequence, &available).is_err());
        assert!(next_group_tab("qnc.first", &[], &available).is_err());
        assert!(next_group_tab("qnc.unknown", &sequence, &available).is_err());
    }

    #[test]
    fn navigation_action_is_in_external_keyboard_contract() {
        assert!(
            include_str!("../../../contracts/qnc-keyboard-shortcuts.json")
                .contains(&format!("\"{}\"", DesktopNavigation::NextGroup.action_id()))
        );
    }
}
