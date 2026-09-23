//! Compatibility path for older Ingest callers.
//!
//! The content database owner is `qnc-content-store`. Ingest keeps only its
//! registry here; this module must not grow a second content DB implementation.

pub use qnc_content_store::*;
