use eframe::egui::{self, Button, Color32, CornerRadius, RichText, Stroke, Vec2};

pub const STANDARD_ACTION_BUTTON_WIDTH: f32 = 96.0;
pub const STANDARD_ACTION_BUTTON_GAP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormActionBarStyle {
    pub text: Color32,
    pub primary_text: Color32,
    pub primary_fill: Color32,
    pub border: Color32,
    pub font_size: f32,
    pub height: f32,
    pub button_width: f32,
    pub gap: f32,
    pub right_margin: f32,
}

impl FormActionBarStyle {
    pub fn new(
        text: Color32,
        primary_fill: Color32,
        border: Color32,
        font_size: f32,
        height: f32,
    ) -> Self {
        Self {
            text,
            primary_text: Color32::WHITE,
            primary_fill,
            border,
            font_size,
            height,
            button_width: STANDARD_ACTION_BUTTON_WIDTH,
            gap: STANDARD_ACTION_BUTTON_GAP,
            right_margin: STANDARD_ACTION_BUTTON_GAP,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FormActionBarResponse {
    pub confirm_clicked: bool,
    pub cancel_clicked: bool,
}

pub fn toggle_exclusive_panel(current_open: &mut bool, other_open: &mut bool) -> bool {
    let opened = !*current_open;
    *current_open = opened;
    if opened {
        *other_open = false;
    }
    opened
}

pub fn show_form_action_bar(
    ui: &mut egui::Ui,
    style: &FormActionBarStyle,
    confirm_label: &str,
    confirm_enabled: bool,
    cancel_label: &str,
) -> FormActionBarResponse {
    let mut response = FormActionBarResponse::default();
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), style.height),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = style.gap;
            ui.add_space(style.right_margin);
            if primary_button(ui, style, confirm_label, confirm_enabled).clicked() {
                response.confirm_clicked = true;
            }
            if action_button(ui, style, cancel_label).clicked() {
                response.cancel_clicked = true;
            }
        },
    );
    response
}

fn primary_button(
    ui: &mut egui::Ui,
    style: &FormActionBarStyle,
    label: &str,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(
            RichText::new(label)
                .color(style.primary_text)
                .strong()
                .size(style.font_size),
        )
        .fill(style.primary_fill)
        .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
        .corner_radius(CornerRadius::same(0))
        .min_size(action_button_size(style)),
    )
}

fn action_button(ui: &mut egui::Ui, style: &FormActionBarStyle, label: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).color(style.text).size(style.font_size))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, style.border))
            .corner_radius(CornerRadius::same(0))
            .min_size(action_button_size(style)),
    )
}

fn action_button_size(style: &FormActionBarStyle) -> Vec2 {
    Vec2::new(style.button_width, style.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_action_buttons_have_equal_width() {
        let style = FormActionBarStyle::new(
            Color32::WHITE,
            Color32::from_rgb(1, 2, 3),
            Color32::from_rgb(4, 5, 6),
            14.0,
            30.0,
        );

        assert_eq!(style.button_width, STANDARD_ACTION_BUTTON_WIDTH);
        assert_eq!(style.gap, STANDARD_ACTION_BUTTON_GAP);
    }

    #[test]
    fn opening_one_panel_closes_the_other() {
        let mut current_open = false;
        let mut other_open = true;

        let opened = toggle_exclusive_panel(&mut current_open, &mut other_open);

        assert!(opened);
        assert!(current_open);
        assert!(!other_open);
    }

    #[test]
    fn closing_current_panel_does_not_open_the_other() {
        let mut current_open = true;
        let mut other_open = false;

        let opened = toggle_exclusive_panel(&mut current_open, &mut other_open);

        assert!(!opened);
        assert!(!current_open);
        assert!(!other_open);
    }
}
