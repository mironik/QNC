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

/// The one strong button of a row (e.g. Uvezi): white bold text on the accent fill.
pub fn strong_button(ui: &mut Ui, text: &str, enabled: bool, fill: Color32, height: f32) -> egui::Response {
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(fill)
            .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
            .corner_radius(CornerRadius::same(0))
            .min_size(Vec2::new(0.0, height)),
    )
}

/// Fills the rest of a block with its background and gives that rect.
pub fn fill_block(ui: &mut Ui, background: Color32) -> egui::Rect {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, background);
    rect
}

/// How a button looks, for forms whose buttons differ from the chrome ones (e.g. the
/// Project form: rounded, its UI font size). Everything comes from the form's contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonLook {
    pub text: Color32,
    pub fill: Color32,
    /// A one pixel border, or none.
    pub border: Option<Color32>,
    pub font_size: Option<f32>,
    pub strong: bool,
    /// Square corners (the chrome buttons) or egui's default rounding.
    pub square: bool,
    pub min_size: Vec2,
}

/// A button drawn as `look` says.
pub fn look_button(ui: &mut Ui, text: &str, enabled: bool, look: ButtonLook) -> egui::Response {
    let mut rich = RichText::new(text).color(look.text);
    if let Some(size) = look.font_size {
        rich = rich.size(size);
    }
    if look.strong {
        rich = rich.strong();
    }
    let mut button = Button::new(rich).fill(look.fill).min_size(look.min_size);
    if let Some(border) = look.border {
        button = button.stroke(Stroke::new(1.0, border));
    }
    if look.square {
        button = button.corner_radius(CornerRadius::same(0));
    }
    ui.add_enabled(enabled, button)
}

/// The small button of a list row (egui's own look, small text), greyed while not `enabled`; `selected` draws it as chosen
/// (e.g. Edit while that segment is edited).
pub fn small_button(ui: &mut Ui, text: &str, enabled: bool, selected: bool) -> egui::Response {
    ui.add_enabled(enabled, Button::new(RichText::new(text).small()).selected(selected))
}

/// A button of egui's own look at a size (e.g. Undo, Save and Close of the trim zoom), put where
/// its caller places it.
pub fn sized_button(text: impl Into<egui::WidgetText>, size: Vec2) -> Button<'static> {
    Button::new(text).min_size(size)
}

/// A button of its caller's text (its size, colour, font), a fill and a 1 px `border` or none,
/// square corners or egui's rounding, at least `size`; put or added where its caller places it.
pub fn framed_button(text: impl Into<egui::WidgetText>, size: Vec2, fill: Color32, border: Option<Color32>, square: bool) -> Button<'static> {
    let mut button = Button::new(text).fill(fill).min_size(size);
    if let Some(border) = border {
        button = button.stroke(Stroke::new(1.0, border));
    }
    if square {
        button = button.corner_radius(CornerRadius::same(0));
    }
    button
}

/// A check box of the gui-kit, greyed while not `enabled`.
pub fn check_box(ui: &mut Ui, checked: &mut bool, text: impl Into<egui::WidgetText>, enabled: bool) -> egui::Response {
    ui.add_enabled(enabled, egui::Checkbox::new(checked, text))
}
