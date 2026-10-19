//! Passive public monitor / preview surface.
//!
//! Any application embeds this component with a caller-owned surface id and
//! already-confirmed pixels. It does not own a clock, decode media, read a
//! database, or know which application hosts it.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui};

pub const MODULE_ID: &str = "qnc.module.monitor";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The monitor's own switch (catalog actions without a key): Auto leaves the picture to the
/// program output on an external screen while it runs, On keeps drawing it here. Part of
/// the monitor, so every form with a preview monitor has it (user 2026-10-19).
pub const MONITOR_AUTO: &str = "monitor_auto";
pub const MONITOR_ON: &str = "monitor_on";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorPaint {
    Picture,
    Message,
    Empty,
}

#[derive(Clone, Copy)]
pub struct MonitorPicture<'a> {
    pub session_id: &'a str,
    pub generation: u64,
    pub sequence: u64,
    pub size: [usize; 2],
    pub rgba: &'a [u8],
    /// Where the player writes these pictures: the program output on an external
    /// screen reads the same map at its own refresh.
    pub frame_map: Option<&'a std::sync::Arc<std::path::Path>>,
}

#[derive(Clone, Copy)]
pub struct MonitorChrome {
    pub fill: Color32,
    pub border: Color32,
    pub muted: Color32,
    pub font_size: f32,
}

#[derive(Clone, Copy)]
pub struct MonitorSurface<'a> {
    pub id: egui::Id,
    pub chrome: MonitorChrome,
    pub picture: Option<MonitorPicture<'a>>,
    pub message: Option<&'a str>,
    /// Monitor Auto (user 2026-10-08): while the program is on an external screen the
    /// picture is left to it, so a weaker laptop does not draw it twice. The monitor draws
    /// its Auto | On switch while that screen is there.
    pub yield_to_external: bool,
}

/// What a monitor that leaves its picture to the external screen shows.

pub fn paint_monitor(ui: &mut Ui, rect: Rect, surface: MonitorSurface<'_>) -> MonitorPaint {
    ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, surface.chrome.fill);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, surface.chrome.border),
        StrokeKind::Inside,
    );

    if let Some(picture) = surface.picture {
        if surface.yield_to_external && qnc_program_output::showing(ui.ctx()) {
            if let Some(frame_map) = picture.frame_map {
                qnc_program_output::offer(ui.ctx(), frame_map);
            }
            paint_placeholder(ui, rect, surface.chrome, qnc_ui_text::text("player.picture_hdmi_output"));
            return MonitorPaint::Message;
        }
        if qnc_ui_kit::paint_stream_frame(
            ui,
            rect.shrink(1.0),
            surface.id,
            (picture.session_id, picture.generation, picture.sequence),
            picture.size,
            picture.rgba,
        ) {
            // The same pictures go to the program output on an external screen, when one is there.
            if let Some(frame_map) = picture.frame_map {
                qnc_program_output::offer(ui.ctx(), frame_map);
            }
            return MonitorPaint::Picture;
        }
    }

    if let Some(message) = surface.message.filter(|text| !text.is_empty()) {
        let galley = ui.painter().layout(
            message.to_string(),
            FontId::proportional(surface.chrome.font_size),
            surface.chrome.muted,
            (rect.width() - 24.0).max(1.0),
        );
        ui.painter().galley(
            rect.center() - galley.size() * 0.5,
            galley,
            surface.chrome.muted,
        );
        return MonitorPaint::Message;
    }

    MonitorPaint::Empty
}

pub fn paint_placeholder(ui: &mut Ui, rect: Rect, chrome: MonitorChrome, label: &str) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(chrome.font_size),
        chrome.muted,
    );
}

/// A still picture of the shown clip (its poster), for when the player has no frame yet.
#[derive(Clone, Copy)]
pub struct MonitorPoster<'a> {
    /// Where the picture comes from; part of the texture identity.
    pub uri: &'a str,
    pub content_key: u64,
    pub size: [usize; 2],
    pub rgba: &'a [u8],
}

/// The preview of any form, in one order: the player frame, else the poster of the shown clip
/// with the player message as a caption, else the message alone, else the label. A message
/// (e.g. the clip's data is still being prepared while the filmstrip is made) never hides the
/// poster (user 2026-10-10). The caller only passes what it knows.
pub fn paint_source_monitor(
    ui: &mut Ui,
    rect: Rect,
    surface: MonitorSurface<'_>,
    poster: Option<MonitorPoster<'_>>,
    label: &str,
) -> Option<&'static str> {
    let (chrome, auto) = (surface.chrome, surface.yield_to_external);
    paint_source_view(ui, rect, surface, poster, label);
    paint_switch(ui, rect, chrome, auto)
}

/// The Auto | On switch in the top right corner, only while the program output is on an
/// external screen; the clicked action.
fn paint_switch(ui: &mut Ui, rect: Rect, chrome: MonitorChrome, auto: bool) -> Option<&'static str> {
    if !qnc_program_output::showing(ui.ctx()) {
        return None;
    }
    let font = FontId::proportional(chrome.font_size);
    let mut right = rect.right() - 6.0;
    let mut clicked = None;
    for (action, label, on) in [
        (MONITOR_ON, qnc_ui_text::text("monitor.on"), !auto),
        (MONITOR_AUTO, qnc_ui_text::text("monitor.auto"), auto),
    ] {
        let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), chrome.muted);
        let size = galley.size() + egui::vec2(12.0, 6.0);
        let button = Rect::from_min_size(egui::pos2(right - size.x, rect.top() + 6.0), size);
        right = button.left() - 4.0;
        let response = ui.interact(button, ui.id().with(("qnc-monitor-switch", action)), Sense::click());
        let fill = if on { chrome.border } else { chrome.fill };
        ui.painter().rect_filled(button, 2.0, fill);
        ui.painter().rect_stroke(button, 2.0, Stroke::new(1.0, chrome.border), StrokeKind::Inside);
        ui.painter().text(button.center(), Align2::CENTER_CENTER, label, font.clone(), if on || response.hovered() { Color32::WHITE } else { chrome.muted });
        if response.clicked() {
            clicked = Some(action);
        }
    }
    clicked
}

fn paint_source_view(
    ui: &mut Ui,
    rect: Rect,
    surface: MonitorSurface<'_>,
    poster: Option<MonitorPoster<'_>>,
    label: &str,
) {
    let message = surface.message.filter(|text| !text.is_empty());
    let picture_only = MonitorSurface {
        message: if poster.is_some() { None } else { surface.message },
        ..surface
    };
    match paint_monitor(ui, rect, picture_only) {
        MonitorPaint::Picture | MonitorPaint::Message => return,
        MonitorPaint::Empty => {}
    }
    if let Some(poster) = poster {
        if qnc_ui_kit::paint_rgba_image(
            ui,
            rect.shrink(1.0),
            poster.uri,
            poster.content_key,
            poster.size,
            poster.rgba,
        ) {
            if let Some(message) = message {
                paint_caption(ui, rect, surface.chrome, message);
            }
            return;
        }
    }
    match message {
        Some(message) => paint_placeholder(ui, rect, surface.chrome, message),
        None => paint_placeholder(ui, rect, surface.chrome, label),
    }
}

/// A message over the bottom of a poster, on a dark band so it stays readable.
fn paint_caption(ui: &mut Ui, rect: Rect, chrome: MonitorChrome, message: &str) {
    let galley = ui.painter().layout(
        message.to_string(),
        FontId::proportional(chrome.font_size),
        chrome.muted,
        (rect.width() - 24.0).max(1.0),
    );
    let band = Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.bottom() - galley.size().y - 13.0),
        egui::pos2(rect.right() - 1.0, rect.bottom() - 1.0),
    );
    ui.painter().rect_filled(band, 0.0, chrome.fill.gamma_multiply(0.75));
    ui.painter().galley(
        egui::pos2(rect.center().x - galley.size().x * 0.5, band.top() + 6.0),
        galley,
        chrome.muted,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_surface_does_not_invent_a_picture() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, egui::vec2(160.0, 90.0));
                assert_eq!(
                    paint_monitor(
                        ui,
                        rect,
                        MonitorSurface {
                            id: egui::Id::new("any-app-monitor"),
                            chrome: MonitorChrome {
                                fill: Color32::BLACK,
                                border: Color32::GRAY,
                                muted: Color32::GRAY,
                                font_size: 12.0,
                            },
                            picture: None,
                            message: None,
                            yield_to_external: false,
                        },
                    ),
                    MonitorPaint::Empty
                );
            });
        });
    }

    #[test]
    fn valid_pixels_paint_as_a_picture() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, egui::vec2(160.0, 90.0));
                assert_eq!(
                    paint_monitor(
                        ui,
                        rect,
                        MonitorSurface {
                            id: egui::Id::new("any-app-monitor"),
                            chrome: MonitorChrome {
                                fill: Color32::BLACK,
                                border: Color32::GRAY,
                                muted: Color32::GRAY,
                                font_size: 12.0,
                            },
                            picture: Some(MonitorPicture {
                                session_id: "session",
                                generation: 1,
                                sequence: 4,
                                size: [2, 1],
                                rgba: &[255; 8],
                                frame_map: None,
                            }),
                            message: Some("must not hide a confirmed frame"),
                            yield_to_external: false,
                        },
                    ),
                    MonitorPaint::Picture
                );
            });
        });
    }

    #[test]
    fn invalid_pixels_do_not_count_as_a_painted_picture() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, egui::vec2(160.0, 90.0));
                assert_eq!(
                    paint_monitor(
                        ui,
                        rect,
                        MonitorSurface {
                            id: egui::Id::new("any-app-monitor"),
                            chrome: MonitorChrome {
                                fill: Color32::BLACK,
                                border: Color32::GRAY,
                                muted: Color32::GRAY,
                                font_size: 12.0,
                            },
                            picture: Some(MonitorPicture {
                                session_id: "session",
                                generation: 1,
                                sequence: 1,
                                size: [2, 2],
                                rgba: &[255, 0, 0],
                                frame_map: None,
                            }),
                            message: Some("decoder error"),
                            yield_to_external: false,
                        },
                    ),
                    MonitorPaint::Message
                );
            });
        });
    }
}
