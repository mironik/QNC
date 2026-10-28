//! The Media Assist and Story form on the desktop board (rule 11: a form is only a layout
//! grid of panels). Each place gets its public panel with the editorial layout's measures and
//! texts and the state of the editorial view; the panel's answer becomes an editorial intent.

use eframe::egui::{self, Align, Layout, Rect, Ui};

use qnc_editorial_application::{EditorialIntent, EditorialView, LibraryTab};
use qnc_editorial_layout::{theme::Theme, EditorialContracts};
use qnc_media_card::{CardGridAction, CardMetrics, StatusDotsMode, Ticks};
use qnc_panel_focus::{paint_focus, Panel};
use qnc_timeline::TimelineIntent;

/// Media Assist and Story on the one desktop board: preview in the monitor, pool tabs and
/// transport in the head row, the clip cards in the body, the group's right panel (Segmenti
/// for Story) on the right, the source timeline in the dock; drawn in `frame` as one layout
/// tree with it.
pub fn render_desktop(
    ui: &mut Ui,
    frame: &mut qnc_board::Frame<'_>,
    contracts: &EditorialContracts,
    theme: &Theme,
    view: &EditorialView,
) -> Option<EditorialIntent> {
    let (faces, board) = (theme.board_faces(), contracts.board_layout());
    let framed = |intent, ui: &Ui, rect, panels: &[Panel]| {
        paint_focus(ui, rect, panels.contains(&view.focus.panel), theme.focus);
        intent
    };
    qnc_board::show_in_frame(ui, frame, &board, |name| faces.named(name), |ui, block, rect| qnc_dev_diagnostics::timed_block(block, || match block {
        "preview" => preview(ui, rect, contracts, theme, view),
        "pool-head" => framed(pool_tabs(ui, rect, contracts, theme, view), ui, rect, &[Panel::PoolTabs]),
        "transport" => qnc_transport_panel::show(ui, rect, theme, contracts.tab_gap()).map(|action| EditorialIntent::action(&action)),
        "clip-cards" => framed(pool(ui, contracts, theme, view), ui, rect, &[Panel::Pool]),
        // The group's right panel by its layout contract (Segmenti for Story, the transcript for Audio AI).
        "right-panel" => {
            let (answer, segments) = qnc_right_panel::show_tabbed(ui, &contracts.composition().right_panel, &contracts.composition().right_panel_tabs, theme, &qnc_right_panel::Shown { segments: &view.segments, cover_lengths: &contracts.editorial.cover_lengths, levels: view.preview.output_levels, transcript: &view.focus.transcript, shots: &view.focus.shots, script: &view.focus.script, timecode: view.source_timeline.timecode.as_ref() });
            framed(answer.map(EditorialIntent::Action), ui, rect, if segments { &[Panel::Segments] } else { &[] })
        }
        "source-dock" => framed(source_dock(ui, contracts, theme, view), ui, rect, if view.segments.zoom.is_some() { &[Panel::Segments, Panel::SourceButtons] } else { &[Panel::SourceTimeline, Panel::SourceButtons] }),
        _ => None,
    }))
}

fn preview(ui: &mut Ui, rect: Rect, contracts: &EditorialContracts, theme: &Theme, view: &EditorialView) -> Option<EditorialIntent> {
    let label = view.current_clip_label().unwrap_or(contracts.editorial.preview.empty_label.as_str());
    let turned = qnc_trim_overlay::show(ui.ctx(), &view.segments); // the trim zoom, its wheel the panel's
    let shown = view.shown_clip().map(|clip| clip as &dyn qnc_clip_row::PoolItem);
    qnc_monitor_panel::show(ui, rect, egui::Id::new(("qnc-monitor", "editorial-source")), theme, &view.preview.monitor(), shown, label);
    turned.map(EditorialIntent::Action) // the monitor gives no intent
}

fn pool_tabs(ui: &mut Ui, rect: Rect, contracts: &EditorialContracts, theme: &Theme, view: &EditorialView) -> Option<EditorialIntent> {
    let tabs: Vec<_> = contracts.pool_tabs().iter().map(|tab| qnc_pool_tabs_panel::PoolTab { label: tab.label(), action_id: tab.action_id() }).collect();
    qnc_pool_tabs_panel::show(ui, rect, (theme, contracts.tab_gap()), &tabs, |id| view.tab_selected(id), |id| view.action_enabled(id)).map(EditorialIntent::action)
}

/// The one source timeline with this group's action buttons in its header.
fn source_dock(ui: &mut Ui, contracts: &EditorialContracts, theme: &Theme, view: &EditorialView) -> Option<EditorialIntent> {
    let (rect, mut clicked) = (ui.available_rect_before_wrap(), None);
    let label = view.current_clip_label().unwrap_or(&contracts.editorial.source_dock.clip_label_fallback);
    let actions = contracts.composition().source_dock.actions_rtl.iter().map(|action| qnc_source_timeline::HeaderAction { label: action.label(), action_id: action.action_id(), strong: false });
    let header = |ui: &mut Ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (enabled, on) = (|id: &str| view.action_enabled(id), |id: &str| view.segments.action_selected(id) || view.focus.transcript.selected(id));
            clicked = qnc_source_timeline::header_actions(ui, theme, actions, enabled, on, view.focus.button.as_deref());
        });
    };
    // The enlarged slot under the header in place of the timeline: the buttons stay (user 2026-10-22).
    let mut zoom = qnc_segment_panel::zoom_body(&view.segments, qnc_theme_styles::timeline(theme), &contracts.editorial.cover_lengths);
    let (dock, instead) = ((theme, contracts.editorial.source_dock.header_timeline_gap), zoom.as_mut().map(|zoom| zoom as &mut qnc_source_timeline::Instead));
    let (intent, zoomed) = qnc_source_timeline::show_with(ui, rect, dock, label, header, (&view.preview.timeline, &view.preview.assets), &view.source_timeline, instead);
    // A click on A1/A2 and the wheel are the source timeline's own catalog actions.
    match intent {
        TimelineIntent::None => zoomed.map(EditorialIntent::Action).or(clicked.map(EditorialIntent::action)),
        intent => qnc_source_timeline::SourceTimeline::event_of(&intent).map(EditorialIntent::Action).or(clicked.map(EditorialIntent::action)),
    }
}

fn pool(ui: &mut Ui, contracts: &EditorialContracts, theme: &Theme, view: &EditorialView) -> Option<EditorialIntent> {
    if view.library_tab == LibraryTab::Segment {
        let rect = qnc_ui_kit::fill_block(ui, theme.bg).shrink(contracts.editorial.board.block_pad);
        let shown = ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| qnc_segment_panel::show_segment_list(ui, &view.segments, qnc_theme_styles::timeline(theme), theme.danger));
        return shown.inner.map(EditorialIntent::Action);
    }
    let cards = &contracts.editorial.media_card;
    let composition = &contracts.composition().media_card;
    let spec = qnc_media_pool_panel::PoolSpec {
        pad: contracts.editorial.board.block_pad,
        metrics: CardMetrics { min_card_width: cards.min_card_width, card_text_height: cards.card_text_height, grid_gap: cards.grid_gap },
        ticks: if composition.ticks_several { Ticks::Selected } else { Ticks::chosen_if(composition.selection_check) },
        status_dots: StatusDotsMode::from_contract(&composition.status_dots).unwrap_or(StatusDotsMode::Off),
        imported_marker: None,
        saving: "",
        save_failed: "",
    };
    // Virtual lists the shorts, B-roll the shots of the covers (v5 pool tabs).
    let shot_tab = view.library_tab.lists_shots();
    let now = qnc_media_pool_panel::PoolNow {
        chosen_id: if shot_tab { view.chosen_shot_id.as_deref().unwrap_or("") } else { view.chosen_clip_id().unwrap_or("") },
        panel_focused: view.focus.panel == Panel::Pool,
        step: view.focus.pool_step,
        empty_message: cards.empty_message(view.library_tab, view.loading, &view.message),
        id_salt: "editorial_clip_grid",
    };
    let clicked = if shot_tab {
        let shots: Vec<_> = view.shorts.iter().filter(|shot| shot.b_roll == (view.library_tab == LibraryTab::Broll)).collect();
        qnc_media_pool_panel::show(ui, theme, &spec, &now, &shots)
    } else {
        qnc_media_pool_panel::show(ui, theme, &spec, &now, &view.focus.transcript.pool_clips(view.library_tab == LibraryTab::Transcript, &view.clips))
    };
    Some(match clicked? { CardGridAction::ToggleSelection(id) if composition.ticks_several && !shot_tab => EditorialIntent::Action(qnc_action_event::ActionEvent::key(qnc_transcript::MARK).with_target(id)), CardGridAction::Activate(id) | CardGridAction::ToggleSelection(id) => if shot_tab { EditorialIntent::PreviewShort(id) } else { EditorialIntent::PreviewClip(id) } })
}
