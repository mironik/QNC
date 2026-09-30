//! Block: the project list (title, one row per project with open and remove, status).

use eframe::egui::{self, RichText, Sense, Vec2};
use qnc_project_application::ProjectRow;
use qnc_project_session::ProjectIntent;

use crate::{layout_contract::ProjectListMetrics, theme::Theme, Blocks};

impl Blocks<'_> {
    pub(crate) fn project_list(&mut self, ui: &mut egui::Ui, width: f32, height: f32) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let metrics = self.contracts.project.left_project_list.clone();
        let (panel_rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        ui.painter().rect_filled(panel_rect, 0.0, t.bg);
        let content = egui::Rect::from_min_max(
            egui::pos2(
                panel_rect.left() + metrics.panel_pad,
                panel_rect.top() + metrics.panel_pad,
            ),
            egui::pos2(
                panel_rect.right() - metrics.panel_pad,
                panel_rect.bottom() - metrics.panel_pad,
            ),
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(content)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(content);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                self.project_list_contents(ui, content, &metrics);
            },
        );
    }

    fn project_list_contents(
        &mut self,
        ui: &mut egui::Ui,
        content: egui::Rect,
        metrics: &ProjectListMetrics,
    ) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let content_w = content.width().max(40.0);
        let (title_rect, _) = ui.allocate_exact_size(
            Vec2::new(content_w, metrics.title_row_height),
            Sense::hover(),
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(title_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.label(
                    RichText::new(&metrics.title)
                        .size(self.contracts.shell.theme_metrics.font_ui)
                        .strong()
                        .color(t.text),
                );
            },
        );
        ui.painter().hline(
            title_rect.x_range(),
            title_rect.bottom() - 0.5,
            egui::Stroke::new(1.0, t.border),
        );
        ui.add_space(metrics.below_title_pad);

        let foot_h = self.contracts.shell.theme_metrics.chrome_row_height;
        let table_top = ui.cursor().top();
        let table_bottom = (content.bottom() - foot_h).max(table_top + 40.0);
        let table_rect = egui::Rect::from_min_max(
            egui::pos2(content.left(), table_top),
            egui::pos2(content.right(), table_bottom),
        );
        let foot_rect = egui::Rect::from_min_size(
            egui::pos2(content.left(), table_bottom),
            Vec2::new(content_w, foot_h),
        );

        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(table_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(table_rect);
                ui.set_max_width(table_rect.width().max(40.0));
                egui::ScrollArea::vertical()
                    .id_salt("project_list")
                    .max_height(table_rect.height())
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_max_width(table_rect.width().max(40.0));
                        if self.session.projects.is_empty() {
                            ui.label(
                                RichText::new(&metrics.empty_text)
                                    .size(self.contracts.shell.theme_metrics.font_ui)
                                    .color(t.muted),
                            );
                            return;
                        }
                        for index in 0..self.session.projects.len() {
                            let project = self.session.projects[index].clone();
                            self.project_row(ui, index, &project, metrics);
                            if index + 1 < self.session.projects.len() {
                                ui.add_space(metrics.row_gap);
                            }
                        }
                    });
            },
        );

        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(foot_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_clip_rect(foot_rect);
                ui.label(
                    RichText::new(&self.session.status)
                        .size(self.contracts.shell.theme_metrics.font_ui)
                        .color(t.muted),
                );
            },
        );
    }

    fn project_row(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        project: &ProjectRow,
        metrics: &ProjectListMetrics,
    ) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        let table_w = ui.available_width().max(40.0);
        let (row_rect, _) =
            ui.allocate_exact_size(Vec2::new(table_w, metrics.row_height), Sense::hover());
        let delete_hit_w = metrics.delete_column_width.max(48.0);
        let delete_rect = egui::Rect::from_min_size(
            egui::pos2(row_rect.right() - delete_hit_w, row_rect.top()),
            Vec2::new(delete_hit_w, metrics.row_height),
        );
        let select_rect = egui::Rect::from_min_max(
            row_rect.min,
            egui::pos2(delete_rect.left() - metrics.column_gap, row_rect.bottom()),
        );
        let response = ui.interact(
            select_rect,
            ui.id().with(("project_list_row_select", index)),
            Sense::click(),
        );
        let selected = self.session.selected_project == Some(index);
        if selected || response.hovered() {
            let fill = if selected {
                t.surface
            } else {
                t.surface.linear_multiply(0.55)
            };
            ui.painter().rect_filled(row_rect, 0.0, fill);
        }
        if response.clicked() {
            self.intents.push(ProjectIntent::OpenProjectRow { index });
        }

        let label_rect = egui::Rect::from_min_max(
            egui::pos2(row_rect.left() + 20.0, row_rect.top()),
            select_rect.right_bottom(),
        );
        let date = format!("- {}", project.created_date);
        let date_galley = ui.painter().layout_no_wrap(
            date,
            egui::FontId::proportional(self.contracts.shell.theme_metrics.font_ui),
            t.muted,
        );
        let date_x = (label_rect.right() - date_galley.size().x).max(label_rect.left());
        let name_rect = egui::Rect::from_min_max(
            label_rect.min,
            egui::pos2(
                (date_x - metrics.column_gap).max(label_rect.left()),
                label_rect.bottom(),
            ),
        );
        ui.painter().with_clip_rect(label_rect).galley(
            egui::pos2(date_x, label_rect.center().y - date_galley.size().y * 0.5),
            date_galley,
            t.muted,
        );
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(name_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_clip_rect(name_rect.intersect(ui.clip_rect()));
                let mut label = RichText::new(&project.name)
                    .size(self.contracts.shell.theme_metrics.font_ui)
                    .color(t.text);
                if project.active {
                    label = label.strong();
                }
                ui.add(egui::Label::new(label).truncate().selectable(false));
            },
        );

        let delete_response = ui
            .interact(
                delete_rect,
                ui.id().with(("project_list_row_delete", index)),
                Sense::click(),
            )
            .on_hover_text("Ukloni projekt");
        let delete_color = if delete_response.hovered() {
            t.text
        } else {
            t.muted
        };
        ui.painter().text(
            delete_rect.center(),
            egui::Align2::CENTER_CENTER,
            "×",
            egui::FontId::proportional(self.contracts.shell.theme_metrics.font_ui + 5.0),
            delete_color,
        );
        if delete_response.clicked() {
            self.intents.push(ProjectIntent::RequestDeleteProject { index });
        }
    }
}
