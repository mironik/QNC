mod metadata;
mod publish;
mod records;
mod scan;
mod run;
pub mod selection_config;
mod session;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
mod types;

pub use selection_config::{Binding, ProbeConfig, SelectionConfig, SourceConfig};
pub use run::run;
pub use session::SelectSession;
pub use types::*;
pub(crate) use run::*;

use qnc_camera_adapter::{CameraRegistry, MetadataSufficiency};
use qnc_ingest_store::content::{
    Access, CatalogClip, ContentTarget, ContentWriteData, ContentWriteResult,
    ContentWriteTransport, ImportStatus, StoredClip,
};
use qnc_media_probe::ProbeBackend;
use qnc_media_record_db::project::ProjectMediaRecords as Client;
use qnc_media_record_db::contract::*;
use qnc_source_groups::IndexDocument;
use qnc_source_reader::{SourceReader, SourceReference, MAX_TEXT_BYTES};
use selection_config::Result;
use std::fmt;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread::JoinHandle,
};

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
