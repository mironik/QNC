//! The trim zoom (user 2026-10-10: the timeline has no room for four pictures; "a window
//! in the centre of the screen, a kind of quasi zoom", opened by the Ctrl keys that
//! already take something). While a slot (Ctrl+S), a marker (Ctrl+M) or a segment edge
//! (Ctrl+I/O) is taken, a window in the centre of the screen shows its edit pictures
//! large, in one row, each named, with a line where the marker is; it closes when the
//! taken thing is let go (Enter, Escape). Passive paint only: the pictures and their
//! names come from the program view, this piece reads, decodes and decides nothing.
//! Under the pictures (user 2026-10-25) the sound around the cut, A1 and A2, read finely
//! from the media, and a thin line through the cut: between the two pictures and
//! across both waves.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use qnc_action_event::ActionEvent;
use qnc_program_view::{ClipEdge, EditStill, SegmentsView, ZoomCutSound, ZoomSound};

pub const MODULE_ID: &str = "qnc.module.trim-overlay";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The catalog action of a cut at a word (contracts/qnc-keyboard-shortcuts.json).
const CUT_WORD: &str = "transcript_cut_word";

/// The largest picture (16:9) and the gaps of the window.
const MAX_PICTURE: Vec2 = Vec2::new(480.0, 270.0);
const GAP: f32 = 10.0;
const MARGIN: f32 = 14.0;
const LABEL_HEIGHT: f32 = 22.0;
/// One lane of the zoom's sound and the gap above the lanes.
const LANE_HEIGHT: f32 = 64.0;
const LANES_GAP: f32 = 8.0;
/// The row of the words said on A1, under its wave (user 2026-10-26).
const WORDS_HEIGHT: f32 = 22.0;
/// The thin line through the cut.
const CUT_LINE: Color32 = Color32::from_rgb(230, 30, 30);
/// The sides of the taken cut's sound on A2 (user 2026-10-29), as red as on the Wrap row; a
/// click within `SIDE_REACH` pixels takes a side.
const SIDE_LINE: Color32 = Color32::from_rgb(255, 64, 64);
const SIDE_REACH: f32 = 40.0;
/// The system actions of a side of the cut's sound (contracts/qnc-keyboard-shortcuts.json).
const TAKE_SIDE: &str = "timeline_take_cut_edge";
const DRAG_SIDE: &str = "timeline_drag_cut_edge";
/// The voice no word was heard for: the cut line's red, faint.
const UNHEARD_FILL: Color32 = Color32::from_rgba_premultiplied(70, 10, 10, 70);
/// The share of the screen width the window may take.
const SCREEN_SHARE: f32 = 0.92;

/// Shows the trim zoom over everything while `segments` has edit pictures (the Wrap
/// rows' or, for a segment edge, the source timeline's).
/// The wheel and the touchpad over the window are the wheel of the panel the taken thing is
/// on (user 2026-10-26: "in the zoom only the arrows work"), its system action.
pub fn show(ctx: &egui::Context, segments: &SegmentsView) -> Option<ActionEvent> {
    let source = !segments.source_edit_stills.is_empty();
    let stills = if source { &segments.source_edit_stills } else { &segments.edit_stills };
    if stills.is_empty() {
        return None;
    }
    let mut turned = None;
    let count = stills.len() as f32;
    let screen = ctx.screen_rect();
    let width = ((screen.width() * SCREEN_SHARE - 2.0 * MARGIN - GAP * (count - 1.0)) / count).min(MAX_PICTURE.x);
    let picture = Vec2::new(width.max(16.0), width.max(16.0) * 9.0 / 16.0);
    let sound = segments.zoom_sound.as_ref().filter(|sound| sound.to > sound.from);
    let lanes = if sound.is_some() { LANES_GAP + 2.0 * LANE_HEIGHT + GAP / 2.0 + WORDS_HEIGHT } else { 0.0 };
    let size = Vec2::new(count * picture.x + GAP * (count - 1.0), picture.y + LABEL_HEIGHT + lanes) + Vec2::splat(2.0 * MARGIN);
    egui::Area::new(egui::Id::new("qnc_trim_overlay"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let wheel = qnc_wheel_jog::take(ui, rect, ui.id().with("wheel")).or_else(|| qnc_touchpad::take(ui, rect, ui.id().with("touchpad")));
            let (actions, panel) = if source { (qnc_wheel_jog::SOURCE_ACTIONS, "source_timeline") } else { (qnc_wheel_jog::SEGMENTS_ACTIONS, "segments") };
            turned = wheel.map(|wheel| ActionEvent::key(wheel.action(actions)).at(panel));
            let visuals = ui.visuals().clone();
            ui.painter().rect_filled(rect, 6.0, visuals.window_fill);
            ui.painter().rect_stroke(rect, 6.0, visuals.window_stroke, StrokeKind::Inside);
            let top = rect.min + Vec2::splat(MARGIN);
            let row_width = size.x - 2.0 * MARGIN;
            let mut gap_at = None;
            for (index, still) in stills.iter().enumerate() {
                let left = top.x + index as f32 * (picture.x + GAP);
                let pane = Rect::from_min_size(egui::pos2(left, top.y), picture);
                paint_still(ui, pane, still, &visuals);
                // A picture that switches what is edited (a SubSegment's zoom) is clicked.
                if let Some((action, target)) = &still.click {
                    let response = ui.interact(pane, ui.id().with(("still", index)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if response.clicked() {
                        turned = Some(ActionEvent::key(action).with_target(target.clone()));
                    }
                }
                // The marker lies between the picture before it and the one after it.
                if still.after && index > 0 && !stills[index - 1].after {
                    let x = left - GAP / 2.0;
                    gap_at.get_or_insert(x);
                    let accent = visuals.selection.bg_fill;
                    ui.painter().line_segment([egui::pos2(x, pane.top() - 4.0), egui::pos2(x, pane.bottom() + 4.0)], Stroke::new(3.0, accent));
                }
            }
            if let Some(sound) = sound {
                let area = Rect::from_min_size(egui::pos2(top.x, top.y + picture.y + LABEL_HEIGHT + LANES_GAP), Vec2::new(row_width, 2.0 * LANE_HEIGHT + GAP / 2.0 + WORDS_HEIGHT));
                let (word, side) = paint_sound(ui, area, sound, gap_at, top.y, &visuals);
                // A double clicked word on a source timeline's zoom is a cut, as on the transcript.
                if let Some((from, to)) = word.filter(|_| source) {
                    turned = Some(ActionEvent::key(CUT_WORD).with_value(from).with_target(to.to_string()).at("source_timeline"));
                }
                if side.is_some() {
                    turned = side;
                }
            }
        });
    turned
}

/// How high a peak (per mille of full scale) is drawn, 0 to 1: on a dB scale from
/// -54 dBFS, so quiet speech (-40 LUFS on a camera channel) and the pause after a word
/// differ at a glance, as an editor's audio zoom shows them.
fn shown_height(peak: u16) -> f32 {
    if peak == 0 {
        return 0.0;
    }
    let dbfs = 20.0 * (f32::from(peak) / 1000.0).log10();
    ((dbfs + 54.0) / 54.0).clamp(0.0, 1.0)
}

/// A1 and A2 around the cut, each a lane of its peaks mirrored about its middle, and the
/// thin line of the cut: from between the pictures when it lies under that gap, across
/// both lanes at the cut's place in the window.
fn paint_sound(ui: &mut egui::Ui, area: Rect, sound: &ZoomSound, gap_at: Option<f32>, pictures_top: f32, visuals: &egui::Visuals) -> (Option<(u64, u64)>, Option<ActionEvent>) {
    let mut word = None;
    let mut side = None;
    let span = (sound.to - sound.from) as f32;
    let cut_x = area.left() + area.width() * (sound.cut.saturating_sub(sound.from)) as f32 / span;
    let x_of = |frame: u64| area.left() + area.width() * (frame.saturating_sub(sound.from)) as f32 / span;
    for lane in 0..2 {
        let top = area.top() + lane as f32 * (LANE_HEIGHT + GAP / 2.0 + WORDS_HEIGHT);
        if lane == 1 {
            word = paint_words(ui, Rect::from_min_size(egui::pos2(area.left(), top - WORDS_HEIGHT - GAP / 4.0), Vec2::new(area.width(), WORDS_HEIGHT)), sound, &x_of, visuals);
        }
        let band = Rect::from_min_size(egui::pos2(area.left(), top), Vec2::new(area.width(), LANE_HEIGHT));
        ui.painter().rect_filled(band, 2.0, Color32::from_rgb(17, 103, 88));
        let peaks = &sound.peaks[lane];
        let middle = band.center().y;
        let half = LANE_HEIGHT / 2.0 - 2.0;
        if !peaks.is_empty() {
            let step = band.width() / peaks.len() as f32;
            for (column, peak) in peaks.iter().enumerate() {
                let height = shown_height(*peak) * half;
                let x = band.left() + (column as f32 + 0.5) * step;
                ui.painter().line_segment([egui::pos2(x, middle - height), egui::pos2(x, middle + height.max(0.5))], Stroke::new(step.max(1.0), Color32::from_rgb(80, 190, 148)));
            }
        }
        ui.painter().text(band.left_top() + Vec2::new(6.0, 4.0), Align2::LEFT_TOP, if lane == 0 { "A1" } else { "A2" }, FontId::proportional(12.0), visuals.text_color());
        if lane == 1 {
            if let Some(cut_sound) = &sound.cut_sound {
                side = paint_cut_sound(ui, band, sound, cut_sound, &x_of);
            }
        }
    }
    // One thin line from between the pictures down through both waves when the cut is there.
    let from_top = match gap_at {
        Some(x) if (x - cut_x).abs() < 2.0 => pictures_top - 4.0,
        _ => area.top(),
    };
    ui.painter().line_segment([egui::pos2(cut_x, from_top), egui::pos2(cut_x, area.bottom())], Stroke::new(1.5, CUT_LINE));
    (word, side)
}

/// The taken cut's A2 sound on the zoom's A2 (user 2026-10-29: "that option in the zoom too"):
/// where the sound after starts (J) and where the sound before ends (L), red, the moving ones
/// thick, joined at the top of A2. A click near a side takes it, a drag moves it, the same
/// system actions as on the Wrap row.
fn paint_cut_sound(ui: &mut egui::Ui, band: Rect, sound: &ZoomSound, cut_sound: &ZoomCutSound, x_of: &dyn Fn(u64) -> f32) -> Option<ActionEvent> {
    let sides: Vec<u64> = cut_sound.lead.into_iter().chain(cut_sound.tail).filter(|frame| (sound.from..=sound.to).contains(frame)).collect();
    let cut_x = x_of(sound.cut);
    let (mut left, mut right) = (cut_x, cut_x);
    for side in &sides {
        let x = x_of(*side);
        let width = if cut_sound.moving.contains(side) { 3.0 } else { 2.0 };
        ui.painter().line_segment([egui::pos2(x, band.top()), egui::pos2(x, band.bottom())], Stroke::new(width, SIDE_LINE));
        (left, right) = (left.min(x), right.max(x));
    }
    ui.painter().line_segment([egui::pos2(left, band.top()), egui::pos2(right, band.top())], Stroke::new(1.0, SIDE_LINE));
    let response = ui.interact(band, ui.id().with("cut_sound"), Sense::click_and_drag());
    let span = (sound.to - sound.from).max(1) as f32;
    let frame_at = |x: f32| sound.from + (((x - band.left()) / band.width()).clamp(0.0, 1.0) * span).round() as u64;
    let near = |x: f32| sides.iter().any(|side| (x_of(*side) - x).abs() <= SIDE_REACH);
    let event = |action: &str, x: f32| ActionEvent::key(action).with_target(cut_sound.marker_id.clone()).with_value(frame_at(x)).at("segments");
    if response.hovered() && response.hover_pos().is_some_and(|pos| near(pos.x)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    let drag_key = ui.id().with("cut_sound_drag");
    if response.drag_started() {
        let started = ui.ctx().input(|i| i.pointer.press_origin()).is_some_and(|pos| near(pos.x));
        ui.ctx().data_mut(|d| d.insert_temp(drag_key, started));
    }
    let dragging = ui.ctx().data(|d| d.get_temp::<bool>(drag_key)).unwrap_or(false);
    if response.dragged() && dragging {
        return response.interact_pointer_pos().map(|pos| event(DRAG_SIDE, pos.x));
    }
    if response.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<bool>(drag_key));
    }
    response.clicked().then(|| response.interact_pointer_pos()).flatten().filter(|pos| near(pos.x)).map(|pos| event(TAKE_SIDE, pos.x))
}

/// The words said on A1 in the window, each from where it is said, cut where the next one
/// starts; a short tick marks a word's start, so a pause between words is seen.
/// A double click on a word gives its frames (user 2026-10-26: as on the transcript, a cut).
fn paint_words(ui: &mut egui::Ui, row: Rect, sound: &ZoomSound, x_of: &dyn Fn(u64) -> f32, visuals: &egui::Visuals) -> Option<(u64, u64)> {
    let mut clicked = None;
    let font = FontId::proportional(13.0);
    // Voice no word was heard for (user 2026-10-26): red, so a word the transcript missed is seen.
    for &(from, to) in &sound.unheard {
        let (left, right) = (x_of(from).max(row.left()), x_of(to).min(row.right()));
        if right > left {
            let span = Rect::from_min_max(egui::pos2(left, row.top() + 2.0), egui::pos2(right, row.bottom() - 2.0));
            ui.painter().rect_filled(span, 2.0, UNHEARD_FILL);
            ui.painter().rect_stroke(span, 2.0, Stroke::new(1.0, CUT_LINE), StrokeKind::Inside);
            ui.painter().with_clip_rect(span).text(span.center(), Align2::CENTER_CENTER, "?", font.clone(), CUT_LINE);
        }
    }
    for (index, word) in sound.words.iter().enumerate() {
        let left = x_of(word.from).max(row.left());
        let right = sound.words.get(index + 1).map_or(row.right(), |next| x_of(next.from)).min(row.right()).max(left);
        let end = x_of(word.to).clamp(left, right);
        let said = Rect::from_min_max(egui::pos2(left, row.top() + 2.0), egui::pos2(end.max(left + 1.0), row.bottom() - 2.0));
        ui.painter().rect_filled(said, 2.0, visuals.faint_bg_color);
        ui.painter().line_segment([egui::pos2(left, row.top()), egui::pos2(left, row.bottom())], Stroke::new(1.0, visuals.weak_text_color()));
        let cell = Rect::from_min_max(egui::pos2(left + 3.0, row.top()), egui::pos2(right, row.bottom()));
        ui.painter().with_clip_rect(cell).text(egui::pos2(cell.left(), row.center().y), Align2::LEFT_CENTER, word.text.trim(), font.clone(), visuals.text_color());
        let hit = Rect::from_min_max(egui::pos2(left, row.top()), egui::pos2(right.max(left + 4.0), row.bottom()));
        let response = ui.interact(hit, ui.id().with(("qnc_trim_word", index)), Sense::click());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.double_clicked() {
            clicked = Some((word.from, word.to.max(word.from + 1)));
        }
    }
    clicked
}

fn paint_still(ui: &mut egui::Ui, pane: Rect, still: &EditStill, visuals: &egui::Visuals) {
    ui.painter().rect_filled(pane, 0.0, Color32::BLACK);
    if let Some(image) = still.image.as_deref().filter(|_| still.picture.is_some()) {
        let _ = qnc_ui_kit::paint_rgba_image(ui, pane, &still.id, image.content_key, image.size, &image.pixels);
    }
    ui.painter().rect_stroke(pane, 0.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), StrokeKind::Inside);
    if let Some(edge) = still.clip_edge {
        // The clip ends here: a red line on that side (user 2026-10-13).
        // A band a sixteenth of the picture wide (at least 14 px), well seen at a glance.
        let width = (pane.width() / 16.0).max(14.0);
        let band = match edge {
            ClipEdge::Start => Rect::from_min_max(pane.min, egui::pos2(pane.left() + width, pane.bottom())),
            ClipEdge::End => Rect::from_min_max(egui::pos2(pane.right() - width, pane.top()), pane.max),
        };
        ui.painter().rect_filled(band, 0.0, Color32::from_rgb(230, 30, 30));
    }
    if still.cut {
        // The picture at the cut being moved (user 2026-10-13: a red frame).
        ui.painter().rect_stroke(pane, 0.0, Stroke::new(4.0, Color32::from_rgb(230, 30, 30)), StrokeKind::Inside);
    }
    ui.painter().text(
        egui::pos2(pane.center().x, pane.bottom() + LABEL_HEIGHT / 2.0 + 2.0),
        Align2::CENTER_CENTER,
        &still.label,
        FontId::proportional(14.0),
        visuals.text_color(),
    );
}

#[cfg(test)]
mod tests {
    use super::shown_height;

    #[test]
    fn peaks_are_drawn_on_a_db_scale() {
        assert_eq!(shown_height(0), 0.0);
        assert_eq!(shown_height(1000), 1.0);
        assert!((shown_height(50) - 0.52).abs() < 0.02, "-26 dBFS about half");
        assert_eq!(shown_height(1), 0.0, "-60 dBFS below the floor");
    }
}
