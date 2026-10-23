//! One Select of a source in Ingest (user 2026-10-23: "Ingest into blocks"): the record of
//! the chosen source, the Select session running off the caller's thread with the cameras
//! it can read, and its events applied to the clip list the form shows. The work itself is
//! `qnc-ingest-select`; the clip list rules are `qnc-ingest-clip-list`. Nothing here is a
//! source of truth: Select writes the project database, the list only shows it.

use std::sync::Arc;

use qnc_camera_adapter::CameraRegistry;
use qnc_ingest_clip_list::{ClipListItem, SelectEventState};
use qnc_ingest_select::{SelectSession, SelectTarget, SelectionConfig};
use qnc_ingest_store::SourceSelectionRecord;
use qnc_source_reader::SourceReference;

pub const MODULE_ID: &str = "qnc.module.ingest-select-run";

/// What the events of one poll changed, together.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Polled {
    pub changed: bool,
    /// The last message of the events.
    pub message: Option<String>,
    pub warning_count: Option<usize>,
    /// Clips gone from the source: their artifacts are forgotten.
    pub removed_ids: Vec<String>,
    /// The clip on the preview is one of them.
    pub preview_removed: bool,
    /// Select ended: `Some(true)` when it went through.
    pub finished: Option<bool>,
}

#[derive(Debug, Default)]
pub struct SelectRun {
    session: SelectSession,
    events: SelectEventState,
    cameras: Arc<CameraRegistry>,
}

impl SelectRun {
    /// The cameras Select can read (composition root only).
    pub fn with_cameras(mut self, cameras: CameraRegistry) -> Self {
        self.cameras = Arc::new(cameras);
        self
    }

    /// Starts Select of the chosen source; a running one is stopped first.
    pub fn start(&mut self, config: SelectionConfig, selected: SourceReference, target: SelectTarget) -> Result<(), String> {
        self.session.start(config, selected, target, self.cameras.clone())?;
        self.events.reset();
        Ok(())
    }

    pub fn is_busy(&self) -> bool {
        self.session.has_pending_work()
    }

    pub fn cancel(&mut self) {
        self.session.cancel();
    }

    /// Applies the events Select sent since the last call to `clips`.
    pub fn poll<T: ClipListItem>(&mut self, clips: &mut Vec<T>, preview_clip_id: Option<&str>) -> Polled {
        let mut polled = Polled::default();
        for event in self.session.poll(64) {
            polled.changed = true;
            let outcome = qnc_ingest_clip_list::apply_select_event(clips, preview_clip_id, &mut self.events, event);
            polled.warning_count = outcome.warning_count.or(polled.warning_count);
            polled.message = outcome.message.or(polled.message);
            polled.removed_ids.extend(outcome.removed_ids);
            polled.preview_removed |= outcome.preview_removed;
            if outcome.finished {
                polled.finished = Some(outcome.finished_ok);
            }
        }
        polled
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn from_session_for_test(session: SelectSession) -> Self {
        Self { session, ..Self::default() }
    }
}

/// The record of the chosen source: its name is the name of the medium, else the folder
/// the browser shows, else its URI.
pub fn source_record(
    uri: &str,
    kind: &str,
    chosen: &qnc_source_browse::SourceMetadata,
    path_label: &str,
    private_local_path: Option<std::path::PathBuf>,
) -> SourceSelectionRecord {
    let display_name = [chosen.name.as_str(), path_label]
        .into_iter()
        .find(|name| !name.trim().is_empty())
        .unwrap_or(uri)
        .to_string();
    SourceSelectionRecord {
        source_uri: uri.to_string(),
        source_kind: kind.to_string(),
        display_name,
        serial_number: chosen.serial_number.clone(),
        volume_name: chosen.volume_name.clone(),
        private_local_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_source_is_named_after_its_medium_else_its_folder_else_its_uri() {
        let mut chosen = qnc_source_browse::SourceMetadata { name: "FX6 card".into(), serial_number: "S1".into(), volume_name: "V".into() };
        assert_eq!(source_record("qnc://local/source/a", "local", &chosen, "E:/A", None).display_name, "FX6 card");
        chosen.name = " ".into();
        assert_eq!(source_record("qnc://local/source/a", "local", &chosen, "E:/A", None).display_name, "E:/A");
        let record = source_record("qnc://local/source/a", "local", &chosen, "", None);
        assert_eq!((record.display_name.as_str(), record.serial_number.as_str()), ("qnc://local/source/a", "S1"));
    }

    #[test]
    fn a_run_without_a_select_is_idle_and_polls_nothing() {
        let mut run = SelectRun::default();
        let mut clips: Vec<qnc_clip_row::ClipRow> = Vec::new();
        assert!(!run.is_busy());
        assert_eq!(run.poll(&mut clips, None), Polled::default());
    }
}
