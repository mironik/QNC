//! Import execution for a claimed clip.
//!
//! What is copied and where it goes is decided only by the project settings
//! (`storage.ingest_media`, `playback.input`) through [`IngestWorkPlan`]:
//!
//! * `link`: media is not copied; the imported media is the source URI chosen by
//!   `playback.input`;
//! * `proxy`: the proxy of the card is copied into the project `proxy` folder;
//! * `original`: the original is copied into the project `original` folder.
//!
//! Source media is read through the media stream transport (local disk, LAN or
//! intranet behind one contract), never by a path. The copy is a plain copy. The outcome is
//! written through the content transport
//! (`finish_import`). Nothing here depends on an operating system or a drive.
//!
//! With the media the poster is copied too. Not done here: proxy generation (transcode).

mod config;
mod optimize;

mod copy;
mod plan;
mod queue;
mod run;
pub use config::ConfigMediaOpener;
mod process;

pub use process::{ImportRun, 
    launch_worker, run_import, run_service, ImportSummary, WORKER_EXECUTABLE,
};

pub use copy::{import_clip, import_clip_beating, import_poster};
pub use plan::{action_for, Action, Folder};
pub use queue::{queue_selected, ImportQueue, TransportQueue};
pub use run::{drain, run_next, Outcome};
pub(crate) use copy::*;
pub(crate) use plan::*;
use qnc_ingest_store::content::{ContentClient, ImportedCopy, StoredClip};
use qnc_media_metadata::MediaRepresentation;
use qnc_ingest_work_plan::{IngestMedia, IngestWorkPlan, PlaybackInput};
use std::{io::Read, path::Path};

pub const MODULE_ID: &str = "qnc.module.ingest-import-worker";
pub const VERSION: &str = "0.1.0";


/// One open source medium: bytes and the length the transport reports.
pub trait MediaRead: Read + Send {
    fn byte_len(&self) -> u64;
}

/// Opens source media by QNC URI: local disk, LAN or intranet.
pub trait MediaOpener: Send + Sync {
    fn open(&self, media_uri: &str) -> Result<Box<dyn MediaRead>, String>;
    /// The copy waits while this is true (for example while the player works).
    fn paused(&self) -> bool {
        false
    }
    /// A local file of the medium when its source is on this machine: what a local extractor
    /// needs. `None` for a source reached over LAN or intranet.
    fn local_path(&self, _media_uri: &str) -> Option<std::path::PathBuf> {
        None
    }
    /// The description of a file the import made in the project (one probe of the new
    /// file, still the Ingest process), recorded under `media_uri`.
    fn describe(&self, _media_uri: &str, _file: &Path) -> Result<MediaRepresentation, String> {
        Err(qnc_ui_text::text("import.new_file_cannot_be_described_this").into())
    }
}

#[cfg(test)]
mod tests;
