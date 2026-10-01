//! Media pool head: library tabs (All / Virtual / B-roll / Segment) and the
//! compact transport strip (play, mark IN, mark OUT, quick cover, export).
//! Passive paint (mirrors qnc_v4 `editorial::media_pool::show_head`): the caller
//! owns state, colours and what each action does; the module returns one intent.

use eframe::egui::{self, Color32, RichText, Sense, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryTab {
    #[default]
    All,
    Virtual,
    /// Shown as "B-roll", as in qnc_v4.
    Cover,
    Segment,
}

impl LibraryTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Virtual => "Virtual",
            Self::Cover => "B-roll",
            Self::Segment => "Segment",
        }
    }
}

/// Which parts of the head are on (from the group composition in
/// `contracts/ui/editorial.layout.json`).
#[derive(Debug, Clone, Copy)]
pub struct PoolHeadInput {
    pub library_tab: LibraryTab,
    pub playing: bool,
    pub show_segment_tab: bool,
    pub show_cover_tab: bool,
    pub show_export_hires: bool,
    pub export_hires_pending: bool,
    pub show_quick_cover: bool,
}

/// Neutral intent. The caller decides what it means; the module does nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolHeadAction {
    None,
    SwitchTab(LibraryTab),
    TogglePlay,
    MarkIn,
    MarkOut,
    QuickCover,
    ExportHiRes,
}

/// Colours and metrics from the UI contract (`pool_head` / theme).
#[derive(Debug, Clone, Copy)]
pub struct PoolHeadStyle {
    pub chrome_fill: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub font_ui: f32,
    pub chrome_row_height: f32,
    pub chrome_control_height: f32,
    pub chrome_pad_x: i8,
    pub chrome_pad_y: i8,
    pub tab_gap: f32,
    pub transport_button_width: f32,
    pub export_button_width: f32,
}

/// Tabs in display order for the given flags.
pub fn visible_tabs(input: &PoolHeadInput) -> Vec<LibraryTab> {
    [
        Some(LibraryTab::All),
        Some(LibraryTab::Virtual),
        input.show_cover_tab.then_some(LibraryTab::Cover),
        input.show_segment_tab.then_some(LibraryTab::Segment),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn export_label(pending: bool) -> &'static str {
    if pending {
        "Export..."
    } else {
        "Export HI-res"
    }
}

pub fn play_label(playing: bool) -> &'static str {
    if playing {
        "||"
    } else {
        ">"
    }
}

pub fn show_head(ui: &mut egui::Ui, style: &PoolHeadStyle, input: PoolHeadInput) -> PoolHeadAction {
    let mut action = PoolHeadAction::None;
    chrome_row(ui, style, |ui| {
        for tab in visible_tabs(&input) {
            let active = input.library_tab == tab;
            if link_tab(ui, style, tab.label(), active).clicked() {
                action = PoolHeadAction::SwitchTab(tab);
            }
            ui.add_space(style.tab_gap);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if input.show_export_hires
                && transport_btn(
                    ui,
                    style,
                    export_label(input.export_hires_pending),
                    input.export_hires_pending,
                )
                .clicked()
            {
                action = PoolHeadAction::ExportHiRes;
            }
            if input.show_quick_cover
                && transport_btn(ui, style, "B", false)
                    .on_hover_text("Quick cover")
                    .clicked()
            {
                action = PoolHeadAction::QuickCover;
            }
            if transport_btn(ui, style, "]", false)
                .on_hover_text("Mark OUT")
                .clicked()
            {
                action = PoolHeadAction::MarkOut;
            }
            if transport_btn(ui, style, "[", false)
                .on_hover_text("Mark IN")
                .clicked()
            {
                action = PoolHeadAction::MarkIn;
            }
            if transport_btn(ui, style, play_label(input.playing), false)
                .on_hover_text("Play / Pause")
                .clicked()
            {
                action = PoolHeadAction::TogglePlay;
            }
        });
    });
    action
}

/// Fixed-height chrome strip: vertical pad and a full-width bottom rule.
fn chrome_row(ui: &mut egui::Ui, style: &PoolHeadStyle, add_contents: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let out = ui.allocate_ui_with_layout(
        Vec2::new(width, style.chrome_row_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            egui::Frame::NONE
                .fill(style.chrome_fill)
                .inner_margin(egui::Margin {
                    left: style.chrome_pad_x,
                    right: style.chrome_pad_x,
                    top: style.chrome_pad_y,
                    bottom: style.chrome_pad_y,
                })
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), style.chrome_control_height));
                    ui.set_max_height(style.chrome_control_height);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_height(style.chrome_control_height);
                        add_contents(ui);
                    });
                });
        },
    );
    let r = out.response.rect;
    ui.painter().hline(
        r.x_range(),
        r.bottom() - 0.5,
        egui::Stroke::new(1.0, style.border),
    );
}

/// Ghost chrome button (`>`, `[`, `]`, `B`, Export HI-res).
fn transport_btn(
    ui: &mut egui::Ui,
    style: &PoolHeadStyle,
    label: &str,
    active: bool,
) -> egui::Response {
    let width = if label.starts_with("Export") {
        style.export_button_width
    } else {
        style.transport_button_width
    };
    let text = if active { style.accent } else { style.text };
    let fill = if active {
        style.accent.gamma_multiply(0.16)
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if active {
        egui::Stroke::new(1.0, style.accent)
    } else {
        egui::Stroke::new(1.0, style.border)
    };
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .color(text)
                .monospace()
                .size(style.font_ui),
        )
        .min_size(Vec2::new(width, style.chrome_control_height))
        .fill(fill)
        .stroke(stroke),
    )
}

/// Text tab link (underline when active).
fn link_tab(ui: &mut egui::Ui, style: &PoolHeadStyle, label: &str, active: bool) -> egui::Response {
    let text = if active {
        RichText::new(label)
            .color(style.text)
            .strong()
            .size(style.font_ui)
    } else {
        RichText::new(label).color(style.muted).size(style.font_ui)
    };
    let resp = ui.add(
        egui::Label::new(text)
            .sense(Sense::click())
            .selectable(false),
    );
    if active {
        let r = resp.rect;
        ui.painter().hline(
            r.left()..=r.right(),
            r.bottom() + 1.0,
            egui::Stroke::new(2.0, style.accent),
        );
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(segment: bool, cover: bool) -> PoolHeadInput {
        PoolHeadInput {
            library_tab: LibraryTab::All,
            playing: false,
            show_segment_tab: segment,
            show_cover_tab: cover,
            show_export_hires: false,
            export_hires_pending: false,
            show_quick_cover: false,
        }
    }

    #[test]
    fn media_assist_head_has_cover_but_no_segment_tab() {
        assert_eq!(
            visible_tabs(&head(false, true)),
            [LibraryTab::All, LibraryTab::Virtual, LibraryTab::Cover]
        );
    }

    #[test]
    fn story_head_has_all_four_tabs_in_order() {
        assert_eq!(
            visible_tabs(&head(true, true)),
            [
                LibraryTab::All,
                LibraryTab::Virtual,
                LibraryTab::Cover,
                LibraryTab::Segment
            ]
        );
    }

    #[test]
    fn minimal_head_is_all_and_virtual() {
        assert_eq!(
            visible_tabs(&head(false, false)),
            [LibraryTab::All, LibraryTab::Virtual]
        );
    }

    #[test]
    fn labels_match_qnc_v4() {
        assert_eq!(LibraryTab::Cover.label(), "B-roll");
        assert_eq!(export_label(false), "Export HI-res");
        assert_eq!(export_label(true), "Export...");
        assert_eq!(play_label(true), "||");
        assert_eq!(play_label(false), ">");
    }
}
