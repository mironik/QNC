use eframe::egui::{self, Color32, RichText};

pub struct OptionItem {
    pub id: Option<String>,
    pub label: String,
    pub selected: bool,
}

pub struct OptionColumn {
    pub id: String,
    pub label: String,
    pub options: Vec<OptionItem>,
}

pub struct OptionColumnsStyle {
    pub font_size: f32,
    pub text: Color32,
    pub min_column_width: f32,
    pub column_gap: f32,
    pub row_gap: f32,
}

pub struct OptionSelection {
    pub column_id: String,
    pub option_id: Option<String>,
}

fn column_width(available: f32, count: usize, style: &OptionColumnsStyle) -> f32 {
    ((available - style.column_gap * count.saturating_sub(1) as f32) / count.max(1) as f32)
        .max(style.min_column_width)
}

/// Passive, unframed option columns. The caller owns selection and validation.
pub fn show_option_columns(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    columns: &[OptionColumn],
    style: &OptionColumnsStyle,
) -> Option<OptionSelection> {
    if columns.is_empty() {
        return None;
    }
    let id = egui::Id::new(id);
    let width = column_width(ui.available_width(), columns.len(), style);
    let mut selected = None;
    egui::ScrollArea::horizontal()
        .id_salt(id.with("scroll"))
        .show(ui, |ui| {
            egui::Grid::new(id.with("columns"))
                .num_columns(columns.len())
                .striped(false)
                .spacing([style.column_gap, style.row_gap])
                .show(ui, |ui| {
                    for column in columns {
                        ui.push_id(&column.id, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(width);
                                ui.spacing_mut().item_spacing.y = style.row_gap;
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                ui.label(
                                    RichText::new(&column.label)
                                        .size(style.font_size)
                                        .color(style.text),
                                );
                                for option in &column.options {
                                    if ui
                                        .radio(
                                            option.selected,
                                            RichText::new(&option.label)
                                                .size(style.font_size)
                                                .color(style.text),
                                        )
                                        .clicked()
                                    {
                                        selected = Some(OptionSelection {
                                            column_id: column.id.clone(),
                                            option_id: option.id.clone(),
                                        });
                                    }
                                }
                            });
                        });
                    }
                    ui.end_row();
                });
        });
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_have_equal_width_and_scroll_instead_of_stacking() {
        let style = OptionColumnsStyle {
            font_size: 14.0,
            text: Color32::WHITE,
            min_column_width: 160.0,
            column_gap: 12.0,
            row_gap: 6.0,
        };
        assert_eq!(column_width(652.0, 2, &style), 320.0);
        assert_eq!(column_width(200.0, 3, &style), 160.0);
    }

    #[test]
    fn option_columns_paint_without_mutating_input() {
        let columns = vec![OptionColumn {
            id: "group".into(),
            label: "Group".into(),
            options: vec![OptionItem {
                id: Some("one".into()),
                label: "One".into(),
                selected: true,
            }],
        }];
        let style = OptionColumnsStyle {
            font_size: 14.0,
            text: Color32::WHITE,
            min_column_width: 160.0,
            column_gap: 12.0,
            row_gap: 6.0,
        };
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(show_option_columns(ui, "test", &columns, &style).is_none());
            });
        });
        assert!(!output.shapes.is_empty());
        assert!(columns[0].options[0].selected);
    }
}
