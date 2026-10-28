//! Saving a SubClip (audit 2026-10-25: the checks and the write were in the Story application,
//! and the write waited for the database on the form thread). The clip must be one of the
//! imported clips of the pool and IN/OUT confirmed on the player; the short is written in the
//! background through `qnc-virtual-shots`, and the outcome is taken once it is there.

use std::sync::mpsc::{self, Receiver, TryRecvError};

use qnc_clip_row::ClipRow;
use qnc_db_broker::ProjectDbTarget;

pub const MODULE_ID: &str = "qnc.module.short-save";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A saved short: its id, its clip and its IN/OUT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub shot_id: String,
    pub clip_id: String,
    pub marks: (u64, u64),
}

/// The save of one short at a time.
#[derive(Default)]
pub struct ShortSave {
    pending: Option<Receiver<Result<Saved, String>>>,
}

impl ShortSave {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks what is asked and starts the write: the chosen clip of `clips`, the marks the
    /// source timeline shows, the project database and project. The error is the message to
    /// show; nothing is written then.
    pub fn start(
        &mut self,
        chosen: Option<&str>,
        clips: &[ClipRow],
        marks: Option<(u64, u64)>,
        target: Option<&ProjectDbTarget>,
        project_id: Option<&str>,
    ) -> Result<(), String> {
        // Why a SubClip was not saved goes to the log too (live 2026-10-28: none was saved and
        // the log did not say why).
        let started = self.check_and_write(chosen, clips, marks, target, project_id);
        if let Err(error) = &started {
            log(&format!("short-save refused clip={chosen:?} marks={marks:?}: {error}"));
        }
        started
    }

    fn check_and_write(
        &mut self,
        chosen: Option<&str>,
        clips: &[ClipRow],
        marks: Option<(u64, u64)>,
        target: Option<&ProjectDbTarget>,
        project_id: Option<&str>,
    ) -> Result<(), String> {
        let clip_id = chosen.ok_or_else(|| text("story.msg.pick_clip"))?;
        let clip = clips.iter().find(|clip| clip.clip_id == clip_id).ok_or_else(|| text("ingest.msg.clip_not_found"))?;
        if !clip.imported {
            return Err(qnc_ui_text::fill("story.msg.clip_not_imported", &[("clip", clip_id)]));
        }
        let (in_frame, out_frame) = marks.ok_or_else(|| text("story.msg.marks_not_confirmed"))?;
        let target = target.ok_or_else(|| text("story.msg.db_not_writable"))?.clone();
        let project_id = project_id.ok_or_else(|| text("story.msg.no_active_project"))?.to_string();
        let (clip_id, name) = (clip.clip_id.clone(), clip.name.clone());
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("qnc-short-save".into())
            .spawn(move || {
                let saved = qnc_virtual_shots::save_short_now(&target, &project_id, &clip_id, &name, in_frame, out_frame)
                    .map(|shot| Saved { shot_id: shot.shot_id, clip_id, marks: (in_frame, out_frame) });
                let _ = sender.send(saved);
            })
            .map_err(|error| error.to_string())?;
        self.pending = Some(receiver);
        Ok(())
    }

    /// The outcome of the save, once.
    pub fn poll(&mut self) -> Option<Result<Saved, String>> {
        let outcome = match self.pending.as_ref()?.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(text("story.msg.db_not_writable")),
        };
        self.pending = None;
        match &outcome {
            Ok(saved) => log(&format!("short-save saved {} clip={} marks={:?}", saved.shot_id, saved.clip_id, saved.marks)),
            Err(error) => log(&format!("short-save failed: {error}")),
        }
        Some(outcome)
    }
}

fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}

fn text(code: &'static str) -> String {
    qnc_ui_text::text(code).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(imported: bool) -> ClipRow {
        ClipRow { clip_id: "c".into(), name: "C".into(), imported, ..ClipRow::default() }
    }

    #[test]
    fn nothing_is_written_without_an_imported_clip_marks_and_a_database() {
        let mut save = ShortSave::new();
        assert_eq!(save.start(None, &[], Some((0, 5)), None, Some("p")), Err(text("story.msg.pick_clip")));
        assert_eq!(save.start(Some("c"), &[clip(false)], Some((0, 5)), None, Some("p")).unwrap_err(), qnc_ui_text::fill("story.msg.clip_not_imported", &[("clip", "c")]));
        assert_eq!(save.start(Some("c"), &[clip(true)], None, None, Some("p")), Err(text("story.msg.marks_not_confirmed")));
        assert_eq!(save.start(Some("c"), &[clip(true)], Some((0, 5)), None, Some("p")), Err(text("story.msg.db_not_writable")));
        assert!(save.poll().is_none(), "no write was started");
    }
}
