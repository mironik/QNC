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
    /// The SubClip whose IN or OUT was taken on the source timeline: `(shot, clip)`.
    fine_tuning: Option<(String, String)>,
    /// The cut last written for a SubClip (its row is read again after the write).
    written: Option<(String, (u64, u64))>,
}

/// A SubClip open on the source timeline: its id, clip and saved IN/OUT.
pub type OpenShort<'a> = (&'a str, &'a str, (u64, u64));

impl ShortSave {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fine tuning of an open SubClip (user 2026-10-29: it went back to its old cut): while
    /// its IN or OUT is taken on the source timeline of its clip the tuning runs; once the
    /// mark is let go (Enter, a click) a cut other than the saved one is written as its new
    /// IN/OUT, and the outcome is given like a save's (this is `poll` too). New marks set
    /// with I and O without taking one stay a new SubClip (Add SubClip).
    pub fn follow(&mut self, open: Option<OpenShort<'_>>, shown_clip: Option<&str>, marks: Option<(u64, u64)>, taken: bool, target: Option<&ProjectDbTarget>) -> Option<Result<Saved, String>> {
        self.tune(open, shown_clip, marks, taken, target);
        self.poll()
    }

    fn tune(&mut self, open: Option<OpenShort<'_>>, shown_clip: Option<&str>, marks: Option<(u64, u64)>, taken: bool, target: Option<&ProjectDbTarget>) {
        let open = open.filter(|(_, clip, _)| Some(*clip) == shown_clip);
        if taken {
            self.fine_tuning = open.map(|(shot, clip, _)| (shot.to_string(), clip.to_string())).or(self.fine_tuning.take());
            return;
        }
        let Some((shot, _)) = self.fine_tuning.take() else { return };
        let (Some((open_shot, clip_id, saved)), Some(cut), Some(target)) = (open, marks, target) else { return };
        let already = self.written.as_ref().is_some_and(|(id, written)| *id == shot && *written == cut);
        if open_shot != shot || cut == saved || already || self.pending.is_some() {
            return;
        }
        log(&format!("short-save fine tuning {shot} {saved:?} -> {cut:?}"));
        self.written = Some((shot.clone(), cut));
        let (target, clip_id) = (target.clone(), clip_id.to_string());
        let (sender, receiver) = mpsc::channel();
        let started = std::thread::Builder::new().name("qnc-short-trim".into()).spawn(move || {
            let trimmed = qnc_virtual_shots::trim_short_now(&target, &shot, cut.0, cut.1).map(|_| Saved { shot_id: shot, clip_id, marks: cut });
            let _ = sender.send(trimmed);
        });
        if started.is_ok() {
            self.pending = Some(receiver);
        }
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
        // Whether the clip is imported is the database's, read when the short is written
        // (`qnc-virtual-shots`), not the pool row a form read earlier (live 2026-10-29: a clip
        // imported after the pool was read could not become a SubClip).
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
    fn nothing_is_written_without_a_clip_marks_and_a_database() {
        let mut save = ShortSave::new();
        assert_eq!(save.start(None, &[], Some((0, 5)), None, Some("p")), Err(text("story.msg.pick_clip")));
        // A row read before the import does not refuse: the database decides when it writes.
        assert_eq!(save.start(Some("c"), &[clip(false)], Some((0, 5)), None, Some("p")), Err(text("story.msg.db_not_writable")));
        assert_eq!(save.start(Some("c"), &[clip(true)], None, None, Some("p")), Err(text("story.msg.marks_not_confirmed")));
        assert_eq!(save.start(Some("c"), &[clip(true)], Some((0, 5)), None, Some("p")), Err(text("story.msg.db_not_writable")));
        assert!(save.poll().is_none(), "no write was started");
    }
}
