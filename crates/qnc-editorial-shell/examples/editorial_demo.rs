//! Visual check of the three editorial modules with fake data.
//!
//! Composition comes from `contracts/ui/editorial.layout.json` for the chosen
//! group, colours and chrome metrics from `contracts/ui/shell.layout.json`.
//! No application, database or player is involved.
//!
//!     cargo run -p qnc-editorial-shell --example editorial_demo -- e
//!
//! Groups: e, g, l (empty right panel) and o (segment panel placeholder).

use std::collections::{HashMap, HashSet};

use eframe::egui::{self, Color32, Sense, TextureHandle};
use qnc_editorial_shell::{
    content_panel, editorial_shell, media_column_monitor, preview, PreviewInput, ShellGeometry,
    ShellSide, ShellStyle,
};
use qnc_media_card::{
    show_card_grid, CardGridAction, CardGridInput, CardMetrics, CardRow, CardStyle,
    MediaCardFeatures, StatusDotsMode,
};
use qnc_media_pool_head::{
    show_head, LibraryTab, PoolHeadAction, PoolHeadInput, PoolHeadStyle,
};
use serde_json::Value;

const EDITORIAL: &str = include_str!("../../../contracts/ui/editorial.layout.json");
const SHELL: &str = include_str!("../../../contracts/ui/shell.layout.json");

fn f(value: &Value) -> f32 {
    value.as_f64().expect("number in contract") as f32
}

fn rgb(value: &Value) -> Color32 {
    let c = value.as_array().expect("colour array");
    Color32::from_rgb(
        c[0].as_u64().unwrap() as u8,
        c[1].as_u64().unwrap() as u8,
        c[2].as_u64().unwrap() as u8,
    )
}

struct Row {
    id: String,
    title: String,
    seconds: f64,
    status: &'static str,
}

struct Demo {
    group: String,
    geometry: ShellGeometry,
    shell_style: ShellStyle,
    head_style: PoolHeadStyle,
    card_style: CardStyle,
    card_metrics: CardMetrics,
    features: MediaCardFeatures,
    head_flags: PoolHeadInput,
    empty_label: String,
    empty_font_size: f32,
    right_panel: String,
    face: Color32,
    block_pad: i8,
    tab: LibraryTab,
    playing: bool,
    selected: String,
    checked: HashSet<String>,
    rows: Vec<Row>,
    last_action: String,
}

impl Demo {
    fn new(group: &str) -> Self {
        let editorial: Value = serde_json::from_str(EDITORIAL).expect("editorial contract");
        let shell: Value = serde_json::from_str(SHELL).expect("shell contract");
        let composition = &editorial["groups"][group];
        assert!(composition.is_object(), "unknown group '{group}' (use e, g, l or o)");

        let metrics = &shell["theme_metrics"];
        let colors = &shell["colors"];
        let (bg, surface, raised, border, text, muted, accent) = (
            rgb(&colors["bg"]),
            rgb(&colors["surface"]),
            rgb(&colors["raised"]),
            rgb(&colors["border"]),
            rgb(&colors["text"]),
            rgb(&colors["muted"]),
            rgb(&colors["accent"]),
        );
        let chrome_row_height = f(&metrics["chrome_row_height"]);

        let geometry = ShellGeometry {
            left_ratio: f(&editorial["shell"]["left_ratio"]),
            divider_width: f(&editorial["shell"]["divider_width"]),
            left_min_width: f(&editorial["shell"]["left_min_width"]),
            right_min_width: f(&editorial["shell"]["right_min_width"]),
            chrome_row_height,
            preview_reserve_below: f(&editorial["preview"]["reserve_below"]),
            preview_min_height: f(&editorial["preview"]["min_height"]),
            preview_width_inset: f(&editorial["preview"]["width_inset"]),
            preview_min_width: f(&editorial["preview"]["min_width"]),
        };
        let pool = &composition["pool_head"];
        let flag = |name: &str| pool[name].as_bool().expect("pool_head flag");
        let card = &composition["media_card"];

        Self {
            group: group.to_string(),
            geometry,
            shell_style: ShellStyle {
                bg,
                left_face: surface,
                border,
                // Demo-only: the contract names this face but gives no value.
                preview_black: Color32::BLACK,
                muted,
            },
            head_style: PoolHeadStyle {
                chrome_fill: surface,
                border,
                text,
                muted,
                accent,
                font_ui: f(&metrics["font_ui"]),
                chrome_row_height,
                chrome_control_height: f(&metrics["chrome_control_height"]),
                chrome_pad_x: metrics["chrome_pad_x"].as_i64().unwrap() as i8,
                chrome_pad_y: metrics["chrome_pad_y"].as_i64().unwrap() as i8,
                tab_gap: 10.0,
                transport_button_width: 40.0,
                export_button_width: 120.0,
            },
            card_style: CardStyle {
                raised,
                surface,
                border,
                text,
                muted,
                // Demo-only: qnc_v4 SELECT_RED.
                select_red: Color32::from_rgb(239, 68, 68),
            },
            card_metrics: CardMetrics {
                min_card_width: f(&editorial["media_card"]["min_card_width"]),
                card_text_height: f(&editorial["media_card"]["card_text_height"]),
                grid_gap: f(&editorial["media_card"]["grid_gap"]),
            },
            features: MediaCardFeatures {
                selection_check: card["selection_check"].as_bool().unwrap(),
                status_dots: StatusDotsMode::from_contract(card["status_dots"].as_str().unwrap())
                    .expect("status_dots value"),
            },
            head_flags: PoolHeadInput {
                library_tab: LibraryTab::All,
                playing: false,
                show_segment_tab: flag("show_segment_tab"),
                show_cover_tab: flag("show_cover_tab"),
                show_export_hires: flag("show_export_hires"),
                export_hires_pending: false,
                show_quick_cover: flag("show_quick_cover"),
            },
            empty_label: editorial["preview"]["empty_label"].as_str().unwrap().to_string(),
            empty_font_size: f(&editorial["preview"]["empty_font_size"]),
            right_panel: composition["right_panel"].as_str().unwrap().to_string(),
            face: bg,
            block_pad: editorial["shell"]["block_pad"].as_f64().unwrap() as i8,
            tab: LibraryTab::All,
            playing: false,
            selected: "clip-003".to_string(),
            checked: HashSet::new(),
            rows: (1..=30)
                .map(|i| Row {
                    id: format!("clip-{i:03}"),
                    title: format!("Izjava {i:03}.MXF"),
                    seconds: 20.0 + (i as f64 * 7.3) % 190.0,
                    status: ["detected", "queued", "imported", "error"][i % 4],
                })
                .collect(),
            last_action: "—".to_string(),
        }
    }
}

fn timecode(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

impl eframe::App for Demo {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.shell_style.bg))
            .show(ctx, |ui| {
                // Split the borrows: the shell callback needs shared style and
                // mutable state at the same time.
                let geometry = self.geometry;
                let shell_style = self.shell_style;
                let head_style = self.head_style;
                let card_style = self.card_style;
                let card_metrics = self.card_metrics;
                let features = self.features;
                let no_thumbs: HashMap<String, TextureHandle> = HashMap::new();
                let cards: Vec<CardRow<'_>> = self
                    .rows
                    .iter()
                    .map(|row| CardRow {
                        id: &row.id,
                        thumb_id: &row.id,
                        title: &row.title,
                        duration_sec: row.seconds,
                        duration_label: "",
                        import_status: row.status,
                        status_proxy: if row.status == "queued" { "pending" } else { "ready" },
                        status_original: "ready",
                        checked: self.checked.contains(&row.id),
                    })
                    .collect();

                let mut pool_action = PoolHeadAction::None;
                let mut grid_action: Option<CardGridAction> = None;
                let group = self.group.clone();
                let right_panel = self.right_panel.clone();
                let empty_label = self.empty_label.clone();
                let head_input = PoolHeadInput {
                    library_tab: self.tab,
                    playing: self.playing,
                    ..self.head_flags
                };
                let (face, block_pad, empty_font_size) =
                    (self.face, self.block_pad, self.empty_font_size);
                let selected = self.selected.clone();
                let last_action = self.last_action.clone();

                editorial_shell(ui, &geometry, &shell_style, |ui, m, side| match side {
                    ShellSide::Left => {
                        media_column_monitor(
                            ui,
                            m,
                            &geometry,
                            |ui, preview_h| {
                                preview(
                                    ui,
                                    &shell_style,
                                    PreviewInput {
                                        height: preview_h,
                                        texture: None,
                                        empty_label: &empty_label,
                                        empty_font_size,
                                        sense: Sense::hover(),
                                    },
                                );
                            },
                            |ui, _rest| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                pool_action = show_head(ui, &head_style, head_input);
                                let body = ui.available_height().max(0.0);
                                content_panel(ui, face, block_pad, body, |ui| {
                                    grid_action = show_card_grid(
                                        ui,
                                        &card_style,
                                        &card_metrics,
                                        &CardGridInput {
                                            height: body,
                                            selected_id: &selected,
                                            focused_id: "",
                                            panel_focused: false,
                                            cards: &cards,
                                            thumb_textures: &no_thumbs,
                                            tc: &timecode,
                                            features,
                                            empty_message: "Nema klipova.",
                                            id_salt: "editorial_demo_grid",
                                        },
                                    );
                                });
                            },
                        );
                    }
                    ShellSide::Right => {
                        if right_panel == "segment_panel" {
                            ui.add_space(12.0);
                            ui.colored_label(
                                shell_style.muted,
                                "Segment panel (modul jos ne postoji)",
                            );
                        }
                        ui.painter().text(
                            ui.max_rect().right_bottom() - egui::vec2(12.0, 12.0),
                            egui::Align2::RIGHT_BOTTOM,
                            format!("grupa {group} | zadnja radnja: {last_action}"),
                            egui::FontId::proportional(12.0),
                            shell_style.muted,
                        );
                    }
                });

                match pool_action {
                    PoolHeadAction::None => {}
                    PoolHeadAction::SwitchTab(tab) => {
                        self.tab = tab;
                        self.last_action = format!("tab {}", tab.label());
                    }
                    PoolHeadAction::TogglePlay => {
                        self.playing = !self.playing;
                        self.last_action = "play/pause".to_string();
                    }
                    other => self.last_action = format!("{other:?}"),
                }
                match grid_action {
                    Some(CardGridAction::Activate(id)) => {
                        self.last_action = format!("odabran {id}");
                        self.selected = id;
                    }
                    Some(CardGridAction::ToggleSelection(id)) => {
                        if !self.checked.remove(&id) {
                            self.checked.insert(id.clone());
                        }
                        self.last_action = format!("kvacica {id}");
                    }
                    None => {}
                }
            });
    }
}

fn main() -> eframe::Result<()> {
    let group = std::env::args().nth(1).unwrap_or_else(|| "e".to_string());
    let demo = Demo::new(&group);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0])
            .with_title(format!("QNC editorial demo (grupa {group})")),
        ..Default::default()
    };
    eframe::run_native(
        "QNC editorial demo",
        options,
        Box::new(move |_cc| Ok(Box::new(demo))),
    )
}
