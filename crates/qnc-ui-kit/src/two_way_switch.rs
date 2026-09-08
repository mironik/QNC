use eframe::egui::{self, Button, Color32, CornerRadius, FontId, RichText, Stroke, Vec2};

use crate::FormActionBarStyle;

/// A passive, equal-width pair of modes, ordered left to right even in an RTL toolbar.
pub fn show_two_way_switch(
    ui: &mut egui::Ui,
    labels: [&str; 2],
    selected: usize,
    mode_colors: [Color32; 2],
    style: &FormActionBarStyle,
) -> Option<usize> {
    let width = labels.iter().fold(style.button_width, |width, label| {
        width.max(
            ui.painter()
                .layout_no_wrap(
                    (*label).into(),
                    FontId::proportional(style.font_size),
                    style.text,
                )
                .size()
                .x
                + 2.0 * ui.spacing().button_padding.x,
        )
    });
    let mut changed = None;
    ui.allocate_ui_with_layout(
        Vec2::new(2.0 * width, style.height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (index, label) in labels.into_iter().enumerate() {
                let active = index == selected;
                let response = ui.add_sized(
                    Vec2::new(width, style.height),
                    Button::new(RichText::new(label).size(style.font_size).color(if active {
                        style.primary_text
                    } else {
                        style.text
                    }))
                    .selected(active)
                    .fill(if active {
                        mode_colors[index]
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(Stroke::new(1.0, style.border))
                    .corner_radius(CornerRadius::same(0)),
                );
                if active {
                    ui.painter().hline(
                        response.rect.x_range().shrink(5.0),
                        response.rect.bottom() - 2.0,
                        Stroke::new(2.0, style.primary_text),
                    );
                }
                if response.clicked() && !active {
                    changed = Some(index);
                }
            }
        },
    );
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn equal_modes_keep_order_and_geometry_in_both_toolbar_directions() {
        for rtl in [false, true] {
            for labels in [["Novi", "Sve"], ["New", "All"]] {
                let ctx = egui::Context::default();
                let mut style = FormActionBarStyle::new(
                    Color32::WHITE,
                    Color32::GREEN,
                    Color32::GRAY,
                    14.0,
                    24.0,
                );
                style.button_width = 48.0;
                let paint = |selected, events| {
                    let mut change = None;
                    let mut neighbor = Rect::NOTHING;
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(400.0, 100.0),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                ui.with_layout(
                                    if rtl {
                                        egui::Layout::right_to_left(egui::Align::Center)
                                    } else {
                                        egui::Layout::left_to_right(egui::Align::Center)
                                    },
                                    |ui| {
                                        change = show_two_way_switch(
                                            ui,
                                            labels,
                                            selected,
                                            [Color32::GREEN, Color32::BLUE],
                                            &style,
                                        );
                                        neighbor = ui
                                            .add_sized(
                                                Vec2::new(60.0, style.height),
                                                Button::new("Next"),
                                            )
                                            .rect;
                                    },
                                );
                            });
                        },
                    );
                    let centers = output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if labels.contains(&text.galley.text()) => {
                                Some(text.pos + text.galley.size() / 2.0)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(centers.len(), 2);
                    for (index, color) in [Color32::GREEN, Color32::BLUE].into_iter().enumerate() {
                        let expected = if index == selected {
                            color
                        } else {
                            Color32::TRANSPARENT
                        };
                        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                            egui::Shape::Rect(rect) if rect.fill == expected && rect.rect.contains(centers[index])
                        )), "only the active mode has a color fill");
                    }
                    if rtl {
                        assert!(
                            neighbor.right() < centers[0].x - 23.0,
                            "neighbor overlaps left mode: {neighbor:?} {centers:?}"
                        );
                    } else {
                        assert!(
                            neighbor.left() > centers[1].x + 23.0,
                            "neighbor overlaps right mode"
                        );
                    }
                    (centers, change)
                };
                let (first, changed) = paint(0, vec![]);
                assert_eq!(first.len(), 2);
                assert!(changed.is_none());
                assert!((first[1].x - first[0].x - 48.0).abs() < 1.0);
                assert!((first[0].y - first[1].y).abs() < 1.0);
                let (second, _) = paint(1, vec![]);
                assert_eq!(first, second, "mode changes cannot resize or move labels");
                let event = |pressed| egui::Event::PointerButton {
                    pos: first[0],
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                };
                paint(1, vec![egui::Event::PointerMoved(first[0]), event(true)]);
                let (_, changed) = paint(1, vec![event(false)]);
                assert_eq!(changed, Some(0), "left mode stays left inside RTL toolbar");
            }
        }
    }
}
