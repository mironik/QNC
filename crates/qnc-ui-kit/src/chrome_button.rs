use eframe::egui::{self, Button, Color32, CornerRadius, RichText, Stroke, Ui, Vec2};

/// Colours and height of a chrome button, from the form's layout contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeButtonStyle {
    pub text: Color32,
    pub border: Color32,
    /// Text and border of a button that is on (one side of a switch).
    pub focus: Color32,
    pub height: f32,
}

/// The flat bordered button of a dock or head row; `on` draws it in the focus colour
/// (the side of a switch that is chosen, e.g. Pokrivalice | Sync/B-roll).
pub fn chrome_button(ui: &mut Ui, text: &str, enabled: bool, on: bool, style: ChromeButtonStyle) -> egui::Response {
    let colour = if on { style.focus } else { style.text };
    let border = if on { style.focus } else { style.border };
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(colour))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(0.0, style.height)),
    )
}
