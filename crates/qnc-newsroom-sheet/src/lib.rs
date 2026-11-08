//! The story sheet (user 2026-10-27: the data of a news story is a sheet the newsroom shapes;
//! an administrator makes their own when none of the prepared ones fits; the project name is
//! written in it and the form takes it). The overlay over the desktop fills a sheet: the
//! project name first, then its fields one under the other, the catalog key for the next
//! field moving on. Its editor changes the sheet (names, kinds, choices, order, new fields)
//! unless it is locked. It writes the sheets of this computer; the values go back to its
//! owner, which keeps them in the project database (`qnc-newsroom-sheet-db`).



use eframe::egui::{self, Align2, RichText, Stroke, Vec2};
use qnc_desktop_theme::Theme;
use qnc_settings_form::{FieldKind, GridStyle};

pub use qnc_newsroom_sheet_model::{code_of, new_field, new_field_in, today, valid_code, Answer, Origin, SheetOverlay, SheetSpec, Sheets};

pub const MODULE_ID: &str = "qnc.module.newsroom-sheet";

const WIDTH: f32 = 760.0;
pub const LABEL_WIDTH: f32 = 200.0;
/// As tall as the template picker row of the Project panel (user 2026-10-28).
pub const ROW_HEIGHT: f32 = 32.0;
pub const ROW_GAP: f32 = 12.0;

const MARGIN: f32 = 24.0;


/// The buttons of the filled sheet as a row of the keyboard (the one row block of every form,
/// `qnc-key-row`): Cancel and Create, left to right, by their catalog actions.
const CANCEL: &str = "project_story_cancel";
const CONFIRM: &str = "project_story_confirm";

/// The keys of the filled sheet from the form's catalog (scope `project`): the next field
/// (pressed while a field is typed in), and Enter and the arrows of a key row (the same
/// actions as on every row of the forms) while the buttons have the keyboard.
struct SheetKeys {
    next_field: bool,
    enter: bool,
    arrow: Option<bool>,
}

impl SheetKeys {
    fn read(ctx: &egui::Context, catalog: &qnc_keyboard_shortcut::ShortcutCatalog) -> Self {
        let typing = ctx.wants_keyboard_input();
        let events = qnc_keyboard_shortcut::egui_shortcut_events(ctx);
        let pressed = |action: &str| !typing && events.iter().any(|event| catalog.action_ids_for_event("project", event).contains(&action));
        let arrow = if pressed("step_back_frame") { Some(true) } else if pressed("step_forward_frame") { Some(false) } else { None };
        Self { next_field: qnc_keyboard_shortcut::pressed_while_typing(ctx, catalog, "project", "form_next_field"), enter: pressed("form_next_field"), arrow }
    }
}

/// Draws the overlay in the middle of the screen, whole on it: a title bar as a window has,
/// the sheet (it scrolls when it is taller than the screen), the buttons on the right at the
/// bottom. The keys come from the form's shortcut catalog. Opened, the project name has the
/// keyboard (user 2026-11-08); every Enter goes to the next field, the last to Create.
pub fn show(ctx: &egui::Context, overlay: &mut SheetOverlay, theme: &Theme, catalog: &qnc_keyboard_shortcut::ShortcutCatalog) -> Option<Answer> {
    let keys = SheetKeys::read(ctx, catalog);
    // Opened now (not drawn the pass before): the keyboard is on the first row, the sheet's
    // choice (user 2026-11-08: no mouse, Enter from row to row).
    let seen = egui::Id::new("qnc_story_sheet_seen");
    let pass = ctx.cumulative_pass_nr();
    let opened = ctx.data(|data| data.get_temp::<u64>(seen)).is_none_or(|last| last + 1 < pass);
    ctx.data_mut(|data| data.insert_temp(seen, pass));
    let place_id = egui::Id::new("qnc_story_sheet_keyboard");
    let mut place: Place = if opened { Place::Row(0, true) } else { ctx.data(|data| data.get_temp(place_id)).unwrap_or(Place::Row(0, true)) };
    // Enter: the next row, from the last one the buttons (Create).
    if keys.next_field || keys.enter {
        let rows = rows_of(overlay);
        place = match place {
            Place::Row(at, _) if at + 1 < rows => Place::Row(at + 1, true),
            Place::Row(..) => {
                let mut row = qnc_key_row::KeyRow::new([CANCEL, CONFIRM]);
                row.select(CONFIRM);
                Place::Buttons(row, true)
            }
            buttons => buttons,
        };
    }
    // A row of choices or the buttons have the keyboard: no text box keeps it.
    if !matches!(place, Place::Row(at, _) if is_typed(overlay, at)) {
        if let Some(focused) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(focused));
        }
    }
    let screen = ctx.screen_rect();
    let width = WIDTH.min(screen.width() - 2.0 * MARGIN);
    let bar = theme.chrome_row_height.max(ROW_HEIGHT + 4.0);
    let buttons = theme.chrome_control_height + 2.0 * MARGIN;
    let body_height = (screen.height() - 2.0 * MARGIN - bar - buttons - 2.0 * MARGIN).max(80.0);
    let editing = overlay.editing.is_some();
    let title = qnc_ui_text::text(if editing { "sheet.edit_title" } else { "sheet.title" });
    let mut answer = None;
    egui::Area::new(egui::Id::new("qnc_story_sheet"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::NONE.fill(theme.panel).stroke(Stroke::new(1.0, theme.border)).show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 0.0;
                title_bar(ui, theme, title, width, bar);
                egui::Frame::NONE.inner_margin(egui::Margin { left: MARGIN as i8, right: MARGIN as i8, top: MARGIN as i8, bottom: 0 }).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    egui::ScrollArea::vertical().max_height(body_height).auto_shrink([false, true]).show(ui, |ui| {
                        if editing {
                            editor(ui, overlay, theme);
                        } else {
                            let at = match &place {
                                Place::Row(at, arrived) => Some((*at, *arrived)),
                                Place::Buttons(..) => None,
                            };
                            // A click into a text box moves the keyboard to its row.
                            if let Some(typed) = fill(ui, overlay, theme, at, keys.arrow) {
                                if !matches!(place, Place::Row(at, _) if at == typed) {
                                    place = Place::Row(typed, false);
                                }
                            }
                        }
                    });
                });
                egui::Frame::NONE.inner_margin(MARGIN).show(ui, |ui| {
                    answer = if editing {
                        editor_buttons(ui, overlay, theme)
                    } else {
                        // The arrows between Cancel and Create, Enter presses the one the keyboard
                        // is on (not the Enter that brought it there).
                        let mut pressed = None;
                        if let Place::Buttons(row, arrived) = &mut place {
                            if let Some(back) = keys.arrow {
                                row.choose(back);
                            } else if keys.enter && !*arrived {
                                pressed = row.chosen().and_then(|action| answer_of(action, overlay));
                            }
                            *arrived = false;
                        }
                        let chosen = match &place {
                            Place::Buttons(row, _) => row.chosen(),
                            Place::Row(..) => None,
                        };
                        let clicked = fill_buttons(ui, overlay, theme, chosen);
                        pressed.or(clicked)
                    };
                });
            });
        });
    // Come to a row once: the next pass it is only there.
    if let Place::Row(at, true) = place {
        place = Place::Row(at, false);
    }
    ctx.data_mut(|data| data.insert_temp(place_id, place));
    answer
}

/// Where the keyboard is in the filled sheet: on a row (and whether it just came there), or on
/// the buttons' key row (`qnc-key-row`).
#[derive(Debug, Clone)]
enum Place {
    Row(usize, bool),
    Buttons(qnc_key_row::KeyRow, bool),
}

/// The rows of the filled sheet: its choice, the project name, its fields.
fn rows_of(overlay: &SheetOverlay) -> usize {
    2 + overlay.sheets.current().map_or(0, |sheet| sheet.fields.len())
}

/// Whether row `at` is typed in (the name, a field with no choices).
fn is_typed(overlay: &SheetOverlay, at: usize) -> bool {
    match at {
        0 => false,
        1 => true,
        _ => overlay.sheets.current().and_then(|sheet| sheet.fields.get(at - 2).map(|field| !qnc_settings_form::is_chosen(field, &sheet.options))).unwrap_or(false),
    }
}

/// The answer of the button the keyboard pressed (Create only when the sheet can be confirmed).
fn answer_of(action: &str, overlay: &SheetOverlay) -> Option<Answer> {
    match action {
        CONFIRM => ready(overlay).then_some(Answer::Confirm),
        CANCEL => Some(Answer::Cancel),
        _ => None,
    }
}

/// The id of the project name's text box.
const NAME_ID: &str = "qnc_story_sheet_name";

/// Whether the sheet can be confirmed: a name and a sheet.
fn ready(overlay: &SheetOverlay) -> bool {
    !overlay.name.trim().is_empty() && overlay.sheets.current().is_some()
}

/// The title of the overlay in a bar along its top, as a window has.
fn title_bar(ui: &mut egui::Ui, theme: &Theme, title: &str, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.surface_alt);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, theme.border));
    ui.painter().text(
        rect.left_center() + Vec2::new(MARGIN, 0.0),
        Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(theme.font_ui),
        theme.text,
    );
}

/// Buttons on the right of their row, the proposed one rightmost.
fn right_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
}

fn style(theme: &Theme) -> GridStyle {
    GridStyle {
        label_color: theme.text_muted,
        label_size: theme.font_ui,
        min_cell_width: 160.0,
        gap: ROW_GAP,
        cell_height: ROW_HEIGHT + 4.0,
        label_beside: Some((LABEL_WIDTH, ROW_HEIGHT)),
    }
}

/// A text box as tall as a row, its text in the middle.
pub fn row_box(text: &mut String, width: f32) -> egui::TextEdit<'_> {
    qnc_ui_kit::row_field(text, width, ROW_HEIGHT)
}

/// A row's label on the left, as wide as the labels of the fields and as tall as the row.
pub fn label(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    let text = RichText::new(text).size(theme.font_ui).color(theme.text_muted);
    ui.add_sized(Vec2::new(LABEL_WIDTH, ROW_HEIGHT), egui::Label::new(text));
    ui.add_space(ROW_GAP);
}

pub fn button(ui: &mut egui::Ui, label: &str, enabled: bool, theme: &Theme) -> bool {
    qnc_ui_kit::chrome_button(ui, label, enabled, false, qnc_theme_styles::button(theme)).clicked()
}

/// The sheet filled: its choice, the project name, its fields. `at` is the row the keyboard is
/// on and whether it just came there; `arrow` walks the choices of a row of choices. The row
/// whose text box has the keyboard (a click into it) comes back.
fn fill(ui: &mut egui::Ui, overlay: &mut SheetOverlay, theme: &Theme, at: Option<(usize, bool)>, arrow: Option<bool>) -> Option<usize> {
    let width = ui.available_width();
    let field_w = width - LABEL_WIDTH - ROW_GAP;
    ui.spacing_mut().interact_size.y = ROW_HEIGHT;
    let mut chosen = None;
    // Row 0, the sheet: its arrows walk the sheets as a key row does.
    if at.is_some_and(|(row, _)| row == 0) {
        if let Some(back) = arrow {
            let mut row = qnc_key_row::KeyRow::new((0..overlay.sheets.list.len()).map(|index| index.to_string()));
            row.select(&overlay.sheets.chosen.to_string());
            chosen = row.choose(back).and_then(|index| index.parse::<usize>().ok()).filter(|index| *index != overlay.sheets.chosen);
        }
    }
    let sheet_row = ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("sheet.sheet"));
        let shown = overlay.sheets.current().map_or("—".to_string(), |sheet| sheet.name.clone());
        egui::ComboBox::from_id_salt("qnc_story_sheet_choice").selected_text(shown).width(field_w * 0.6).show_ui(ui, |ui| {
            for (index, (sheet, _)) in overlay.sheets.list.iter().enumerate() {
                if ui.selectable_label(index == overlay.sheets.chosen, &sheet.name).clicked() {
                    chosen = Some(index);
                }
            }
        });
        if button(ui, qnc_ui_text::text("sheet.edit"), overlay.sheets.current().is_some(), theme) {
            overlay.editing = overlay.sheets.current().cloned();
        }
    });
    qnc_ui_kit::paint_focus(ui, sheet_row.response.rect.expand(2.0), at.is_some_and(|(row, _)| row == 0), theme.focus);
    if let Some(index) = chosen {
        if let Err(error) = overlay.sheets.choose(index) {
            overlay.message = error;
        }
    }
    ui.add_space(ROW_GAP);
    // Row 1, the project name.
    let name_id = egui::Id::new(NAME_ID);
    let name_row = ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("project.project_name"));
        let edit = qnc_ui_kit::row_field(&mut overlay.name, field_w, ROW_HEIGHT).id(name_id);
        ui.add_enabled(overlay.name_editable, edit.hint_text(qnc_ui_text::text("project.name_new_project")));
    });
    // The keyboard is given after the box is drawn: given before, the box itself takes the Enter
    // that brought it there and a one-line box lets the keyboard go at Enter (the cursor gone).
    if at == Some((1, true)) {
        ui.memory_mut(|memory| memory.request_focus(name_id));
    }
    qnc_ui_kit::paint_focus(ui, name_row.response.rect.expand(2.0), at.is_some_and(|(row, _)| row == 1), theme.focus);
    let mut typed = ui.memory(|memory| memory.has_focus(name_id)).then_some(1);
    ui.add_space(ROW_GAP / 2.0);
    // Rows 2.., the sheet's fields.
    if let Some(sheet) = overlay.sheets.current().cloned() {
        let keys = qnc_settings_form::ColumnKeys { at: at.and_then(|(row, _)| row.checked_sub(2)), arrived: at.is_some_and(|(_, arrived)| arrived), arrow, focus: theme.focus };
        let answer = qnc_settings_form::field_column(ui, width, &mut overlay.values, &sheet.fields, &sheet.options, &style(theme), keys);
        typed = typed.or(answer.typed.map(|row| row + 2));
    }
    if !overlay.message.is_empty() {
        ui.label(RichText::new(&overlay.message).color(theme.danger));
    }
    typed
}

/// Confirm (rightmost) and Cancel of the filled sheet; the one the keyboard is on is framed.
fn fill_buttons(ui: &mut egui::Ui, overlay: &SheetOverlay, theme: &Theme, on_button: Option<&str>) -> Option<Answer> {
    let mut answer = None;
    right_row(ui, |ui| {
        let confirm = qnc_ui_kit::strong_button(ui, &overlay.confirm_label, ready(overlay), theme.accent, theme.chrome_control_height);
        if confirm.clicked() {
            answer = Some(Answer::Confirm);
        }
        let cancel = qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("common.cancel"), true, false, qnc_theme_styles::button(theme));
        if cancel.clicked() {
            answer = Some(Answer::Cancel);
        }
        qnc_ui_kit::paint_focus(ui, confirm.rect.expand(2.0), on_button == Some(CONFIRM), theme.focus);
        qnc_ui_kit::paint_focus(ui, cancel.rect.expand(2.0), on_button == Some(CANCEL), theme.focus);
    });
    answer
}

const KINDS: [(FieldKind, &str); 4] = [
    (FieldKind::Text, "sheet.kind_text"),
    (FieldKind::Choice, "sheet.kind_choice"),
    (FieldKind::Duration, "sheet.kind_duration"),
    (FieldKind::Integer, "sheet.kind_number"),
];

/// The editor of the sheet: its name, the lock, each field (name, kind, choices, order,
/// removal) and a new field; Save sheet keeps it on this computer, Back leaves it as it was.
fn editor(ui: &mut egui::Ui, overlay: &mut SheetOverlay, theme: &Theme) {
    let width = ui.available_width();
    ui.spacing_mut().interact_size.y = ROW_HEIGHT;
    ui.spacing_mut().item_spacing.y = ROW_GAP / 2.0;
    let Some(sheet) = overlay.editing.as_mut() else { return };
    ui.horizontal(|ui| {
        label(ui, theme, qnc_ui_text::text("sheet.name"));
        ui.add_enabled(!sheet.locked, row_box(&mut sheet.name, width - LABEL_WIDTH - ROW_GAP));
    });
    qnc_ui_kit::check_box(ui, &mut sheet.locked, qnc_ui_text::text("sheet.locked"), true);
    ui.add_space(ROW_GAP);
    fields_editor(ui, "qnc_story_sheet", &mut sheet.fields, &mut sheet.options, &mut overlay.new_label, sheet.locked, theme);
    if !overlay.message.is_empty() {
        ui.label(RichText::new(&overlay.message).color(theme.danger));
    }
}

/// The editor of a list of fields (user 2026-10-28: the same for a story sheet and for the fields
/// of a kind of script block): each field's name, kind, choices, order and removal, and a new
/// field whose code comes from its name. Nothing changes while `locked`.
pub fn fields_editor(
    ui: &mut egui::Ui,
    salt: &str,
    fields: &mut Vec<qnc_settings_form::FieldSpec>,
    options: &mut qnc_settings_form::OptionSets,
    new_label: &mut String,
    locked: bool,
    theme: &Theme,
) {
    let width = ui.available_width();
    let field_label_w = width * 0.4;
    let (mut up, mut down, mut gone) = (None, None, None);
    let count = fields.len();
    for (index, field) in fields.iter_mut().enumerate() {
        ui.add_enabled_ui(!locked, |ui| {
            ui.horizontal(|ui| {
                ui.add(row_box(&mut field.label, field_label_w));
                let kind = KINDS.iter().find(|(kind, _)| *kind == field.kind).map_or("", |(_, code)| qnc_ui_text::text(code));
                egui::ComboBox::from_id_salt((salt, "kind", index)).selected_text(kind).width(110.0).show_ui(ui, |ui| {
                    for (kind, code) in KINDS {
                        ui.selectable_value(&mut field.kind, kind, qnc_ui_text::text(code));
                    }
                });
                up = up.or((button(ui, "▲", index > 0, theme)).then_some(index));
                down = down.or((button(ui, "▼", index + 1 < count, theme)).then_some(index));
                gone = gone.or((button(ui, qnc_ui_text::text("sheet.remove"), true, theme)).then_some(index));
            });
            if field.kind == FieldKind::Choice {
                choices_row(ui, field, options, width);
            }
        });
    }
    if let Some(index) = up {
        fields.swap(index, index - 1);
    }
    if let Some(index) = down {
        fields.swap(index, index + 1);
    }
    if let Some(index) = gone {
        fields.remove(index);
    }
    ui.add_enabled_ui(!locked, |ui| {
        ui.horizontal(|ui| {
            ui.add(row_box(new_label, field_label_w).hint_text(qnc_ui_text::text("sheet.new_field_hint")));
            if button(ui, qnc_ui_text::text("sheet.add_field"), !new_label.trim().is_empty(), theme) {
                let field = new_field_in(fields, new_label);
                fields.push(field);
                new_label.clear();
            }
        });
    });
}

/// Save sheet (rightmost) and Back of the editor; the overlay stays open either way.
fn editor_buttons(ui: &mut egui::Ui, overlay: &mut SheetOverlay, theme: &Theme) -> Option<Answer> {
    let (mut keep, mut back) = (false, false);
    right_row(ui, |ui| {
        keep = qnc_ui_kit::strong_button(ui, qnc_ui_text::text("sheet.save"), true, theme.accent, theme.chrome_control_height).clicked();
        back = button(ui, qnc_ui_text::text("sheet.back"), true, theme);
    });
    if keep {
        if let Some(Err(error)) = overlay.editing.take().map(|sheet| overlay.sheets.keep(sheet)) {
            overlay.message = error;
        }
    } else if back {
        overlay.editing = None;
    }
    None
}

/// The choices of a choice field, typed as names separated by commas.
fn choices_row(ui: &mut egui::Ui, field: &mut qnc_settings_form::FieldSpec, options: &mut qnc_settings_form::OptionSets, width: f32) {
    let key = field.options.clone().unwrap_or_else(|| field.path.clone());
    let mut typed = options.get(&key).map(|choices| choices.iter().map(|(_, label)| label.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add_space(24.0);
        let edited = ui.add(row_box(&mut typed, width - 24.0).hint_text(qnc_ui_text::text("sheet.choices_hint"))).changed();
        if edited || field.options.is_none() {
            let choices = typed.split(',').map(str::trim).filter(|label| !label.is_empty()).map(|label| (code_of(label), label.to_string())).collect();
            options.insert(key.clone(), choices);
            field.options = Some(key);
        }
    });
}

