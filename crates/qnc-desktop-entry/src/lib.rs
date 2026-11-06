//! An application the desktop can host (docs/99 step 2, a core piece without egui): its id, its
//! tab and its priority group. The launch bar's rules (`qnc-desktop-tabs`) read it without a
//! window; `qnc-shell-desktop-api` re-exports it for the desktop adapters.

pub const MODULE_ID: &str = "qnc.module.desktop-entry";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApplicationRef {
    pub application_id: String,
    pub tab_id: String,
    pub priority_group: String,
}
