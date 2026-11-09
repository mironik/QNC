//! The source timeline as it is drawn (docs/99 step 3): the dock of any form with its header,
//! the level bar and the one source timeline, and the header's action buttons. Its state, keys
//! and the clip's sound are the core piece `qnc-source-timeline`; this draws what it gives.

use eframe::egui::{Rect, Ui};
use qnc_source_dock::SourceTimeline as DockTimeline;
use qnc_source_timeline::SourceTimelineView;
use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_timeline_assets::SourceTimelineAssets;

pub const MODULE_ID: &str = "qnc.module.source-timeline-view";

/// Draws the source dock of any form: its frame, the header row (the clip's `title` and
/// whatever `header` adds) and the one source timeline under it.
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    (theme, header_gap): (&qnc_desktop_theme::Theme, f32),
    title: &str,
    header: impl FnOnce(&mut Ui),
    (projection, assets): (&TimelineProjection, &SourceTimelineAssets),
    view: &SourceTimelineView,
) -> TimelineIntent {
    show_with(ui, rect, (theme, header_gap), title, header, (projection, assets), view, None).0
}

/// What a form draws under the header in place of the timeline, and its answer.
pub type Instead<'a> = dyn FnMut(&mut Ui) -> Option<qnc_action_event::ActionEvent> + 'a;

/// As `show`, with `instead` drawn under the header in place of the timeline (the enlarged
/// slot keeps the header and its buttons).
#[allow(clippy::too_many_arguments)]
pub fn show_with(
    ui: &mut Ui,
    rect: Rect,
    (theme, header_gap): (&qnc_desktop_theme::Theme, f32),
    title: &str,
    header: impl FnOnce(&mut Ui),
    (projection, assets): (&TimelineProjection, &SourceTimelineAssets),
    view: &SourceTimelineView,
    instead: Option<&mut Instead<'_>>,
) -> (TimelineIntent, Option<qnc_action_event::ActionEvent>) {
    let style = &qnc_theme_styles::dock(theme, header_gap);
    // A statement enlarged (user 2026-11-09): the same timeline, a narrower window of its clip.
    let window = view.enlarged.as_ref().map(|enlarged| {
        let mut window = *projection;
        (window.range_start_frame, window.duration_frames) = (enlarged.from, enlarged.to.saturating_sub(enlarged.from).max(1));
        window
    });
    let (projection, assets) = window.as_ref().zip(view.enlarged.as_ref()).map_or((projection, assets), |(window, enlarged)| (window, enlarged.assets.as_ref()));
    let timeline = DockTimeline::from_assets(projection, qnc_theme_styles::timeline(theme), assets)
        .with_timecode(view.timecode)
        .with_a1_channel(view.a1)
        .with_a2_channel(view.a2)
        .with_wave_over_video(view.wave_over_video)
        // The clip's A1/A2 level curves (user 2026-11-07), drawn as on a Wrap row.
        .with_level_curves(qnc_trim_overlay::timeline_level_curves(&view.level_curves, projection.range_start(), projection.range_start() + projection.duration_frames().max(1), 0));
    let mut answer = None;
    let slot = &mut answer;
    let mut toggled = None;
    // The level bar block (meter, NORM and AI) in the middle of the bar above the timeline.
    let header = |ui: &mut Ui| {
        let row = ui.max_rect();
        toggled = qnc_level_bar::show(ui, row, view.levels, (view.normalize, view.sound_assist), &qnc_theme_styles::timeline(theme));
        header(ui);
    };
    let mut drawn = instead.map(|instead| move |ui: &mut Ui| *slot = instead(ui));
    let intent = qnc_source_dock::show_timeline_dock_with(ui, rect, style, title, header, timeline, drawn.as_mut().map(|draw| draw as &mut dyn FnMut(&mut Ui)));
    drop(drawn);
    let norm = toggled.map(qnc_action_event::ActionEvent::key);
    let asked = view.question.as_ref().and_then(|choice| qnc_choice_box::show(ui.ctx(), theme, choice)).map(qnc_action_event::ActionEvent::key);
    (intent, asked.or(answer).or(norm))
}

/// One action button of the header, as a form lists them (label, catalog action).
pub struct HeaderAction<'a> {
    pub label: &'a str,
    pub action_id: Option<&'a str>,
    /// The strong button of the header (the form's main action).
    pub strong: bool,
}

/// The header's action buttons, right to left as listed, each with its state from the form
/// (enabled, on) and the frame of the one the keyboard is on; the clicked action.
pub fn header_actions<'a>(
    ui: &mut Ui,
    theme: &qnc_desktop_theme::Theme,
    actions: impl IntoIterator<Item = HeaderAction<'a>>,
    enabled: impl Fn(&str) -> bool,
    on: impl Fn(&str) -> bool,
    focused: Option<&str>,
) -> Option<&'a str> {
    let mut clicked = None;
    for action in actions {
        let id = action.action_id.unwrap_or_default();
        let (enabled, on) = (action.action_id.is_some_and(&enabled), action.action_id.is_some_and(&on));
        let response = if action.strong {
            qnc_ui_kit::strong_button(ui, action.label, enabled, theme.accent, theme.chrome_control_height)
        } else {
            qnc_ui_kit::chrome_button(ui, action.label, enabled, on, qnc_theme_styles::button(theme))
        };
        qnc_ui_kit::paint_focus(ui, response.rect.expand(2.0), action.action_id.is_some() && focused == Some(id), theme.focus);
        if response.clicked() {
            clicked = action.action_id;
        }
    }
    clicked
}

