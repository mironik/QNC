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
use qnc_edit_verbs::{Lane, Side, Verb};
use qnc_timeline::{ChannelChoice, TimelineTheme};
use qnc_program_view::{ClipEdge, EditStill, SegmentsView, ZoomCutSound, ZoomSound};

pub const MODULE_ID: &str = "qnc.module.trim-overlay";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The side of the taken cut a picture shows, picked by a click (AGENTS 0.12: every event of
/// the zoom is a verb of `qnc-edit-verbs`, the same as the timeline's).
pub const PICK_SIDE: &str = qnc_edit_verbs::PICK_SIDE;

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
/// The sides of the taken cut's sound on A2 (user 2026-10-29), as red as on the Wrap row; a
/// click within `SIDE_REACH` pixels takes a side.
const SIDE_REACH: f32 = 40.0;
/// A lane's label button in the zoom.
const LABEL_SIZE: Vec2 = Vec2::new(30.0, 20.0);
/// The row of Undo, Save and Cancel under the pictures, centred on the cut (user 2026-10-30:
/// "so placing the playhead and saving are not confused"); each is its key's action.
const BUTTONS_HEIGHT: f32 = 30.0;
const BUTTON_SIZE: Vec2 = Vec2::new(78.0, 24.0);
/// Save keeps what is taken (user 2026-11-04: "after Save do not close"); Close is Escape.
const BUTTONS: [(&str, &str); 3] = [("story.ui.undo", qnc_edit_verbs::UNDO), ("common.save", qnc_edit_verbs::SAVE), ("zoom.close", qnc_edit_verbs::DROP)];
/// The share of the screen width the window may take.
const SCREEN_SHARE: f32 = 0.92;

/// Shows the trim zoom over everything while `segments` has edit pictures (the Wrap
/// rows' or, for a segment edge, the source timeline's).
/// The wheel and the touchpad over the window are the wheel of the panel the taken thing is
/// on (user 2026-10-26: "in the zoom only the arrows work"), its system action.
pub fn show(ctx: &egui::Context, segments: &SegmentsView, theme: &TimelineTheme) -> Option<ActionEvent> {
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
    let size = Vec2::new(count * picture.x + GAP * (count - 1.0), picture.y + LABEL_HEIGHT + BUTTONS_HEIGHT + lanes) + Vec2::splat(2.0 * MARGIN);
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
            let (mut gap_at, mut gap_at_cut) = (None, false);
            // The side in focus (user 2026-11-04): its pictures framed red, the same in every zoom.
            let focus = sound.and_then(|sound| sound.focus_side);
            let taken_cut = sound.and_then(|sound| sound.taken_cut.clone());
            for (index, still) in stills.iter().enumerate() {
                let left = top.x + index as f32 * (picture.x + GAP);
                let pane = Rect::from_min_size(egui::pos2(left, top.y), picture);
                let in_focus = focus.map_or(still.cut, |side| still.after == side);
                paint_still(ui, pane, still, in_focus, &visuals, theme);
                // A left click on a picture or on its strip picks its side of the cut and never
                // closes the zoom (user 2026-11-05: "a left click on the picture closed the zoom, it
                // must mark what is chosen"): the owner's own pick when it gives one (a
                // SubSegment's slot), else the one system action of every zoom.
                let pick = still.strip.as_ref().and_then(|strip| strip.select.as_ref()).or(still.click.as_ref()).map(|action| event_of(action, panel)).or_else(|| {
                    taken_cut.clone().map(|cut| Verb::PickSide { cut, side: Side::of(still.after) }.event(panel))
                });
                // The slot's length strip at the bottom of the picture (user 2026-11-05), as on the
                // timeline: a right click on it takes the slot, a right click on the picture
                // outside it the marker of the cut (a right click is Ctrl+, everywhere).
                let strip_rect = still.strip.as_ref().map(|_| Rect::from_min_max(egui::pos2(pane.left(), pane.bottom() - STRIP_HEIGHT), pane.max));
                let response = ui.interact(pane, ui.id().with(("still", index)), Sense::click());
                let at = response.interact_pointer_pos();
                let on_strip = strip_rect.zip(at).is_some_and(|(strip, at)| strip.contains(at));
                if let (Some(strip), Some(rect)) = (still.strip.as_ref(), strip_rect) {
                    paint_strip(ui, rect, strip, in_focus, ui.rect_contains_pointer(rect), theme);
                }
                if response.clicked() {
                    if let Some(pick) = pick {
                        turned = Some(pick);
                    }
                } else if response.secondary_clicked() {
                    let right = if on_strip { still.strip.as_ref().map(|strip| &strip.take) } else { still.right_click.as_ref() };
                    if let Some(action) = right {
                        turned = Some(event_of(action, panel));
                    }
                }
                // The marker lies between the picture before it and the one after it.
                if still.after && index > 0 && !stills[index - 1].after {
                    let x = left - GAP / 2.0;
                    // The cut's line goes through the gap at the taken cut (four pictures: two gaps).
                    if !gap_at_cut && (still.cut || stills[index - 1].cut) {
                        (gap_at, gap_at_cut) = (Some(x), true);
                    }
                    gap_at.get_or_insert(x);
                    let accent = visuals.selection.bg_fill;
                    ui.painter().line_segment([egui::pos2(x, pane.top() - 4.0), egui::pos2(x, pane.bottom() + 4.0)], Stroke::new(3.0, accent));
                }
            }
            // V over the pictures while a cut is taken (user 2026-10-30: J and L of the picture
            // too): a click takes only the picture of the cut, as Ctrl+I there.
            if let Some((sound, cut)) = sound.and_then(|sound| sound.taken_cut.as_deref().map(|cut| (sound, cut))) {
                if let Some(clicked) = paint_take_button(ui, top, "V", Verb::TakePicture { cut: cut.to_string() }.event(panel), sound.picture_taken, &visuals, theme) {
                    turned = Some(clicked);
                }
            }
            if let Some(sound) = sound {
                let area = Rect::from_min_size(egui::pos2(top.x, top.y + picture.y + LABEL_HEIGHT + BUTTONS_HEIGHT + LANES_GAP), Vec2::new(row_width, 2.0 * LANE_HEIGHT + GAP / 2.0 + WORDS_HEIGHT));
                // The side the next action is on, plainly (user 2026-11-05: "the A1 of the clip
                // before or of the next one, so the user sees what the action is on"): its half
                // of the sound under a faint red, framed red as its pictures.
                if let Some(after) = focus {
                    let width = (sound.to - sound.from).max(1) as f32;
                    let x = area.left() + area.width() * (sound.cut.clamp(sound.from, sound.to) - sound.from) as f32 / width;
                    let half = if after { Rect::from_min_max(egui::pos2(x, area.top()), area.max) } else { Rect::from_min_max(area.min, egui::pos2(x, area.bottom())) };
                    if half.width() > 1.0 {
                        ui.painter().rect_filled(half, 0.0, theme.cut.gamma_multiply(28.0 / 255.0));
                        ui.painter().rect_stroke(half, 0.0, Stroke::new(2.0, theme.cut), StrokeKind::Inside);
                    }
                }
                // The level curves of the zoom's owner on its axis, as its timeline draws them
                // (user 2026-11-07: the zoom is the same timeline).
                let curves = timeline_level_curves(&sound.level_curves, sound.from, sound.to, 0);
                let (word, side) = paint_sound(ui, area, sound, gap_at, top.y, (panel, source), (lane_choices(sound, segments), &curves), &visuals, theme);
                // A word in every zoom (user 2026-11-04): the taken cut goes to it, else a cut.
                if let Some((from, to)) = word {
                    turned = Some(Verb::Word { from, to }.event(panel));
                }
                if side.is_some() {
                    turned = side;
                }
            }
            // The buttons stand in the middle of the window (user 2026-11-04: "the buttons ran
            // away, not centred" when the cut lay at the edge of the window).
            let row = Rect::from_min_size(egui::pos2(top.x, top.y + picture.y + LABEL_HEIGHT), Vec2::new(row_width, BUTTONS_HEIGHT));
            if let Some(pressed) = paint_buttons(ui, row, row.center().x) {
                turned = Some(pressed);
            }
        });
    turned
}

/// Undo, Save and Close in one row in the middle of the window; a press is the system action of
/// Backspace, of saving what is taken (it stays taken) or of Escape on the panel in focus.
fn paint_buttons(ui: &mut egui::Ui, row: Rect, centre: f32) -> Option<ActionEvent> {
    let width = BUTTONS.len() as f32 * BUTTON_SIZE.x + (BUTTONS.len() as f32 - 1.0) * GAP;
    let left = (centre - width / 2.0).clamp(row.left(), (row.right() - width).max(row.left()));
    let mut pressed = None;
    for (index, (text, action)) in BUTTONS.iter().enumerate() {
        let at = egui::pos2(left + index as f32 * (BUTTON_SIZE.x + GAP), row.center().y - BUTTON_SIZE.y / 2.0);
        let button = qnc_ui_kit::sized_button(qnc_ui_text::text(text), BUTTON_SIZE);
        if ui.put(Rect::from_min_size(at, BUTTON_SIZE), button).clicked() {
            pressed = Some(ActionEvent::key(*action));
        }
    }
    pressed
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
/// Where the thin line of the cut (the playhead of the zoom) crosses the lanes.
fn cut_line_x(area: Rect, sound: &ZoomSound) -> f32 {
    area.left() + area.width() * (sound.cut.saturating_sub(sound.from)) as f32 / (sound.to - sound.from) as f32
}

#[allow(clippy::too_many_arguments)]
fn paint_sound(ui: &mut egui::Ui, area: Rect, sound: &ZoomSound, gap_at: Option<f32>, pictures_top: f32, (panel, source): (&str, bool), (choices, curves): ([Option<ChannelChoice>; 2], &[qnc_timeline::TimelineLevelCurve]), visuals: &egui::Visuals, theme: &TimelineTheme) -> (Option<(u64, u64)>, Option<ActionEvent>) {
    let mut word = None;
    let mut side = None;
    let span = (sound.to - sound.from) as f32;
    let cut_x = cut_line_x(area, sound);
    let x_of = |frame: u64| area.left() + area.width() * (frame.saturating_sub(sound.from)) as f32 / span;
    for lane in 0..2 {
        let top = area.top() + lane as f32 * (LANE_HEIGHT + GAP / 2.0 + WORDS_HEIGHT);
        if lane == 1 {
            word = paint_words(ui, Rect::from_min_size(egui::pos2(area.left(), top - WORDS_HEIGHT - GAP / 4.0), Vec2::new(area.width(), WORDS_HEIGHT)), sound, &x_of, visuals, theme);
        }
        let band = Rect::from_min_size(egui::pos2(area.left(), top), Vec2::new(area.width(), LANE_HEIGHT));
        ui.painter().rect_filled(band, 2.0, theme.audio_primary_background);
        let peaks = &sound.peaks[lane];
        let middle = band.center().y;
        let half = LANE_HEIGHT / 2.0 - 2.0;
        if !peaks.is_empty() {
            let step = band.width() / peaks.len() as f32;
            for (column, peak) in peaks.iter().enumerate() {
                let height = shown_height(*peak) * half;
                let x = band.left() + (column as f32 + 0.5) * step;
                ui.painter().line_segment([egui::pos2(x, middle - height), egui::pos2(x, middle + height.max(0.5))], Stroke::new(step.max(1.0), theme.audio_wave));
            }
        }
        // The level curves of this lane, the timeline's drawing; the controlled point is on the
        // zoom's line (the playhead of the taken curve). A right click near a line takes it, as on
        // the timeline (Ctrl+Shift+1/2).
        let window = (sound.from, sound.to.saturating_sub(sound.from).max(1));
        qnc_timeline::paint_level_curves(ui.painter(), band, window, lane as u8, curves, Some(sound.cut), [theme.level_curve, theme.lane_taken, theme.cut]);
        let curve_clicked = ui
            .input(|i| i.pointer.secondary_clicked().then(|| i.pointer.interact_pos()).flatten())
            .filter(|pos| band.contains(*pos))
            .and_then(|pos| qnc_timeline::level_curve_hit(band, window, lane as u8, pos, curves));
        if let Some(frame) = curve_clicked {
            let action = if lane == 0 { "select_level_a1" } else { "select_level_a2" };
            side = Some(ActionEvent::key(action).with_value(frame).at(panel));
        }
        // The lane's tools are the timeline's (AGENTS 0.12, docs/97 step 3): its label button
        // (red when off, a yellow rim while taken), its level, and while taken its channel
        // column. The label is made after the band's sides so that its click is its own.
        let taken_lane = sound.cut_sound.as_ref().filter(|cut| usize::from(cut.lane) == lane).is_some();
        if let Some(cut_sound) = sound.cut_sound.as_ref().filter(|cut| usize::from(cut.lane) == lane) {
            if let Some(event) = paint_cut_sound(ui, band, sound, cut_sound, &x_of, lane == 0, panel, theme) {
                side = Some(event);
            }
        }
        if let Some(clicked) = paint_lane_tools(ui, (area, band), lane, (sound, choices[lane], taken_lane), (panel, source), visuals, theme) {
            side = Some(clicked);
        }
        // A right click on the lane's sound is Ctrl+ (user 2026-11-03: "a right click on the
        // sound in the zoom should do the same"): it takes the cut's sound on that lane, as the
        // lane's label does.
        if let Some(cut) = sound.taken_cut.as_deref().filter(|_| curve_clicked.is_none()) {
            let right = ui.input(|i| i.pointer.secondary_clicked() && i.pointer.interact_pos().is_some_and(|pos| band.contains(pos)));
            if right {
                side = Some(Verb::TakeSound { cut: cut.to_string(), lane: lane_of(lane) }.event(panel));
            }
        }
    }
    // One thin line from between the pictures down through both waves when the cut is there.
    let from_top = match gap_at {
        Some(x) if (x - cut_x).abs() < 2.0 => pictures_top - 4.0,
        _ => area.top(),
    };
    // While the player plays the window follows it (user 2026-10-30): the cut line only
    // while the cut is in the window, and the playhead, white, through both waves.
    if (sound.from..=sound.to).contains(&sound.cut) {
        ui.painter().line_segment([egui::pos2(cut_x, from_top), egui::pos2(cut_x, area.bottom())], Stroke::new(1.5, theme.cut));
    }
    if let Some(playhead) = sound.playhead.filter(|frame| (sound.from..=sound.to).contains(frame)) {
        let x = x_of(playhead);
        ui.painter().line_segment([egui::pos2(x, area.top()), egui::pos2(x, area.bottom())], Stroke::new(1.5, visuals.strong_text_color()));
    }
    (word, side)
}

/// The tools of a lane, the timeline's own (`qnc-timeline` lane tools, AGENTS 0.12): the
/// label button, the level heard and, while the lane is taken, the channel column. A click on
/// the label takes the taken cut's sound on that lane (as Ctrl+1/Ctrl+2 with a cut taken), else
/// the lane as the timeline's label does (on a program's row the sound under the playhead).
fn paint_lane_tools(
    ui: &mut egui::Ui,
    (area, band): (Rect, Rect),
    lane: usize,
    (sound, choice, cut_sound_taken): (&ZoomSound, Option<ChannelChoice>, bool),
    (panel, source): (&str, bool),
    visuals: &egui::Visuals,
    theme: &TimelineTheme,
) -> Option<ActionEvent> {
    let text = if lane == 0 { "A1" } else { "A2" };
    let look = qnc_timeline::LaneLook { label_background: visuals.window_fill, silent: theme.cut, taken: theme.lane_taken, text: visuals.text_color(), accent: theme.cut_side };
    let label = Rect::from_min_size(band.left_top() + Vec2::new(2.0, 2.0), LABEL_SIZE);
    // The sound of the taken cut on this lane shows as a taken lane.
    let shown = choice.or(cut_sound_taken.then_some(ChannelChoice { selected: 0, count: 0, open: true, off: false, level_db: 0, heard_db: 0 }));
    qnc_timeline::paint_lane_button_of(ui, label, text, shown, look);
    let track = Rect::from_min_max(egui::pos2(label.right() + 4.0, band.top()), band.max);
    qnc_timeline::paint_lane_heard(ui, track, choice, look);
    let mut asked = None;
    // The channel column while the lane is taken: down from A1, up from A2, inside the sound.
    if let Some(choice) = choice.filter(|choice| choice.open && choice.count > 0) {
        let room = if lane == 0 { (band.bottom(), area.bottom()) } else { (area.top(), band.top()) };
        let column = Rect::from_min_size(egui::pos2(label.left(), if lane == 0 { band.top() } else { band.bottom() - label.height() }), label.size());
        let id = ui.make_persistent_id(("qnc_trim_zoom_channels", lane));
        if let (Some(channel), _) = qnc_timeline::lane_channel_column(ui, id, column, room, lane == 0, choice, look) {
            asked = Some(Verb::PickChannel { lane: lane_of(lane), channel }.event(panel));
        }
    }
    let response = ui.interact(label, ui.id().with(("zoom_lane", lane)), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        asked = Some(match sound.taken_cut.as_deref() {
            Some(cut) => Verb::TakeSound { cut: cut.to_string(), lane: lane_of(lane) },
            // As the timeline's label: on a program's row the sound at the playhead (else the
            // cut), on a source timeline the clip's lane.
            None => Verb::TakeLane { lane: lane_of(lane), frame: (!source).then(|| sound.playhead.unwrap_or(sound.cut)) },
        }
        .event(panel));
    }
    asked
}

/// A1 and A2 of the zoom as the timeline's lanes show them: what its owner gives, else on a
/// program's row the taken segment's or cover's sound (its channel column open).
fn lane_choices(sound: &ZoomSound, segments: &SegmentsView) -> [Option<ChannelChoice>; 2] {
    [0usize, 1].map(|lane| {
        sound
            .lane_choices[lane]
            .map(|shown| ChannelChoice { selected: shown.channel, count: shown.count, open: shown.open, off: shown.off, level_db: shown.level_db, heard_db: shown.level_db })
            .or_else(|| {
                let part = segments.part_sound.as_ref().filter(|part| usize::from(part.lane) == lane)?;
                Some(ChannelChoice { selected: part.channel, count: part.channels, open: true, off: part.off, level_db: part.level_db, heard_db: part.level_db })
            })
    })
}

/// The zoom's lane 0 is A1, lane 1 A2.
fn lane_of(lane: usize) -> Lane {
    if lane == 0 {
        Lane::A1
    } else {
        Lane::A2
    }
}

/// A button of the taken cut at `corner` (A1, A2 or V): a click takes that part of the cut, as
/// its key does; the part taken has a red frame.
fn paint_take_button(ui: &mut egui::Ui, corner: egui::Pos2, text: &str, event: ActionEvent, taken: bool, visuals: &egui::Visuals, theme: &TimelineTheme) -> Option<ActionEvent> {
    let rect = Rect::from_min_size(corner + Vec2::new(4.0, 3.0), Vec2::new(26.0, 18.0));
    let response = ui.interact(rect, ui.id().with(("zoom_take", text)), Sense::click());
    let frame = if taken { theme.cut_side } else if response.hovered() { visuals.strong_text_color() } else { visuals.text_color() };
    ui.painter().rect_filled(rect, 2.0, visuals.window_fill);
    ui.painter().rect_stroke(rect, 2.0, Stroke::new(if taken { 2.0 } else { 1.0 }, frame), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(12.0), visuals.text_color());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked().then_some(event)
}

/// The taken cut's A2 sound on the zoom's A2 (user 2026-10-29: "that option in the zoom too"):
/// where the sound after starts (J) and where the sound before ends (L), red, the moving ones
/// thick, joined at the top of A2. A click near a side takes it, a drag moves it, the same
/// system actions as on the Wrap row.
fn paint_cut_sound(ui: &mut egui::Ui, band: Rect, sound: &ZoomSound, cut_sound: &ZoomCutSound, x_of: &dyn Fn(u64) -> f32, a1: bool, panel: &str, theme: &TimelineTheme) -> Option<ActionEvent> {
    let sides: Vec<u64> = cut_sound.lead.into_iter().chain(cut_sound.tail).filter(|frame| (sound.from..=sound.to).contains(frame)).collect();
    let cut_x = x_of(sound.cut);
    let (mut left, mut right) = (cut_x, cut_x);
    for side in &sides {
        let x = x_of(*side);
        let width = if cut_sound.moving.contains(side) { 3.0 } else { 2.0 };
        ui.painter().line_segment([egui::pos2(x, band.top()), egui::pos2(x, band.bottom())], Stroke::new(width, theme.cut_side));
        (left, right) = (left.min(x), right.max(x));
    }
    // Joined at the picture's edge of the lane: the bottom of A1, the top of A2.
    let edge = if a1 { band.bottom() } else { band.top() };
    ui.painter().line_segment([egui::pos2(left, edge), egui::pos2(right, edge)], Stroke::new(1.0, theme.cut_side));
    let response = ui.interact(band, ui.id().with(("cut_sound", a1)), Sense::click_and_drag());
    let span = (sound.to - sound.from).max(1) as f32;
    let frame_at = |x: f32| sound.from + (((x - band.left()) / band.width()).clamp(0.0, 1.0) * span).round() as u64;
    let near = |x: f32| sides.iter().any(|side| (x_of(*side) - x).abs() <= SIDE_REACH);
    let cut = || cut_sound.marker_id.clone();
    let event = |drag: bool, x: f32| {
        let frame = frame_at(x);
        if drag { Verb::DragSoundSide { cut: cut(), frame } } else { Verb::TakeSoundSide { cut: cut(), frame } }.event(panel)
    };
    if response.hovered() && response.hover_pos().is_some_and(|pos| near(pos.x)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    let drag_key = ui.id().with(("cut_sound_drag", a1));
    if response.drag_started() {
        let started = ui.ctx().input(|i| i.pointer.press_origin()).is_some_and(|pos| near(pos.x));
        ui.ctx().data_mut(|d| d.insert_temp(drag_key, started));
    }
    let dragging = ui.ctx().data(|d| d.get_temp::<bool>(drag_key)).unwrap_or(false);
    if response.dragged() && dragging {
        return response.interact_pointer_pos().map(|pos| event(true, pos.x));
    }
    if response.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<bool>(drag_key));
    }
    response.clicked().then(|| response.interact_pointer_pos()).flatten().filter(|pos| near(pos.x)).map(|pos| event(false, pos.x))
}

/// The words said on A1 in the window, each from where it is said, cut where the next one
/// starts; a short tick marks a word's start, so a pause between words is seen.
/// A double click on a word gives its frames (user 2026-10-26: as on the transcript, a cut).
fn paint_words(ui: &mut egui::Ui, row: Rect, sound: &ZoomSound, x_of: &dyn Fn(u64) -> f32, visuals: &egui::Visuals, theme: &TimelineTheme) -> Option<(u64, u64)> {
    let mut clicked = None;
    let font = FontId::proportional(13.0);
    // Voice no word was heard for (user 2026-10-26): red, so a word the transcript missed is seen.
    for &(from, to) in &sound.unheard {
        let (left, right) = (x_of(from).max(row.left()), x_of(to).min(row.right()));
        if right > left {
            let span = Rect::from_min_max(egui::pos2(left, row.top() + 2.0), egui::pos2(right, row.bottom() - 2.0));
            ui.painter().rect_filled(span, 2.0, theme.unheard);
            ui.painter().rect_stroke(span, 2.0, Stroke::new(1.0, theme.cut), StrokeKind::Inside);
            ui.painter().with_clip_rect(span).text(span.center(), Align2::CENTER_CENTER, "?", font.clone(), theme.cut);
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
        // With its letters laid (user 2026-11-05: letters in the zoom too) every letter stands
        // where it is said, with a tick at its start; a click is on the letter under the mouse.
        let letters: Vec<(char, (f32, f32), (u64, u64))> = word.letters.iter().filter_map(|(letter, span)| span.map(|(from, to)| (*letter, (x_of(from), x_of(to)), (from, to)))).collect();
        let hit = Rect::from_min_max(egui::pos2(left, row.top()), egui::pos2(right.max(left + 4.0), row.bottom()));
        let response = ui.interact(hit, ui.id().with(("qnc_trim_word", index)), Sense::click());
        let under = response.hover_pos().and_then(|at| letters.iter().find(|(_, (a, b), _)| at.x >= *a && at.x < b.max(a + 1.0)).copied());
        if letters.is_empty() {
            ui.painter().with_clip_rect(cell).text(egui::pos2(cell.left(), row.center().y), Align2::LEFT_CENTER, word.text.trim(), font.clone(), visuals.text_color());
        } else {
            let painter = ui.painter().with_clip_rect(Rect::from_min_max(egui::pos2(left, row.top()), egui::pos2(right, row.bottom())));
            for &(letter, (a, b), _) in &letters {
                let lit = under.is_some_and(|(_, (ua, _), _)| ua == a);
                if lit {
                    painter.rect_filled(Rect::from_min_max(egui::pos2(a, row.top() + 2.0), egui::pos2(b.max(a + 2.0), row.bottom() - 2.0)), 1.0, visuals.selection.bg_fill);
                }
                painter.line_segment([egui::pos2(a, row.bottom() - 5.0), egui::pos2(a, row.bottom() - 1.0)], Stroke::new(1.0, visuals.weak_text_color()));
                painter.text(egui::pos2(a + 1.0, row.center().y), Align2::LEFT_CENTER, letter, font.clone(), visuals.text_color());
            }
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(if under.is_some() { egui::CursorIcon::Text } else { egui::CursorIcon::PointingHand });
        }
        // A click is enough (user 2026-11-04: "a click on the text in the zoom puts the wave
        // exactly at the playhead"): the taken edge goes to the word (or the letter), the
        // window follows it.
        if response.clicked() || response.double_clicked() {
            clicked = Some(under.map_or((word.from, word.to.max(word.from + 1)), |(_, _, span)| span));
        }
    }
    clicked
}

/// The height of a slot's length strip at the bottom of a zoom picture.
const STRIP_HEIGHT: f32 = 24.0;

/// A slot's length strip: the colour of its length, its length, brighter when chosen or under
/// the mouse (as a cover's strip on the timeline).
fn paint_strip(ui: &egui::Ui, rect: Rect, strip: &qnc_program_view::StillStrip, focused: bool, hovered: bool, theme: &TimelineTheme) {
    let base = strip.fill.map_or(ui.visuals().selection.bg_fill, qnc_desktop_theme::rgb);
    let fill = if focused || strip.chosen || hovered { base.linear_multiply(0.95) } else { base.linear_multiply(0.6) };
    ui.painter().rect_filled(rect, 1.0, fill);
    // The side the next action is on: framed red as its pictures (user 2026-11-05).
    if focused {
        ui.painter().rect_stroke(rect, 1.0, Stroke::new(3.0, theme.cut), StrokeKind::Inside);
    } else if strip.chosen {
        ui.painter().rect_stroke(rect, 1.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, &strip.label, FontId::proportional(12.0), Color32::BLACK);
}

fn paint_still(ui: &mut egui::Ui, pane: Rect, still: &EditStill, framed: bool, visuals: &egui::Visuals, theme: &TimelineTheme) {
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
        ui.painter().rect_filled(band, 0.0, theme.cut);
    }
    if framed {
        // The focus: the side in focus, else the picture at the cut being moved (user
        // 2026-10-13: a red frame).
        ui.painter().rect_stroke(pane, 0.0, Stroke::new(4.0, theme.cut), StrokeKind::Inside);
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

/// The event of a zoom click: the timeline's action of the same gesture, its target and frame.
fn event_of(action: &qnc_program_view::StillAction, panel: &str) -> ActionEvent {
    let mut event = ActionEvent::key(&action.action).at(panel);
    if let Some(target) = &action.target {
        event = event.with_target(target.clone());
    }
    if let Some(value) = action.value {
        event = event.with_value(value);
    }
    event
}

/// The level curves of a story inside program frames `[from, to)` as a timeline draws them,
/// their frames counted from `origin` (a Wrap row's start, 0 for the zoom's program axis). The
/// one place a story's curve becomes a timeline's (the segment panel and the zoom).
pub fn timeline_level_curves(curves: &[qnc_program_view::LevelCurve], from: u64, to: u64, origin: u64) -> Vec<qnc_timeline::TimelineLevelCurve> {
    curves
        .iter()
        .filter_map(|curve| {
            let (a, b, part) = curve.placed.within(curve.span, from, to)?;
            Some(qnc_timeline::TimelineLevelCurve {
                lane: curve.lane,
                from: a - origin,
                to: b - origin,
                start_db10: part.start_db10,
                end_db10: part.end_db10,
                points: part.points.iter().map(|(at, db10)| (at - origin as f64, *db10)).collect(),
                taken: curve.taken,
            })
        })
        .collect()
}
