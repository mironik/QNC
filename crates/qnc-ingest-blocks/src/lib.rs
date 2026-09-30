//! The Ingest blocks on the desktop board (moved unchanged out of the Ingest form, user
//! rule 2026-09-30: forms are boards of blocks): preview, pool head, source browser,
//! clip cards and source dock. They draw the Ingest view and return intents.

pub(crate) use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Label, Layout, Rect, RichText, ScrollArea, Sense,
    Stroke, Ui, Vec2,
};

pub(crate) use qnc_ingest_application::{
    action_ids, timeline_intent_to_ingest_intent, ClipFilter, ClipView, IngestIntent,
    IngestPayload, IngestViewModel, LocationEntry, SourceKind,
};
pub(crate) use qnc_monitor::{MonitorChrome, MonitorPicture, MonitorPoster, MonitorSurface};
pub(crate) use qnc_timeline::TimelineTheme;
pub(crate) use qnc_ui_kit::FormActionBarStyle;

pub(crate) use qnc_ingest_layout::{theme::Theme, IngestContracts, IngestDirBrowser};

mod board;
mod browser_action_bar;
mod browser_entries;
mod buttons;
mod clip_grid;
mod desktop;
mod dock_chrome;
mod location_browser;
mod player_timeline;
mod pool_head;
mod preview;
mod source_dock;
mod text;

pub use desktop::render_desktop;

use board::*;
use browser_action_bar::*;
use browser_entries::*;
use buttons::*;
use clip_grid::*;
use dock_chrome::*;
use location_browser::*;
use player_timeline::*;
use pool_head::*;
use preview::*;
use source_dock::*;
use text::*;
