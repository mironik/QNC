pub(super) use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Label, Layout, Rect, RichText, ScrollArea, Sense,
    Stroke, Ui, Vec2,
};

pub(super) use qnc_ingest_application::{
    action_ids, timeline_intent_to_ingest_intent, ClipFilter, ClipView, IngestIntent,
    IngestPayload, IngestViewModel, LocationEntry, SourceKind,
};
pub(super) use qnc_monitor::{MonitorChrome, MonitorPicture, MonitorPoster, MonitorSurface};
pub(super) use qnc_timeline::TimelineTheme;
pub(super) use qnc_ui_kit::FormActionBarStyle;

pub(super) use crate::{
    layout_contract::{IngestContracts, IngestDirBrowser},
    theme::Theme,
};

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
