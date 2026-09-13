//! Passive public monitor / preview surface.
//!
//! Any application embeds this component with a caller-owned surface id and
//! already-confirmed pixels. It does not own a clock, decode media, read a
//! database, or know which application hosts it.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui};

pub const MODULE_ID: &str = "qnc.module.monitor";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
}

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
        if qnc_ui_kit::paint_stream_frame(
            ui,
            rect.shrink(1.0),
            surface.id,
            (picture.session_id, picture.generation, picture.sequence),
            picture.size,
            picture.rgba,
        ) {
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
                            }),
                            message: Some("must not hide a confirmed frame"),
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
                            }),
                            message: Some("decoder error"),
                        },
                    ),
                    MonitorPaint::Message
                );
            });
        });
    }
}
