use std::{
    path::Path,
    path::PathBuf,
    process,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use eframe::egui::{self, CentralPanel, Label, RichText, ScrollArea, TextWrapMode, Vec2};
use qnc_dev_diagnostics::{
    DiagnosticCheckId, DiagnosticTestReport, DiagnosticsSettings, DiagnosticsState,
    DiagnosticsStream,
};

const LOG_TEXT_SIZE: f32 = 14.0;
const LOG_HEADER_SIZE: f32 = 13.0;
const LOG_COLUMNS: &[(&str, &str)] = &[
    ("Vrijeme", "__timestamp"),
    ("Tip", "__event"),
    ("Clip", "clip"),
    ("Izvor", "source"),
    ("Mode", "mode"),
    ("Slicica", "frames"),
    ("Build", "build_ms"),
    ("Extract", "extract_ms"),
    ("Load", "load_ms"),
    ("Total", "total_ms"),
    ("Ready", "ready_before"),
    ("Publish", "publish"),
    ("Ostalo", "__remaining"),
];

fn main() -> eframe::Result<()> {
    let root = qnc_dev_diagnostics::locate_qnc_root().unwrap_or_else(|| {
        eprintln!("qnc-dev-diagnostics: QNC root nije pronadjen.");
        process::exit(1);
    });
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--dump-content-db") {
        print_content_db(&read_content_db(&root));
        process::exit(0);
    }
    if args.iter().any(|arg| arg == "--list-tests") {
        print_test_catalog();
        process::exit(0);
    }
    if let Some(index) = args
        .iter()
        .position(|arg| arg == "--run-tests" || arg == "--run-test")
    {
        let ids = match parse_test_ids(args.get(index + 1).map(String::as_str)) {
            Ok(ids) => ids,
            Err(error) => {
                eprintln!("{error}");
                process::exit(2);
            }
        };
        let report = qnc_dev_diagnostics::run_diagnostic_checks(&root, &ids);
        print_test_report(&report);
        process::exit(if report.success() { 0 } else { 1 });
    }
    let app = DiagnosticsApp::new(root);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("QNC Diagnostics")
            .with_inner_size([1640.0, 840.0])
            .with_min_inner_size([1180.0, 640.0]),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "QNC Diagnostics",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(app))
        }),
    )
}

fn parse_test_ids(value: Option<&str>) -> Result<Vec<DiagnosticCheckId>, String> {
    let Some(value) = value else {
        return Ok(qnc_dev_diagnostics::diagnostic_checks()
            .iter()
            .map(|check| check.id)
            .collect());
    };
    if value.trim().eq_ignore_ascii_case("all") {
        return Ok(qnc_dev_diagnostics::diagnostic_checks()
            .iter()
            .map(|check| check.id)
            .collect());
    }
    let mut ids = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some(id) = DiagnosticCheckId::from_id(part) else {
            return Err(format!("Nepoznata diagnostics provjera: {part}"));
        };
        ids.push(id);
    }
    if ids.is_empty() {
        return Err("Nije zadana nijedna diagnostics provjera.".into());
    }
    Ok(ids)
}

fn print_test_catalog() {
    for check in qnc_dev_diagnostics::diagnostic_checks() {
        println!(
            "{}\t{}\t{}",
            check.id.as_str(),
            check.title,
            check.description
        );
    }
}

fn print_test_report(report: &DiagnosticTestReport) {
    let passed = report
        .results
        .iter()
        .filter(|result| result.success)
        .count();
    println!(
        "SUMMARY {passed}/{} OK duration_ms={}",
        report.results.len(),
        report.duration_ms
    );
    for result in &report.results {
        println!(
            "{} {} duration_ms={} exit={}",
            if result.success { "OK" } else { "FAIL" },
            result.id.as_str(),
            result.duration_ms,
            result
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "n/a".into())
        );
        println!("COMMAND {}", result.command_line);
        println!("{}", result.output);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogTab {
    Filmstrip,
    Wave,
    Player,
    ContentDb,
    Tests,
}

#[derive(Debug, Clone, Default)]
struct ContentDbSnapshot {
    project_name: String,
    project_id: String,
    content_uri: String,
    clips: usize,
    waves: usize,
    filmstrips: usize,
    rows: Vec<ContentDbRow>,
    error: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ContentDbRow {
    clip_id: String,
    name: String,
    original_codec: String,
    proxy_codec: String,
    wave: String,
    filmstrip: String,
}

struct DiagnosticsApp {
    root: PathBuf,
    settings: DiagnosticsSettings,
    state: DiagnosticsState,
    player_log: Vec<String>,
    filmstrip_log: Vec<String>,
    wave_log: Vec<String>,
    content_db: ContentDbSnapshot,
    test_selected: Vec<bool>,
    test_result: Option<DiagnosticTestReport>,
    test_receiver: Option<Receiver<DiagnosticTestReport>>,
    active_log_tab: LogTab,
    status: String,
    last_refresh: Instant,
}

impl DiagnosticsApp {
    fn new(root: PathBuf) -> Self {
        let settings = qnc_dev_diagnostics::load_from_root(&root).unwrap_or_default();
        let mut app = Self {
            root,
            settings,
            state: qnc_dev_diagnostics::diagnostics_state(),
            player_log: Vec::new(),
            filmstrip_log: Vec::new(),
            wave_log: Vec::new(),
            content_db: ContentDbSnapshot::default(),
            test_selected: vec![false; qnc_dev_diagnostics::diagnostic_checks().len()],
            test_result: None,
            test_receiver: None,
            active_log_tab: LogTab::Filmstrip,
            status: "Spremno.".into(),
            last_refresh: Instant::now(),
        };
        app.load_logs();
        app.load_content_db();
        app
    }

    fn refresh(&mut self) {
        match qnc_dev_diagnostics::load_from_root(&self.root) {
            Ok(settings) => {
                self.settings = settings;
                self.status = "Stanje ucitano.".into();
            }
            Err(error) => self.status = error,
        }
        self.state = qnc_dev_diagnostics::diagnostics_state();
        self.load_logs();
        self.load_content_db();
        self.last_refresh = Instant::now();
    }

    fn save(&mut self) {
        match qnc_dev_diagnostics::save_to_root(&self.root, self.settings.clone()) {
            Ok(()) => {
                self.status = "Stanje spremljeno.".into();
                self.state = qnc_dev_diagnostics::diagnostics_state();
                self.load_logs();
            }
            Err(error) => self.status = error,
        }
    }

    fn load_logs(&mut self) {
        self.player_log =
            qnc_dev_diagnostics::recent_lines_from_root(&self.root, DiagnosticsStream::Player, 200)
                .unwrap_or_else(|error| vec![error]);
        self.filmstrip_log = qnc_dev_diagnostics::recent_lines_from_root(
            &self.root,
            DiagnosticsStream::Filmstrip,
            200,
        )
        .unwrap_or_else(|error| vec![error]);
        self.wave_log =
            qnc_dev_diagnostics::recent_lines_from_root(&self.root, DiagnosticsStream::Wave, 200)
                .unwrap_or_else(|error| vec![error]);
    }

    fn clear_log(&mut self, stream: DiagnosticsStream) {
        match qnc_dev_diagnostics::clear_log(&self.root, stream) {
            Ok(()) => {
                self.status = "Log ociscen.".into();
                self.load_logs();
            }
            Err(error) => self.status = error,
        }
    }

    fn load_content_db(&mut self) {
        self.content_db = read_content_db(&self.root);
    }

    fn selected_test_ids(&self) -> Vec<DiagnosticCheckId> {
        qnc_dev_diagnostics::diagnostic_checks()
            .iter()
            .enumerate()
            .filter_map(|(index, check)| {
                self.test_selected
                    .get(index)
                    .copied()
                    .unwrap_or(false)
                    .then_some(check.id)
            })
            .collect()
    }

    fn start_tests(&mut self, ids: Vec<DiagnosticCheckId>) {
        if self.test_receiver.is_some() {
            self.status = "Diagnostics provjera je vec pokrenuta.".into();
            return;
        }
        if ids.is_empty() {
            self.status = "Odaberi barem jednu provjeru.".into();
            return;
        }
        let root = self.root.clone();
        let (send, receive) = mpsc::channel();
        match thread::Builder::new()
            .name("qnc-dev-diagnostic-tests".into())
            .spawn(move || {
                let report = qnc_dev_diagnostics::run_diagnostic_checks(&root, &ids);
                let _ = send.send(report);
            }) {
            Ok(_) => {
                self.test_receiver = Some(receive);
                self.status = "Diagnostics provjera je pokrenuta.".into();
            }
            Err(error) => {
                self.status = format!("Pokretanje diagnostics provjere nije uspjelo: {error}")
            }
        }
    }

    fn poll_tests(&mut self) {
        let Some(receiver) = self.test_receiver.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(report) => {
                let passed = report
                    .results
                    .iter()
                    .filter(|result| result.success)
                    .count();
                let total = report.results.len();
                self.status = format!(
                    "Diagnostics provjera zavrsena: {passed}/{total} OK ({:.1} s).",
                    report.duration_ms as f64 / 1000.0
                );
                self.test_result = Some(report);
                self.test_receiver = None;
            }
            Err(TryRecvError::Disconnected) => {
                self.status = "Diagnostics provjera je prekinuta.".into();
                self.test_receiver = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn tests_panel(&mut self, ui: &mut egui::Ui, height: f32) {
        self.poll_tests();
        let running = self.test_receiver.is_some();
        ui.label(RichText::new("Diagnostics testovi").strong());
        ui.label("Provjere se pokrecu kao odvojeni cargo procesi iz QNC roota.");
        ui.add_space(8.0);

        let mut run_single = None;
        egui::Grid::new("diagnostic_checks_grid")
            .spacing(Vec2::new(14.0, 6.0))
            .show(ui, |ui| {
                log_cell(ui, "", true);
                log_cell(ui, "Provjera", true);
                log_cell(ui, "Opis", true);
                log_cell(ui, "Akcija", true);
                ui.end_row();
                for (index, check) in qnc_dev_diagnostics::diagnostic_checks().iter().enumerate() {
                    if index >= self.test_selected.len() {
                        self.test_selected.push(false);
                    }
                    ui.add_enabled_ui(!running, |ui| {
                        ui.checkbox(&mut self.test_selected[index], "");
                    });
                    log_cell(ui, check.title, false);
                    log_cell(ui, check.description, false);
                    if ui
                        .add_enabled(
                            !running,
                            egui::Button::new("Pokreni").min_size(Vec2::new(86.0, 26.0)),
                        )
                        .clicked()
                    {
                        run_single = Some(check.id);
                    }
                    ui.end_row();
                }
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !running,
                    egui::Button::new("Pokreni odabrane").min_size(Vec2::new(142.0, 28.0)),
                )
                .clicked()
            {
                self.start_tests(self.selected_test_ids());
            }
            if ui
                .add_enabled(
                    !running,
                    egui::Button::new("Pokreni sve").min_size(Vec2::new(110.0, 28.0)),
                )
                .clicked()
            {
                self.start_tests(
                    qnc_dev_diagnostics::diagnostic_checks()
                        .iter()
                        .map(|check| check.id)
                        .collect(),
                );
            }
            if ui
                .add_enabled(
                    self.test_result.is_some() && !running,
                    egui::Button::new("Ocisti rezultat").min_size(Vec2::new(126.0, 28.0)),
                )
                .clicked()
            {
                self.test_result = None;
            }
            if running {
                ui.label("Izvrsavanje u tijeku...");
            }
        });
        if let Some(id) = run_single {
            self.start_tests(vec![id]);
        }

        ui.add_space(10.0);
        test_result_panel(
            ui,
            "diagnostic_test_result_scroll",
            self.test_result.as_ref(),
            height,
        );
    }
}

impl eframe::App for DiagnosticsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_tests();
        if self.last_refresh.elapsed() >= Duration::from_secs(1) {
            self.state = qnc_dev_diagnostics::diagnostics_state();
            self.load_logs();
            self.last_refresh = Instant::now();
        }
        CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("QNC Diagnostics");
            ui.add_space(8.0);
            ui.label(format!("Root: {}", self.root.display()));
            ui.label(format!(
                "Config: {}",
                qnc_dev_diagnostics::config_path(&self.root).display()
            ));
            ui.add_space(14.0);

            let mut changed = false;
            changed |= ui
                .checkbox(
                    &mut self.settings.player_diagnostics,
                    "Player diagnostics i mjerenja",
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut self.settings.filmstrip_diagnostics,
                    "Filmstrip worker diagnostics i mjerenja",
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut self.settings.wave_diagnostics,
                    "Wave worker diagnostics i mjerenja",
                )
                .changed();
            if changed {
                self.save();
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);
            ui.label(RichText::new("Efektivno stanje").strong());
            ui.label(status_line(
                "Player",
                self.state.effective_player_diagnostics,
                self.state.player_env_override,
            ));
            ui.label(status_line(
                "Filmstrip",
                self.state.effective_filmstrip_diagnostics,
                self.state.filmstrip_env_override,
            ));
            ui.label(status_line(
                "Wave",
                self.state.effective_wave_diagnostics,
                self.state.wave_env_override,
            ));
            if let Some(error) = &self.state.read_error {
                ui.label(RichText::new(error).color(egui::Color32::from_rgb(255, 170, 120)));
            }

            ui.add_space(10.0);
            let log_height = (ui.available_height() - 46.0).max(260.0);
            ui.horizontal(|ui| {
                log_tab_button(
                    ui,
                    &mut self.active_log_tab,
                    LogTab::Filmstrip,
                    "Filmstrip log",
                );
                log_tab_button(ui, &mut self.active_log_tab, LogTab::Player, "Player log");
                log_tab_button(ui, &mut self.active_log_tab, LogTab::Wave, "Wave log");
                log_tab_button(
                    ui,
                    &mut self.active_log_tab,
                    LogTab::ContentDb,
                    "Content DB",
                );
                log_tab_button(ui, &mut self.active_log_tab, LogTab::Tests, "Tests");
            });
            ui.add_space(6.0);

            let active_stream = match self.active_log_tab {
                LogTab::Filmstrip => {
                    log_panel(
                        ui,
                        "active_log_scroll",
                        "Filmstrip log",
                        &self.filmstrip_log,
                        log_height,
                        self.settings.filmstrip_diagnostics,
                        true,
                    );
                    Some(DiagnosticsStream::Filmstrip)
                }
                LogTab::Player => {
                    log_panel(
                        ui,
                        "active_log_scroll",
                        "Player log",
                        &self.player_log,
                        log_height,
                        self.settings.player_diagnostics,
                        false,
                    );
                    Some(DiagnosticsStream::Player)
                }
                LogTab::Wave => {
                    log_panel(
                        ui,
                        "active_log_scroll",
                        "Wave log",
                        &self.wave_log,
                        log_height,
                        self.settings.wave_diagnostics,
                        false,
                    );
                    Some(DiagnosticsStream::Wave)
                }
                LogTab::ContentDb => {
                    content_db_panel(ui, "content_db_scroll", &self.content_db, log_height);
                    None
                }
                LogTab::Tests => {
                    self.tests_panel(ui, log_height);
                    None
                }
            };

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(Vec2::new(96.0, 28.0), egui::Button::new("Ucitaj"))
                    .clicked()
                {
                    self.refresh();
                }
                if ui
                    .add_enabled(
                        active_stream.is_some(),
                        egui::Button::new("Ocisti log").min_size(Vec2::new(120.0, 28.0)),
                    )
                    .clicked()
                    && active_stream.is_some()
                {
                    self.clear_log(active_stream.unwrap());
                }
                ui.label(&self.status);
            });
        });
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn log_tab_button(ui: &mut egui::Ui, active_tab: &mut LogTab, tab: LogTab, title: &str) {
    if ui.selectable_label(*active_tab == tab, title).clicked() {
        *active_tab = tab;
    }
}

fn log_panel(
    ui: &mut egui::Ui,
    id: &'static str,
    title: &str,
    lines: &[String],
    height: f32,
    enabled: bool,
    combine_filmstrip: bool,
) {
    ui.label(RichText::new(title).strong());
    let state = if enabled { "ON" } else { "OFF" };
    ui.label(format!("Mjerenje: {state}"));
    let rows = if combine_filmstrip {
        filmstrip_rows(lines)
    } else {
        generic_rows(lines)
    };
    ScrollArea::both()
        .id_salt(id)
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            if rows.is_empty() {
                ui.label("Nema zapisa.");
            } else {
                egui::Grid::new((id, "grid"))
                    .spacing(Vec2::new(18.0, 4.0))
                    .show(ui, |ui| {
                        for (title, _) in LOG_COLUMNS {
                            log_cell(ui, title, true);
                        }
                        ui.end_row();
                        for row in &rows {
                            for (_, key) in LOG_COLUMNS {
                                log_cell(ui, &row.value(key), false);
                            }
                            ui.end_row();
                        }
                    });
            }
        });
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogRow {
    timestamp: String,
    event: String,
    fields: Vec<(String, String)>,
    remaining: String,
}

impl LogRow {
    fn value(&self, key: &str) -> String {
        match key {
            "__timestamp" => self.timestamp.clone(),
            "__event" => self.display_event(),
            "__remaining" => self.remaining_fields(),
            _ => self.field(key).unwrap_or_default().to_string(),
        }
    }

    fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value.as_str())
    }

    fn merge(&mut self, other: LogRow) {
        if !other.timestamp.is_empty() {
            self.timestamp = other.timestamp;
        }
        if self.event == "qnc-filmstrip-worker" && other.event == "qnc-filmstrip-worker-total" {
            self.event = "filmstrip".into();
        }
        for (key, value) in other.fields {
            if let Some((_, existing)) = self
                .fields
                .iter_mut()
                .find(|(candidate, _)| candidate == &key)
            {
                *existing = value;
            } else {
                self.fields.push((key, value));
            }
        }
        if !other.remaining.is_empty() {
            if self.remaining.is_empty() {
                self.remaining = other.remaining;
            } else {
                self.remaining.push(' ');
                self.remaining.push_str(&other.remaining);
            }
        }
    }

    fn display_event(&self) -> String {
        match self.event.as_str() {
            "qnc-filmstrip-worker" | "qnc-filmstrip-worker-total" | "filmstrip" => {
                "filmstrip".into()
            }
            value => value.to_string(),
        }
    }

    fn remaining_fields(&self) -> String {
        let mut values = Vec::new();
        if !self.remaining.is_empty() {
            values.push(self.remaining.clone());
        }
        for (key, value) in &self.fields {
            if !LOG_COLUMNS.iter().any(|(_, column_key)| column_key == key) {
                values.push(format!("{key}={value}"));
            }
        }
        values.join(" ")
    }
}

fn generic_rows(lines: &[String]) -> Vec<LogRow> {
    lines.iter().map(|line| parse_log_row(line)).collect()
}

fn filmstrip_rows(lines: &[String]) -> Vec<LogRow> {
    let mut rows: Vec<LogRow> = Vec::new();
    for line in lines {
        let row = parse_log_row(line);
        if row.event == "qnc-filmstrip-worker-total" {
            if let Some(existing) = rows.iter_mut().rfind(|existing| {
                existing.event == "qnc-filmstrip-worker"
                    && existing.field("clip") == row.field("clip")
                    && existing.field("total_ms").is_none()
            }) {
                existing.merge(row);
                continue;
            }
        }
        rows.push(row);
    }
    rows
}

fn parse_log_row(line: &str) -> LogRow {
    let trimmed = line.trim();
    let (timestamp, rest) = trimmed.split_once(' ').unwrap_or((trimmed, ""));
    let (event, payload) = rest.split_once(':').unwrap_or(("", rest));
    let mut fields = Vec::new();
    let mut remaining = Vec::new();
    for token in payload.split_whitespace() {
        if let Some((key, value)) = token.split_once('=') {
            fields.push((key.trim().to_string(), value.trim().to_string()));
        } else if !token.trim().is_empty() {
            remaining.push(token.trim().to_string());
        }
    }
    LogRow {
        timestamp: timestamp.to_string(),
        event: event.trim().to_string(),
        fields,
        remaining: remaining.join(" "),
    }
}

fn log_cell(ui: &mut egui::Ui, text: &str, header: bool) {
    let color = if header {
        egui::Color32::from_rgb(220, 225, 232)
    } else {
        egui::Color32::from_rgb(205, 210, 218)
    };
    let size = if header {
        LOG_HEADER_SIZE
    } else {
        LOG_TEXT_SIZE
    };
    let mut label = RichText::new(text).monospace().size(size).color(color);
    if header {
        label = label.strong();
    }
    ui.add(Label::new(label).wrap_mode(TextWrapMode::Extend));
}

fn content_db_panel(
    ui: &mut egui::Ui,
    id: &'static str,
    snapshot: &ContentDbSnapshot,
    height: f32,
) {
    ui.label(RichText::new("Content DB").strong());
    if let Some(error) = &snapshot.error {
        ui.label(RichText::new(error).color(egui::Color32::from_rgb(255, 170, 120)));
        return;
    }
    ui.label(format!(
        "Projekt: {}  |  clips={} wave={} filmstrip={}",
        snapshot.project_name, snapshot.clips, snapshot.waves, snapshot.filmstrips
    ));
    ui.label(format!(
        "Project ID: {}  |  Content URI: {}",
        snapshot.project_id, snapshot.content_uri
    ));
    ui.add_space(8.0);
    ScrollArea::both()
        .id_salt(id)
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            egui::Grid::new((id, "grid"))
                .spacing(Vec2::new(24.0, 4.0))
                .show(ui, |ui| {
                    log_cell(ui, "Clip", true);
                    log_cell(ui, "Naziv", true);
                    log_cell(ui, "Original", true);
                    log_cell(ui, "Proxy", true);
                    log_cell(ui, "Wave", true);
                    log_cell(ui, "Filmstrip", true);
                    ui.end_row();
                    for row in &snapshot.rows {
                        log_cell(ui, &row.clip_id, false);
                        log_cell(ui, &row.name, false);
                        log_cell(ui, &row.original_codec, false);
                        log_cell(ui, &row.proxy_codec, false);
                        log_cell(ui, &row.wave, false);
                        log_cell(ui, &row.filmstrip, false);
                        ui.end_row();
                    }
                });
            if snapshot.rows.is_empty() {
                ui.label("Nema clip zapisa.");
            }
        });
}

fn test_result_panel(
    ui: &mut egui::Ui,
    id: &'static str,
    report: Option<&DiagnosticTestReport>,
    height: f32,
) {
    ScrollArea::both()
        .id_salt(id)
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            let Some(report) = report else {
                ui.label("Nema pokrenutih provjera.");
                return;
            };
            ui.label(format!(
                "Trajanje: {:.1} s | Rezultat: {}",
                report.duration_ms as f64 / 1000.0,
                if report.success() { "OK" } else { "FAIL" }
            ));
            ui.add_space(8.0);
            for result in &report.results {
                let state = if result.success { "OK" } else { "FAIL" };
                let title = format!(
                    "{state} | {} | {:.1} s",
                    result.title,
                    result.duration_ms as f64 / 1000.0
                );
                egui::CollapsingHeader::new(title)
                    .default_open(!result.success)
                    .show(ui, |ui| {
                        ui.label(RichText::new(&result.command_line).monospace());
                        ui.label(format!(
                            "Exit: {}",
                            result
                                .exit_code
                                .map(|code| code.to_string())
                                .unwrap_or_else(|| "n/a".into())
                        ));
                        ui.add_space(4.0);
                        ui.add(
                            Label::new(
                                RichText::new(&result.output)
                                    .monospace()
                                    .size(LOG_TEXT_SIZE)
                                    .color(egui::Color32::from_rgb(205, 210, 218)),
                            )
                            .wrap_mode(TextWrapMode::Extend),
                        );
                    });
                ui.add_space(6.0);
            }
        });
}

fn read_content_db(root: &Path) -> ContentDbSnapshot {
    match read_content_db_inner(root) {
        Ok(snapshot) => snapshot,
        Err(error) => ContentDbSnapshot {
            error: Some(error),
            ..Default::default()
        },
    }
}

fn read_content_db_inner(root: &Path) -> Result<ContentDbSnapshot, String> {
    let reader = qnc_work_settings::SettingsReader::from_root(root).map_err(|e| e.to_string())?;
    let settings = reader.read().map_err(|e| e.to_string())?;
    let plan = qnc_ingest_work_plan::IngestWorkPlan::from_settings(settings)?;
    let content_uri = qnc_ingest_store::content::content_uri(&plan.settings.workspace_db_uri)?;
    let target = qnc_ingest_store::content::ContentTarget::for_project(&reader, &plan.settings)?;
    let mut client = target.open(qnc_ingest_store::content::Access::ReadOnly)?;
    let mut snapshot = ContentDbSnapshot {
        project_name: plan.settings.project_name.clone(),
        project_id: plan.settings.project_id.clone(),
        content_uri,
        ..Default::default()
    };
    let mut after = None;
    loop {
        let page = client.list(after.clone())?;
        if page.is_empty() {
            break;
        }
        after = page.last().map(|clip| clip.clip.id().to_string());
        for clip in page {
            let wave = client.read_wave(clip.clip.id())?;
            let filmstrip = client.read_filmstrip(clip.clip.id())?;
            if wave.is_some() {
                snapshot.waves += 1;
            }
            if filmstrip.is_some() {
                snapshot.filmstrips += 1;
            }
            snapshot.rows.push(ContentDbRow {
                clip_id: clip.clip.id().to_string(),
                name: clip.clip.name.clone(),
                original_codec: media_codec(&clip.clip.snapshot.metadata.original),
                proxy_codec: clip
                    .clip
                    .snapshot
                    .metadata
                    .proxy
                    .as_ref()
                    .map(media_codec)
                    .unwrap_or_else(|| "-".into()),
                wave: wave
                    .map(|record| {
                        format!(
                            "{} peaks={} a1={} a2={} a3={} a4={}",
                            record.status,
                            record.peak_count,
                            record.a1_peaks.len(),
                            record.a2_peaks.len(),
                            record.a3_peaks.len(),
                            record.a4_peaks.len()
                        )
                    })
                    .unwrap_or_else(|| "missing".into()),
                filmstrip: filmstrip
                    .map(|record| format!("{} frames={}", record.status, record.frame_count))
                    .unwrap_or_else(|| "missing".into()),
            });
            snapshot.clips += 1;
        }
    }
    Ok(snapshot)
}

fn print_content_db(snapshot: &ContentDbSnapshot) {
    if let Some(error) = &snapshot.error {
        println!("ERROR {error}");
        return;
    }
    println!(
        "SUMMARY project={} project_id={} content_uri={} clips={} waves={} filmstrips={}",
        snapshot.project_name,
        snapshot.project_id,
        snapshot.content_uri,
        snapshot.clips,
        snapshot.waves,
        snapshot.filmstrips
    );
    for row in &snapshot.rows {
        println!(
            "{}\t{}\toriginal={}\tproxy={}\twave={}\tfilmstrip={}",
            row.clip_id, row.name, row.original_codec, row.proxy_codec, row.wave, row.filmstrip
        );
    }
}

fn media_codec(media: &qnc_media_metadata::MediaRepresentation) -> String {
    media
        .streams
        .iter()
        .find_map(|stream| match (&stream.details, &stream.codec) {
            (qnc_media_metadata::StreamDetails::Video(_), Some(codec)) => match &codec.value {
                qnc_media_metadata::Signal::Known(value) => Some(value.clone()),
                qnc_media_metadata::Signal::Unspecified => Some("unspecified".into()),
            },
            _ => None,
        })
        .unwrap_or_else(|| "missing".into())
}

fn status_line(name: &str, enabled: bool, env_override: Option<bool>) -> String {
    let state = if enabled { "ON" } else { "OFF" };
    match env_override {
        Some(true) => format!("{name}: {state} (env override ON)"),
        Some(false) => format!("{name}: {state} (env override OFF)"),
        None => format!("{name}: {state}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filmstrip_rows_merge_worker_and_total_for_same_clip() {
        let lines = vec![
            "1 qnc-filmstrip-worker: clip=clip-1 source=Proxy mode=KeyframeSeek frames=13 build_ms=934"
                .to_string(),
            "2 qnc-filmstrip-worker-total: clip=clip-1 source=Proxy mode=KeyframeSeek ready_before=false extract_ms=951 publish=queued load_ms=453 total_ms=1460"
                .to_string(),
        ];
        let rows = filmstrip_rows(&lines);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value("__event"), "filmstrip");
        assert_eq!(rows[0].value("frames"), "13");
        assert_eq!(rows[0].value("build_ms"), "934");
        assert_eq!(rows[0].value("extract_ms"), "951");
        assert_eq!(rows[0].value("total_ms"), "1460");
    }

    #[test]
    fn parse_log_row_keeps_unknown_payload() {
        let row = parse_log_row("1 plain message without structured values");
        assert_eq!(row.value("__timestamp"), "1");
        assert_eq!(
            row.value("__remaining"),
            "plain message without structured values"
        );
    }

    #[test]
    fn cli_test_parser_accepts_all_and_comma_list() {
        let all = parse_test_ids(Some("all")).unwrap();
        assert_eq!(all.len(), qnc_dev_diagnostics::diagnostic_checks().len());

        let selected = parse_test_ids(Some("ingest_core,conformance")).unwrap();
        assert_eq!(
            selected,
            vec![
                DiagnosticCheckId::IngestCore,
                DiagnosticCheckId::Conformance
            ]
        );
        assert!(parse_test_ids(Some("missing")).is_err());
    }
}
