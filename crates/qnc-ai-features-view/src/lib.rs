//! The AI section of a settings draft (docs/102, user 2026-11-11): the AI template, one switch per
//! AI feature, and keep or delete an own AI template. A feature a chosen feature needs is shown
//! on and greyed, with what needs it; the rule and the writing are `qnc-ai-features`.

use eframe::egui::{self, RichText, Vec2};
use qnc_ai_features::{chosen_of, template_of, Choice};
use serde_json::Value;

/// Paints the section; true when the draft changed.
pub fn show(ui: &mut egui::Ui, choice: &mut Choice, settings: &mut Value) -> bool {
    let features = match &choice.features {
        Ok(features) => features.clone(),
        Err(error) => {
            ui.label(qnc_ui_text::fill("ai.features_unreadable", &[("error", error)]));
            return false;
        }
    };
    let mut changed = false;
    let chosen = chosen_of(settings);
    let current = template_of(settings);
    let shown = match choice.templates.iter().find(|template| template.id == current) {
        Some(template) if template.features == chosen => Choice::template_name(template),
        Some(template) => qnc_ui_text::fill("ai.template_changed", &[("name", &Choice::template_name(template))]),
        None => current.clone(),
    };
    let mut picked = None;
    let mut delete = None;
    ui.horizontal(|ui| {
        ui.label(qnc_ui_text::text("ai.template_label"));
        egui::ComboBox::from_id_salt("qnc_ai_template").selected_text(shown).width(260.0).show_ui(ui, |ui| {
            for template in &choice.templates {
                if ui.selectable_label(template.id == current, Choice::template_name(template)).clicked() {
                    picked = Some(template.id.clone());
                }
            }
        });
        let own = choice.templates.iter().any(|template| template.id == current && template.own);
        if own && ui.add(qnc_ui_kit::sized_button(qnc_ui_text::text("ai.delete_template"), Vec2::new(0.0, 22.0))).clicked() {
            delete = Some(current.clone());
        }
    });
    if let Some(id) = picked {
        choice.choose_template(settings, &id);
        changed = true;
    }
    if let Some(id) = delete {
        choice.delete_template(&id);
    }
    let chosen = chosen_of(settings);
    for feature in &features.list {
        let mut on = chosen.contains(&feature.code);
        let needed: Vec<String> = features.needed_by(&chosen, &feature.code).iter().map(|by| qnc_ai_features::words_of(&by.name)).collect();
        ui.horizontal(|ui| {
            if qnc_ui_kit::check_box(ui, &mut on, qnc_ai_features::words_of(&feature.name), needed.is_empty()).changed() {
                choice.switch(settings, &feature.code, on);
                changed = true;
            }
            if !needed.is_empty() {
                ui.label(RichText::new(qnc_ui_text::fill("ai.needed_lock", &[("names", &needed.join(", "))])).weak().small());
            }
            // Where its engine runs on this computer, or why it is not installed (docs/102 step 4).
            for state in choice.engines_of(feature) {
                match state {
                    Ok(qnc_ai_engines::Runs::Here(engine)) => ui.label(RichText::new(qnc_ui_text::fill("ai.engine_here", &[("engine", engine)])).weak().small()),
                    Ok(qnc_ai_engines::Runs::Server { address, engine }) => ui.label(RichText::new(qnc_ui_text::fill("ai.engine_server", &[("address", address), ("engine", engine)])).weak().small()),
                    Err(why) => ui.label(RichText::new(qnc_ui_text::fill("ai.engine_missing", &[("why", why)])).color(qnc_desktop_theme::danger()).small()),
                };
            }
        });
    }
    ui.horizontal(|ui| {
        ui.add(qnc_ui_kit::text_field(&mut choice.name).desired_width(220.0).hint_text(qnc_ui_text::text("ai.keep_name_hint")));
        if ui.add(qnc_ui_kit::sized_button(qnc_ui_text::text("ai.keep_template"), Vec2::new(0.0, 22.0))).clicked() {
            choice.keep_template(settings);
            changed = true;
        }
    });
    for line in choice.errors.iter().chain(std::iter::once(&choice.message).filter(|message| !message.is_empty())) {
        ui.label(RichText::new(line).small());
    }
    changed
}
