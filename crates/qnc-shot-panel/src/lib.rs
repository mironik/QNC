//! The Video AI panel (user 2026-10-27: AI Media Assist, the shot analysis of the clip on the
//! preview): first the usable parts of the take (camera steady or panning evenly, sharp, at
//! least three seconds), each with its start and end timecode and how the camera holds it; a
//! click opens the part with its IN/OUT on the source timeline, so Add SubClip (Shift+S)
//! keeps it. Below them the shots (changes of scene) and their looks; a click puts the playhead
//! on a shot's start. The part or shot the playhead is in is lit. It paints the shots view and
//! gives the clicks as the catalog actions `shot_open_part` and `shot_cue`; it holds no state,
//! reads nothing and knows no form.

use eframe::egui::{self, RichText, Sense};
use qnc_action_event::ActionEvent;
use qnc_clip_shots::{ShotsView, CUE_SHOT, OPEN_PART, PANEL};
use qnc_segment_panel::{cover_fill, CoverLengthBand};
use qnc_source_timecode::SourceTimecode;

pub const MODULE_ID: &str = "qnc.module.shot-panel";

/// Paints the analysis of the shown clip on the whole of `ui`; `timecode` is the shown clip's
/// source timecode (frames are shown without it).
/// A usable part has the frame colour and the length label a cover of its length has in Story
/// (`lengths`: the bands of the layout contract).
pub fn show(ui: &mut egui::Ui, view: &ShotsView, theme: &qnc_desktop_theme::Theme, timecode: Option<&SourceTimecode>, lengths: &[CoverLengthBand]) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let inner = rect.shrink(8.0);
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    ui.set_clip_rect(inner.intersect(ui.clip_rect()));
    let muted = |text: &str| RichText::new(text).color(theme.text_muted).size(theme.font_ui);
    if !view.message.is_empty() {
        ui.label(muted(&view.message));
    }
    let Some(analysis) = view.analysis.as_ref() else {
        ui.label(RichText::new(qnc_ui_text::text("shots.parts_title")).color(theme.text).strong());
        let empty = if view.clip_id.is_some() { "shots.not_analysed" } else { "shots.no_clip" };
        ui.add(egui::Label::new(muted(qnc_ui_text::text(empty))).wrap());
        return None;
    };
    if !analysis.error.is_empty() {
        ui.add(egui::Label::new(RichText::new(&analysis.error).color(theme.danger).size(theme.font_ui)).wrap());
        return None;
    }
    let seconds = analysis.took_ms as f64 / 1000.0;
    let at = |frame: u64| timecode.map_or_else(|| frame.to_string(), |tc| tc.label(frame));
    let mut asked = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut ui, |ui| {
        ui.label(RichText::new(qnc_ui_text::text("shots.parts_title")).color(theme.text).strong());
        if analysis.parts.is_empty() {
            ui.add(egui::Label::new(muted(qnc_ui_text::text("shots.no_parts"))).wrap());
        } else {
            ui.add(egui::Label::new(muted(qnc_ui_text::text("shots.parts_hint"))).wrap());
        }
        ui.add_space(4.0);
        let lit = view.part_at_playhead();
        for (index, part) in analysis.parts.iter().enumerate() {
            let camera = match part.camera.as_str() {
                "steady" => qnc_ui_text::text("shots.camera.steady"),
                "pan" => qnc_ui_text::text("shots.camera.pan"),
                other => other,
            };
            let frames = part.to.saturating_sub(part.from);
            let fps = timecode.map(|tc| tc.fps).filter(|fps| *fps > 0);
            let length = fps.map(|fps| qnc_program_view::duration_label(frames, fps, 1));
            let band = fps.and_then(|fps| cover_fill(frames, Some((fps, 1)), lengths));
            let what = length.map_or_else(|| camera.to_string(), |length| format!("{length}  {camera}"));
            if row(ui, theme, (index, lit, band), &format!("{} – {}", at(part.from), at(part.to)), &what) {
                asked = Some(ActionEvent::key(OPEN_PART).with_target(index.to_string()).at(PANEL));
            }
        }
        ui.add_space(10.0);
        ui.label(RichText::new(qnc_ui_text::text("shots.title")).color(theme.text).strong());
        let lit = view.shot_at_playhead();
        for (index, shot) in analysis.shots.iter().enumerate() {
            let looks: Vec<&str> = shot.looks.iter().map(|look| look_text(look)).collect();
            if row(ui, theme, (index, lit, None), &format!("{} – {}", at(shot.from), at(shot.to)), &looks.join(", ")) {
                asked = Some(ActionEvent::key(CUE_SHOT).with_target(shot.from.to_string()).at(PANEL));
            }
        }
        ui.add_space(6.0);
        let summary = qnc_ui_text::fill(
            "shots.summary",
            &[
                ("parts", &analysis.parts.len().to_string()),
                ("shots", &analysis.shots.len().to_string()),
                ("pictures", &analysis.pictures.to_string()),
                ("seconds", &format!("{seconds:.1}")),
            ],
        );
        ui.label(muted(&summary));
    });
    asked
}

/// One clickable row: the time span and what it is, framed in `band` (the colour of its length)
/// when given; true when clicked.
fn row(ui: &mut egui::Ui, theme: &qnc_desktop_theme::Theme, (index, lit, band): (usize, Option<usize>, Option<egui::Color32>), span: &str, what: &str) -> bool {
    let fill = if lit == Some(index) { theme.focus.gamma_multiply(0.35) } else if index % 2 == 0 { theme.surface_alt } else { egui::Color32::TRANSPARENT };
    let stroke = band.map_or(egui::Stroke::NONE, |colour| egui::Stroke::new(2.0, colour));
    let frame = egui::Frame::new().fill(fill).stroke(stroke).inner_margin(egui::Margin::symmetric(4, 3)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(span).color(theme.text).size(theme.font_ui));
            if !what.is_empty() {
                ui.label(RichText::new(what).color(theme.text_muted).size(theme.font_ui));
            }
        });
    });
    let response = ui.interact(frame.response.rect, ui.id().with(("row", span, index)), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

fn look_text(look: &str) -> &str {
    match look {
        "black" => qnc_ui_text::text("shots.look.black"),
        "soft" => qnc_ui_text::text("shots.look.soft"),
        "moving" => qnc_ui_text::text("shots.look.moving"),
        "still" => qnc_ui_text::text("shots.look.still"),
        other => other,
    }
}
