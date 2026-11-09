//! The reading of the whole story in one recording (user 2026-11-09): started and stopped by
//! its button; while it records Space, a mouse click or a tap on the pad marks a new reading.

use std::sync::mpsc;

use qnc_source_preview::SourcePreview;
use qnc_voice_take::{RecordedSession, Session};

use crate::Script;

/// The button of the recording: starts or stops it.
pub const SESSION: &str = "script_session";
/// A press while it records: a new reading starts here.
pub const SESSION_MARK: &str = "script_session_mark";

pub(crate) enum SessionAnswer {
    Started(Session),
    Finished(Result<RecordedSession, String>),
    Sorted(Result<qnc_session_readings::Sorted, String>),
    Cancelled,
}

/// New readings wait for their waves: the background worker makes them.
pub(crate) fn wake_worker() {
    let Some(root) = qnc_dev_diagnostics::resolve_qnc_root() else { return };
    if let Err(error) = qnc_worker_wake::wake_active(&root) {
        log(&format!("script worker not woken: {error}"));
    }
}

fn log(line: &str) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}

impl Script {
    pub(crate) fn session_key(&mut self, action_id: &str, preview: &SourcePreview) -> Option<bool> {
        match action_id {
            SESSION => Some(self.toggle_session(preview)),
            // Space goes to the recording while it runs, never to the player.
            SESSION_MARK | "play_pause" if self.session.is_some() => {
                self.session.as_mut().map(Session::mark);
                Some(true)
            }
            // A press while one OFF records: a new reading, in place of channels (user 2026-11-09).
            SESSION_MARK | "play_pause" if self.take.is_some() => {
                self.take.as_mut().map(|(_, take)| take.mark());
                Some(true)
            }
            "clear_focus" if self.session.is_some() => {
                let session = self.session.take()?;
                self.spawn_session(move || {
                    let _ = session.cancel();
                    SessionAnswer::Cancelled
                });
                Some(true)
            }
            _ => None,
        }
    }

    fn toggle_session(&mut self, preview: &SourcePreview) -> bool {
        if self.session_busy.is_some() || self.take.is_some() || self.taking.is_some() {
            return false;
        }
        if let Some(session) = self.session.take() {
            self.spawn_session(move || SessionAnswer::Finished(session.finish()));
            return true;
        }
        let setting = qnc_dev_diagnostics::resolve_qnc_root().map(|root| qnc_dev_diagnostics::config_path(&root).with_file_name(qnc_voice_record::INPUT_SETTING));
        let (Some(locator), Some(setting)) = (preview.still_locator(), setting) else {
            self.view.message = qnc_ui_text::text("script.no_project").into();
            return true;
        };
        self.view.message = qnc_ui_text::text("script.record_starting").into();
        self.spawn_session(move || match Session::start(&locator, &setting) {
            Ok(session) => SessionAnswer::Started(session),
            Err(error) => SessionAnswer::Finished(Err(error)),
        });
        true
    }

    /// Hears the recording and puts each of its readings on the OFF it reads, off the form thread.
    fn sort_session(&mut self, preview: &SourcePreview, done: &RecordedSession) {
        let Some(locator) = preview.still_locator() else { return };
        let offs: Vec<qnc_session_readings::Off> = self
            .view
            .blocks
            .iter()
            .filter(|row| row.recorded && !row.block.text.trim().is_empty())
            .map(|row| qnc_session_readings::Off { block_id: row.block.id, mark: row.mark.clone(), text: row.block.text.clone() })
            .collect();
        self.view.message = qnc_ui_text::text("script.session_sorting").into();
        let done = done.clone();
        self.spawn_session(move || {
            let sorted = qnc_session_readings::sort(&locator, &done.wav_uri, &done.timecode, &done.marks, &offs, done.silent);
            wake_worker();
            SessionAnswer::Sorted(sorted)
        });
    }

    fn spawn_session(&mut self, work: impl FnOnce() -> SessionAnswer + Send + 'static) {
        let (send, answer) = mpsc::channel();
        if std::thread::Builder::new().name("qnc-script-session".into()).spawn(move || _ = send.send(work())).is_ok() {
            self.session_busy = Some(answer);
        }
    }

    pub(crate) fn session_answers(&mut self, preview: &SourcePreview) {
        if let Some(answer) = self.session_busy.as_ref().and_then(|answer| answer.try_recv().ok()) {
            self.session_busy = None;
            match answer {
                SessionAnswer::Started(session) => {
                    self.session = Some(session);
                    self.view.message = qnc_ui_text::text("script.recording").into();
                }
                SessionAnswer::Finished(Ok(done)) => {
                    log(&format!("script-session stopped seconds={:.1} presses={} silent={}", done.seconds, done.marks.len(), done.silent));
                    if done.silent {
                        self.message_after_write = Some(qnc_notice::raised(qnc_ui_text::fill("script.take_silent", &[("take", "1")])));
                    }
                    self.sort_session(preview, &done);
                    self.write(preview, move |target| qnc_script_db::add_session(target, done.wav_uri, done.seconds, done.timecode, done.marks).map(|_| None));
                }
                SessionAnswer::Finished(Err(error)) => {
                    log(&format!("script-session failed: {error}"));
                    self.view.message = qnc_notice::raised(error);
                }
                SessionAnswer::Sorted(Ok(sorted)) => {
                    log(&format!("script-session sorted placed={} left={}", sorted.placed, sorted.left));
                    self.view.message = qnc_ui_text::fill("script.session_sorted", &[("placed", &sorted.placed.to_string()), ("left", &sorted.left.to_string())]);
                }
                SessionAnswer::Sorted(Err(error)) => {
                    log(&format!("script-session not sorted: {error}"));
                    self.view.message = qnc_notice::raised(error);
                }
                SessionAnswer::Cancelled => self.view.message = qnc_ui_text::text("script.record_cancelled").into(),
            }
        }
        self.view.session = self.session.as_ref().map(|session| (session.seconds(), session.take_peak(), session.marks()));
        self.view.session_busy = self.session_busy.is_some();
    }
}
