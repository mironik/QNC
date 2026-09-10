use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process,
};

use qnc_contracts::{
    contains_hardcoded_shortcut_pattern, parse_qnc_uri, validate_application_manifest_json,
    validate_keyboard_catalog_json, validate_module_manifest_json,
    validate_ui_layout_contract_json, validate_ui_layout_reference_doc, ValidationReport,
};
use qnc_db_contract::validate_db_contract_json;

mod player_boundary;

struct CheckResult {
    name: String,
    report: ValidationReport,
}

impl CheckResult {
    fn ok(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            report: ValidationReport::new(),
        }
    }

    fn from_report(name: impl Into<String>, report: ValidationReport) -> Self {
        Self {
            name: name.into(),
            report,
        }
    }

    fn error(name: impl Into<String>, message: impl Into<String>) -> Self {
        let mut report = ValidationReport::new();
        report.error(message);
        Self {
            name: name.into(),
            report,
        }
    }
}

fn main() {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("current dir"));

    let checks = run_checks(&root);
    let mut failed = 0usize;

    println!("QNC conformance root: {}", root.display());
    for check in &checks {
        if check.report.is_ok() {
            if check.report.warnings.is_empty() {
                println!("OK   {}", check.name);
            } else {
                println!("WARN {}", check.name);
                for warning in &check.report.warnings {
                    println!("     - {warning}");
                }
            }
        } else {
            failed += 1;
            println!("FAIL {}", check.name);
            for error in &check.report.errors {
                println!("     - {error}");
            }
            for warning in &check.report.warnings {
                println!("     - warning: {warning}");
            }
        }
    }

    if failed > 0 {
        println!("QNC conformance: {failed} failed check(s)");
        process::exit(1);
    }

    println!("QNC conformance: all checks passed");
}

fn run_checks(root: &Path) -> Vec<CheckResult> {
    let mut checks = vec![
        require_file(root, "AGENTS.md"),
        require_file(root, "apps/qnc-app/Cargo.toml"),
        require_file(root, "apps/qnc-app/src/main.rs"),
        require_file(root, "crates/qnc-project-desktop/src/lib.rs"),
        require_file(root, "crates/qnc-project-store/src/lib.rs"),
        require_file(root, "crates/qnc-project-desktop-adapter/src/lib.rs"),
        require_file(root, "crates/qnc-ui-kit/src/lib.rs"),
        require_file(root, "seed/system_seed.json"),
        require_file(root, "contracts/qnc-keyboard-shortcuts.json"),
        require_file(root, "docs/07-ui-layout-reference.md"),
        require_file(root, "docs/11-project-freeze.md"),
        validate_project_freeze_policy(root),
        validate_project_seed(root),
        validate_keyboard_catalog(root),
        validate_keyboard_matches_qnc_v4(root),
        validate_ingest_keyboard_actions(root),
        validate_ui_layout_contract(root),
        validate_app_registry(root),
    ];
    checks.extend(validate_json_manifests(
        root,
        "contracts/modules",
        validate_module_manifest_json,
    ));
    checks.extend(validate_json_manifests(
        root,
        "contracts/applications",
        validate_application_manifest_json,
    ));
    checks.extend(validate_json_manifests(
        root,
        "contracts/databases",
        validate_db_contract_json,
    ));
    checks.extend(validate_json_manifests(
        root,
        "contracts/ui",
        validate_ui_layout_contract_json,
    ));
    checks.push(validate_manifest_graph(root));
    checks.push(validate_sample_qnc_uris());
    checks.push(scan_rust_for_hardcoded_shortcuts(root));
    checks.push(scan_business_app_isolation(root));
    checks.push(scan_shell_app_boundary(root));
    checks.push(scan_project_app_boundary(root));
    checks.push(scan_shared_ui_patterns(root));
    checks.push(CheckResult::from_report(
        "public player boundary",
        player_boundary::check(root),
    ));

    checks
}

fn require_file(root: &Path, relative: &str) -> CheckResult {
    let path = root.join(relative);
    if path.is_file() {
        CheckResult::ok(format!("required file {relative}"))
    } else {
        CheckResult::error(
            format!("required file {relative}"),
            format!("missing required file: {}", path.display()),
        )
    }
}

fn validate_project_freeze_policy(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let agents_path = root.join("AGENTS.md");
    let freeze_path = root.join("docs").join("11-project-freeze.md");

    let agents = match fs::read_to_string(&agents_path) {
        Ok(contents) => contents,
        Err(error) => {
            return CheckResult::error(
                "Project freeze policy",
                format!("cannot read {}: {error}", agents_path.display()),
            );
        }
    };
    let freeze = match fs::read_to_string(&freeze_path) {
        Ok(contents) => contents,
        Err(error) => {
            return CheckResult::error(
                "Project freeze policy",
                format!("cannot read {}: {error}", freeze_path.display()),
            );
        }
    };

    for required in [
        "## 14. Project freeze",
        "Zamrznuto na korisnikov zahtjev 2026-09-04",
        "docs\\11-project-freeze.md",
    ] {
        if !agents.contains(required) {
            report.error(format!("AGENTS.md missing Project freeze rule: {required}"));
        }
    }
    if !(agents.contains("ne smije se")
        && agents.contains("mijenjati bez izricite korisnicke dozvole"))
    {
        report.error(
            "AGENTS.md missing Project freeze rule: ne smije se mijenjati bez izricite korisnicke dozvole"
                .to_string(),
        );
    }

    report.merge(validate_project_freeze_status(&freeze));
    for required in [
        "apps/qnc-project/**",
        "crates/qnc-project-desktop/**",
        "crates/qnc-project-store/**",
        "crates/qnc-project-desktop-adapter/**",
        "contracts/applications/project.application.json",
        "contracts/databases/project-registry.database.json",
        "contracts/databases/project-workspace.database.json",
        "contracts/ui/project.layout.json",
        "contracts/qnc-keyboard-shortcuts.json",
        "seed/system_seed.json",
        "tools/qnc-conformance/**",
    ] {
        if !freeze.contains(required) {
            report.error(format!(
                "docs/11-project-freeze.md missing protected Project scope: {required}"
            ));
        }
    }

    CheckResult::from_report("Project freeze policy", report)
}

fn validate_project_freeze_status(freeze: &str) -> ValidationReport {
    let mut report = ValidationReport::new();
    let statuses: Vec<_> = freeze
        .lines()
        .filter_map(|line| line.strip_prefix("Status:"))
        .map(str::trim)
        .collect();
    match statuses.as_slice() {
        ["zamrznuto"] => {}
        ["odmrznuto samo za odobreni zahvat"] => {
            for required in [
                "## Aktivno odobrenje",
                "izricito potvrdio:",
                "Odobrenje se odnosi",
                "Izvan gore navedenog odobrenja",
                "vratiti status na zamrznuto",
            ] {
                if !freeze.contains(required) {
                    report.error(format!("Scoped Project thaw missing approval/scope record: {required}"));
                }
            }
        }
        _ => report.error("Project freeze status must be frozen or explicitly scoped; unrestricted thaw is not allowed"),
    }
    report
}

fn validate_project_seed(root: &Path) -> CheckResult {
    let relative = "seed/system_seed.json";
    let path = root.join(relative);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            return CheckResult::error(
                "project system seed",
                format!("cannot read {}: {err}", path.display()),
            );
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&contents) {
        Ok(value) => value,
        Err(err) => {
            return CheckResult::error("project system seed", format!("invalid JSON: {err}"));
        }
    };
    let mut report = ValidationReport::new();
    let source_templates = value
        .get("source_templates")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let project_templates = value
        .get("project_templates")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if source_templates == 0 {
        report.error("seed/system_seed.json must contain source_templates");
    }
    if project_templates == 0 {
        report.error("seed/system_seed.json must contain project_templates");
    }
    let has_breaking_news = value
        .get("project_templates")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|templates| {
            templates.iter().any(|template| {
                template
                    .get("template_id")
                    .and_then(serde_json::Value::as_str)
                    == Some("tpl_breaking_news")
            })
        });
    if !has_breaking_news {
        report.error("seed/system_seed.json must contain tpl_breaking_news");
    }
    if report.is_ok() {
        CheckResult::ok(format!(
            "project system seed source_templates={source_templates} project_templates={project_templates}"
        ))
    } else {
        CheckResult::from_report("project system seed", report)
    }
}

fn validate_keyboard_catalog(root: &Path) -> CheckResult {
    let relative = "contracts/qnc-keyboard-shortcuts.json";
    let path = root.join(relative);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            return CheckResult::error(
                "keyboard shortcut catalog",
                format!("cannot read {}: {err}", path.display()),
            );
        }
    };

    match validate_keyboard_catalog_json(relative, &contents) {
        Ok(summary) => CheckResult::ok(format!(
            "keyboard shortcut catalog version={} active_preset={} actions={} presets={}",
            summary.version, summary.active_preset, summary.action_count, summary.preset_count
        )),
        Err(report) => CheckResult::from_report("keyboard shortcut catalog", report),
    }
}

fn validate_keyboard_matches_qnc_v4(root: &Path) -> CheckResult {
    let local = root.join("contracts/qnc-keyboard-shortcuts.json");
    let Some(parent) = root.parent() else {
        let mut report = ValidationReport::new();
        report.warning("root has no parent, cannot compare qnc_v4 keyboard catalog");
        return CheckResult::from_report("keyboard catalog qnc_v4 match", report);
    };
    let reference = parent
        .join("qnc_v4")
        .join("seed")
        .join("keyboard-shortcuts.json");

    if !reference.is_file() {
        let mut report = ValidationReport::new();
        report.warning(format!(
            "qnc_v4 reference keyboard catalog not found at {}",
            reference.display()
        ));
        return CheckResult::from_report("keyboard catalog qnc_v4 match", report);
    }

    let mut report = ValidationReport::new();
    let local = match fs::read_to_string(&local) {
        Ok(contents) => contents,
        Err(err) => {
            return CheckResult::error(
                "keyboard catalog qnc_v4 extension",
                format!("cannot read local keyboard catalog: {err}"),
            )
        }
    };
    let reference = match fs::read_to_string(&reference) {
        Ok(contents) => contents,
        Err(err) => {
            return CheckResult::error(
                "keyboard catalog qnc_v4 extension",
                format!("cannot read qnc_v4 keyboard catalog: {err}"),
            )
        }
    };

    let local = match serde_json::from_str::<serde_json::Value>(&local) {
        Ok(value) => value,
        Err(err) => {
            return CheckResult::error(
                "keyboard catalog qnc_v4 extension",
                format!("cannot parse local keyboard catalog: {err}"),
            )
        }
    };
    let reference = match serde_json::from_str::<serde_json::Value>(&reference) {
        Ok(value) => value,
        Err(err) => {
            return CheckResult::error(
                "keyboard catalog qnc_v4 extension",
                format!("cannot parse qnc_v4 keyboard catalog: {err}"),
            )
        }
    };

    require_reference_keyboard_subset(&mut report, &local, &reference);
    CheckResult::from_report("keyboard catalog qnc_v4 extension", report)
}

fn validate_ingest_keyboard_actions(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let keyboard_path = root.join("contracts").join("qnc-keyboard-shortcuts.json");
    let component_path = root
        .join("crates")
        .join("qnc-ingest-components")
        .join("src")
        .join("lib.rs");

    let actions = match fs::read_to_string(&keyboard_path)
        .ok()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .and_then(|value| {
            value
                .get("actions")
                .and_then(serde_json::Value::as_object)
                .cloned()
        }) {
        Some(actions) => actions,
        None => {
            report.error(format!(
                "{}: cannot read keyboard actions object",
                display_relative(root, &keyboard_path)
            ));
            return CheckResult::from_report("Ingest keyboard action catalog", report);
        }
    };

    let Ok(component_source) = fs::read_to_string(&component_path) else {
        report.error(format!(
            "{}: cannot read Ingest component source",
            display_relative(root, &component_path)
        ));
        return CheckResult::from_report("Ingest keyboard action catalog", report);
    };

    let action_ids = extract_string_constants(&component_source);
    for action_id in action_ids {
        if !actions.contains_key(&action_id) {
            report.error(format!(
                "{}: Ingest action_id '{action_id}' is missing from contracts/qnc-keyboard-shortcuts.json",
                display_relative(root, &component_path)
            ));
        }
    }

    CheckResult::from_report("Ingest keyboard action catalog", report)
}

fn extract_string_constants(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with("pub const ") || !trimmed.contains("&str") {
                return None;
            }
            trimmed.split('"').nth(1).map(str::to_string)
        })
        .collect()
}

fn require_reference_keyboard_subset(
    report: &mut ValidationReport,
    local: &serde_json::Value,
    reference: &serde_json::Value,
) {
    let Some(local_actions) = local.get("actions").and_then(serde_json::Value::as_object) else {
        report.error("local keyboard catalog missing actions object");
        return;
    };
    let Some(reference_actions) = reference
        .get("actions")
        .and_then(serde_json::Value::as_object)
    else {
        report.error("qnc_v4 keyboard catalog missing actions object");
        return;
    };

    for (action_id, reference_action) in reference_actions {
        match local_actions.get(action_id) {
            Some(local_action) if local_action == reference_action => {}
            Some(_) => report.error(format!(
                "keyboard action '{action_id}' differs from qnc_v4 reference"
            )),
            None => report.error(format!(
                "keyboard action '{action_id}' is missing from local catalog"
            )),
        }
    }

    let Some(local_presets) = local.get("presets").and_then(serde_json::Value::as_object) else {
        report.error("local keyboard catalog missing presets object");
        return;
    };
    let Some(reference_presets) = reference
        .get("presets")
        .and_then(serde_json::Value::as_object)
    else {
        report.error("qnc_v4 keyboard catalog missing presets object");
        return;
    };

    for (preset_id, reference_preset) in reference_presets {
        let Some(local_preset) = local_presets.get(preset_id) else {
            report.error(format!("keyboard preset '{preset_id}' is missing"));
            continue;
        };
        require_reference_preset_subset(report, preset_id, local_preset, reference_preset);
    }
}

fn require_reference_preset_subset(
    report: &mut ValidationReport,
    preset_id: &str,
    local_preset: &serde_json::Value,
    reference_preset: &serde_json::Value,
) {
    let Some(local_object) = local_preset.as_object() else {
        report.error(format!("keyboard preset '{preset_id}' must be an object"));
        return;
    };
    let Some(reference_object) = reference_preset.as_object() else {
        report.error(format!(
            "qnc_v4 keyboard preset '{preset_id}' must be an object"
        ));
        return;
    };

    for (scope_id, reference_scope) in reference_object {
        if matches!(scope_id.as_str(), "name" | "description") {
            if local_object.get(scope_id) != Some(reference_scope) {
                report.error(format!(
                    "keyboard preset '{preset_id}' field '{scope_id}' differs from qnc_v4"
                ));
            }
            continue;
        }

        let Some(local_scope) = local_object
            .get(scope_id)
            .and_then(serde_json::Value::as_object)
        else {
            report.error(format!(
                "keyboard preset '{preset_id}' missing scope '{scope_id}'"
            ));
            continue;
        };
        let Some(reference_scope) = reference_scope.as_object() else {
            report.error(format!(
                "qnc_v4 keyboard preset '{preset_id}' scope '{scope_id}' must be an object"
            ));
            continue;
        };

        for (action_id, reference_bindings) in reference_scope {
            match local_scope.get(action_id) {
                Some(local_bindings) if local_bindings == reference_bindings => {}
                Some(_) => report.error(format!(
                    "keyboard preset '{preset_id}' scope '{scope_id}' action '{action_id}' bindings differ from qnc_v4"
                )),
                None => report.error(format!(
                    "keyboard preset '{preset_id}' scope '{scope_id}' missing action '{action_id}'"
                )),
            }
        }
    }
}

fn validate_ui_layout_contract(root: &Path) -> CheckResult {
    let relative = "docs/07-ui-layout-reference.md";
    let path = root.join(relative);
    match fs::read_to_string(&path) {
        Ok(contents) => CheckResult::from_report(
            "UI/layout mirror contract",
            validate_ui_layout_reference_doc(relative, &contents),
        ),
        Err(err) => CheckResult::error(
            "UI/layout mirror contract",
            format!("cannot read {}: {err}", path.display()),
        ),
    }
}

fn validate_app_registry(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let mut app_manifest_paths = Vec::new();
    let apps_dir = root.join("apps");
    let entries = match fs::read_dir(&apps_dir) {
        Ok(entries) => entries,
        Err(err) => {
            return CheckResult::error(
                "QNC app registry",
                format!("cannot read {}: {err}", apps_dir.display()),
            );
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("qnc-app.json");
            if manifest_path.is_file() {
                app_manifest_paths.push(manifest_path);
            }
        }
    }

    if app_manifest_paths.is_empty() {
        report.error(format!(
            "no QNC app registry manifests found under {}",
            apps_dir.display()
        ));
    }

    let application_contracts = read_id_index(
        root,
        "contracts/applications",
        "application_id",
        &mut report,
    );
    let mut seen_application_ids = BTreeSet::new();
    let mut seen_tab_ids = BTreeSet::new();
    let mut enabled_count = 0usize;

    for path in app_manifest_paths {
        let name = display_relative(root, &path);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) => {
                report.error(format!("{name}: cannot read app registry manifest: {err}"));
                continue;
            }
        };
        let value = match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(value) => value,
            Err(err) => {
                report.error(format!("{name}: invalid JSON: {err}"));
                continue;
            }
        };
        let Some(object) = value.as_object() else {
            report.error(format!(
                "{name}: app registry manifest must be a JSON object"
            ));
            continue;
        };

        let application_id =
            required_app_registry_string(&name, object, "application_id", &mut report);
        let tab_id = required_app_registry_string(&name, object, "tab_id", &mut report);
        let label = required_app_registry_string(&name, object, "label", &mut report);
        let host_mode = required_app_registry_string(&name, object, "host_mode", &mut report);
        let desktop_entry =
            required_app_registry_string(&name, object, "desktop_entry", &mut report);
        let enabled = required_app_registry_bool(&name, object, "enabled", &mut report);
        let system = required_app_registry_bool(&name, object, "system", &mut report);
        let removable = required_app_registry_bool(&name, object, "removable", &mut report);
        let group = required_app_registry_string(&name, object, "priority_group", &mut report);

        if enabled == Some(true) {
            enabled_count += 1;
        }

        if let Some(application_id) = application_id {
            if !application_id.starts_with("qnc.") || application_id.starts_with("qnc.module.") {
                report.error(format!(
                    "{name}: application_id '{application_id}' is not a QNC application id"
                ));
            }
            if !seen_application_ids.insert(application_id.to_string()) {
                report.error(format!(
                    "{name}: duplicate app registry application_id '{application_id}'"
                ));
            }
            if !application_contracts.contains_key(application_id) {
                report.error(format!(
                    "{name}: application_id '{application_id}' has no contracts/applications manifest"
                ));
            }
        }

        if let Some(tab_id) = tab_id {
            if !seen_tab_ids.insert(tab_id.to_string()) {
                report.error(format!("{name}: duplicate app registry tab_id '{tab_id}'"));
            }
        }

        if let Some(host_mode) = host_mode {
            if !matches!(host_mode, "embedded_public_api" | "external_component") {
                report.error(format!("{name}: unsupported host_mode '{host_mode}'"));
            }
        }

        if system == Some(true) && removable == Some(true) {
            report.error(format!(
                "{name}: system app registry entries cannot be removable"
            ));
        }

        match object
            .get("standalone_executable")
            .and_then(serde_json::Value::as_str)
        {
            Some(executable) if !executable.trim().is_empty() => {}
            _ => report.error(format!(
                "{name}: standalone_executable is required because every QNC application must run outside the shell desktop"
            )),
        }

        if group.is_some_and(|group| group.len() != 1 || !group.as_bytes()[0].is_ascii_lowercase())
        {
            report.error(format!(
                "{name}: priority_group must be a single letter a-z"
            ));
        }

        let _ = (label, desktop_entry);
    }

    if enabled_count == 0 {
        report.error("QNC app registry must contain at least one enabled application");
    }

    CheckResult::from_report("QNC app registry", report)
}

fn required_app_registry_string<'a>(
    name: &str,
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    report: &mut ValidationReport,
) -> Option<&'a str> {
    match object.get(field).and_then(serde_json::Value::as_str) {
        Some(value) if !value.trim().is_empty() => Some(value),
        Some(_) => {
            report.error(format!("{name}: {field} must not be empty"));
            None
        }
        None => {
            report.error(format!("{name}: missing string field {field}"));
            None
        }
    }
}

fn required_app_registry_bool(
    name: &str,
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    report: &mut ValidationReport,
) -> Option<bool> {
    match object.get(field).and_then(serde_json::Value::as_bool) {
        Some(value) => Some(value),
        None => {
            report.error(format!("{name}: missing bool field {field}"));
            None
        }
    }
}

fn validate_json_manifests(
    root: &Path,
    relative_dir: &str,
    validate: fn(&str, &str) -> ValidationReport,
) -> Vec<CheckResult> {
    let dir = root.join(relative_dir);
    let mut files = Vec::new();
    collect_json_files(&dir, &mut files);

    if files.is_empty() {
        return vec![CheckResult::error(
            format!("manifest directory {relative_dir}"),
            format!("no JSON manifest files found in {}", dir.display()),
        )];
    }

    files
        .into_iter()
        .map(|path| {
            let name = display_relative(root, &path);
            match fs::read_to_string(&path) {
                Ok(contents) => CheckResult::from_report(name.clone(), validate(&name, &contents)),
                Err(err) => CheckResult::error(name, format!("cannot read manifest: {err}")),
            }
        })
        .collect()
}

fn validate_sample_qnc_uris() -> CheckResult {
    let mut report = ValidationReport::new();
    for uri in [
        "qnc://local/db/project_registry",
        "qnc://local/db/ingest_content/source_123",
        "qnc://lan/storage-a/media/source_123/clip_456/original",
        "qnc://intranet/mam-a/db/story/news_story_001",
    ] {
        if let Err(err) = parse_qnc_uri(uri) {
            report.error(err);
        }
    }

    for raw_path in [
        r"C:\media\clip001.mp4",
        "/mnt/media/clip001.mp4",
        r"\\server\share\clip001.mp4",
    ] {
        if parse_qnc_uri(raw_path).is_ok() {
            report.error(format!("raw path accepted as QNC URI: {raw_path}"));
        }
    }

    CheckResult::from_report("QNC URI parser", report)
}

fn validate_manifest_graph(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let modules = read_id_index(root, "contracts/modules", "module_id", &mut report);
    let applications = read_id_index(
        root,
        "contracts/applications",
        "application_id",
        &mut report,
    );
    let databases = read_json_documents(root, "contracts/databases", &mut report);
    let ui_contracts = read_relative_json_paths(root, "contracts/ui");

    if modules.is_empty() {
        report.error("manifest graph: no module ids found");
    }
    if applications.is_empty() {
        report.error("manifest graph: no application ids found");
    }

    for (module_id, path) in &modules {
        if !module_id.starts_with("qnc.module.") {
            report.error(format!(
                "{}: module_id '{module_id}' must start with qnc.module.",
                display_relative(root, path)
            ));
        }
    }

    for (application_id, path) in &applications {
        if !application_id.starts_with("qnc.") || application_id.starts_with("qnc.module.") {
            report.error(format!(
                "{}: application_id '{application_id}' must be an application id, not a module id",
                display_relative(root, path)
            ));
        }
    }

    validate_application_module_dependencies(root, &mut report, &modules);
    validate_application_ui_layout_contracts(root, &mut report, &ui_contracts);
    validate_shell_manifest_host(root, &mut report, &applications);
    validate_module_forbidden_dependencies(root, &mut report, &modules);
    validate_database_owner_applications(root, &mut report, &applications, databases);

    CheckResult::from_report("application/module/DB manifest graph", report)
}

fn validate_shell_manifest_host(
    root: &Path,
    report: &mut ValidationReport,
    applications: &BTreeMap<String, PathBuf>,
) {
    let Some(path) = applications.get("qnc.shell") else {
        report.error("missing qnc.shell host manifest");
        return;
    };
    let Ok(contents) = fs::read_to_string(path) else {
        report.error(format!(
            "{}: cannot read shell manifest",
            display_relative(root, path)
        ));
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        report.error(format!(
            "{}: cannot parse shell manifest",
            display_relative(root, path)
        ));
        return;
    };

    let application_kind = value
        .get("application_kind")
        .and_then(serde_json::Value::as_str);
    if application_kind != Some("desktop_host") {
        report.error(format!(
            "{}: qnc.shell must be application_kind=desktop_host, not a business form",
            display_relative(root, path)
        ));
    }

    let owned_databases = value
        .get("owned_database_contracts")
        .and_then(serde_json::Value::as_array);
    if !matches!(owned_databases, Some(databases) if databases.is_empty()) {
        report.error(format!(
            "{}: qnc.shell must not own business databases",
            display_relative(root, path)
        ));
    }
}

fn validate_application_ui_layout_contracts(
    root: &Path,
    report: &mut ValidationReport,
    ui_contracts: &BTreeSet<String>,
) {
    for (path, value) in read_json_documents(root, "contracts/applications", report) {
        let name = display_relative(root, &path);
        let Some(object) = value.as_object() else {
            continue;
        };
        let Some(contracts) = object
            .get("ui_layout_contracts")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };

        for contract in contracts {
            let Some(contract_path) = contract.as_str() else {
                report.error(format!(
                    "{name}: ui_layout_contracts entries must be strings"
                ));
                continue;
            };
            if !contract_path.starts_with("contracts/ui/") {
                report.error(format!(
                    "{name}: UI layout contract '{contract_path}' must live under contracts/ui/"
                ));
                continue;
            }
            if !ui_contracts.contains(contract_path) {
                report.error(format!(
                    "{name}: UI layout contract '{contract_path}' does not exist"
                ));
            }
        }
    }
}

fn validate_application_module_dependencies(
    root: &Path,
    report: &mut ValidationReport,
    modules: &BTreeMap<String, PathBuf>,
) {
    let cargo_packages = cargo_package_index(root);
    for (path, value) in read_json_documents(root, "contracts/applications", report) {
        let name = display_relative(root, &path);
        let Some(object) = value.as_object() else {
            continue;
        };
        let application_id = object
            .get("application_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Some(dependencies) = object
            .get("module_dependencies")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        let runtime_crates =
            application_runtime_crates(root, application_id, &cargo_packages, report);

        for dependency in dependencies {
            let Some(module_id) = dependency.as_str() else {
                report.error(format!(
                    "{name}: module_dependencies entries must be strings"
                ));
                continue;
            };
            if !module_id.starts_with("qnc.module.") {
                report.error(format!(
                    "{name}: dependency '{module_id}' is not a module id"
                ));
                continue;
            }
            if !modules.contains_key(module_id) {
                report.error(format!(
                    "{name}: dependency '{module_id}' has no module manifest"
                ));
            }
            if let Some(runtime_crates) = runtime_crates.as_ref() {
                let Some(required_crate) = runtime_crate_for_module(module_id) else {
                    report.error(format!(
                        "{name}: dependency '{module_id}' has no implemented runtime crate for {application_id}"
                    ));
                    continue;
                };
                if !runtime_crates.contains(required_crate) {
                    report.error(format!(
                        "{name}: dependency '{module_id}' requires runtime crate '{required_crate}', but {application_id} does not depend on it transitively"
                    ));
                }
            }
        }
    }
}

fn runtime_crate_for_module(module_id: &str) -> Option<&'static str> {
    match module_id {
        "qnc.module.application-catalog-reader" => Some("qnc-application-catalog"),
        "qnc.module.manifest-capability" => Some("qnc-contracts"),
        "qnc.module.transport-resolver" => Some("qnc-transport-resolver"),
        "qnc.module.db-contract-validation" => Some("qnc-db-contract"),
        "qnc.module.dir-browser" => Some("qnc-dir-browser"),
        "qnc.module.keyboard-shortcut" => Some("qnc-keyboard-shortcut"),
        "qnc.module.frame-timebase" => Some("qnc-frame-timebase"),
        "qnc.module.player-contract" => Some("qnc-player-contract"),
        "qnc.module.player-input" => Some("qnc-player-input"),
        "qnc.module.player-client" => Some("qnc-player-client"),
        "qnc.module.media-stream" => Some("qnc-media-stream"),
        "qnc.module.media-decode" => Some("qnc-media-decode"),
        "qnc.module.ffmpeg-decode" => Some("qnc-ffmpeg-decode"),
        "qnc.module.decoder-catalog" => Some("qnc-decoder-catalog"),
        "qnc.module.ui-widget" => Some("qnc-ui-kit"),
        "qnc.module.workstation-identity" => Some("qnc-workstation-identity"),
        "qnc.module.work-settings" => Some("qnc-work-settings"),
        "qnc.module.camera-patterns" => Some("qnc-camera-patterns"),
        "qnc.module.camera-detector" => Some("qnc-camera-detector"),
        "qnc.module.scanner" => Some("qnc-scanner"),
        "qnc.module.source-reader" => Some("qnc-source-reader"),
        "qnc.module.source-groups" => Some("qnc-source-groups"),
        "qnc.module.source-index-db" => Some("qnc-source-index-db"),
        "qnc.module.sony-metadata" => Some("qnc-sony-metadata"),
        "qnc.module.media-record-db" => Some("qnc-media-record-db"),
        "qnc.module.media-metadata" => Some("qnc-media-metadata"),
        "qnc.module.image-assets" => Some("qnc-image-assets"),
        "qnc.module.media-probe" => Some("qnc-media-probe"),
        "qnc.module.ffprobe-metadata" => Some("qnc-ffprobe-metadata"),
        "qnc.module.media-metadata-compose" => Some("qnc-media-metadata-compose"),
        _ => None,
    }
}

fn application_runtime_crates(
    _root: &Path,
    application_id: &str,
    cargo_packages: &BTreeMap<String, PathBuf>,
    report: &mut ValidationReport,
) -> Option<BTreeSet<String>> {
    let crate_name = match application_id {
        "qnc.project" => "qnc-project",
        "qnc.ingest" => "qnc-ingest",
        _ => return None,
    };
    if !cargo_packages.contains_key(crate_name) {
        report.error(format!(
            "{application_id}: missing runtime Cargo package '{crate_name}'"
        ));
        return Some(BTreeSet::new());
    }
    Some(collect_transitive_cargo_dependencies(
        crate_name,
        cargo_packages,
    ))
}

fn cargo_package_index(root: &Path) -> BTreeMap<String, PathBuf> {
    let mut cargo_files = Vec::new();
    collect_named_files(root, "Cargo.toml", &mut cargo_files);
    let mut packages = BTreeMap::new();
    for cargo_toml in cargo_files {
        let Ok(contents) = fs::read_to_string(&cargo_toml) else {
            continue;
        };
        if let Some(package_name) = parse_cargo_package_name(&contents) {
            packages.insert(package_name, cargo_toml);
        }
    }
    packages
}

fn collect_transitive_cargo_dependencies(
    root_crate: &str,
    cargo_packages: &BTreeMap<String, PathBuf>,
) -> BTreeSet<String> {
    let mut visited = BTreeSet::new();
    let mut stack = vec![root_crate.to_string()];
    while let Some(crate_name) = stack.pop() {
        if !visited.insert(crate_name.clone()) {
            continue;
        }
        let Some(cargo_toml) = cargo_packages.get(&crate_name) else {
            continue;
        };
        let Ok(contents) = fs::read_to_string(cargo_toml) else {
            continue;
        };
        for dependency in cargo_dependency_names(&contents, cargo_packages) {
            if !visited.contains(&dependency) {
                stack.push(dependency);
            }
        }
    }
    visited
}

fn parse_cargo_package_name(contents: &str) -> Option<String> {
    let mut in_package = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if in_package && trimmed.starts_with('[') {
            return None;
        }
        if in_package && trimmed.starts_with("name") {
            let (_, value) = trimmed.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn cargo_dependency_names(
    contents: &str,
    cargo_packages: &BTreeMap<String, PathBuf>,
) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut in_dependencies = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" || trimmed == "[dev-dependencies]" {
            in_dependencies = true;
            continue;
        }
        if in_dependencies && trimmed.starts_with('[') {
            in_dependencies = false;
        }
        if !in_dependencies || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if cargo_packages.contains_key(name) {
            dependencies.push(name.to_string());
        }
    }
    dependencies
}

fn validate_module_forbidden_dependencies(
    root: &Path,
    report: &mut ValidationReport,
    modules: &BTreeMap<String, PathBuf>,
) {
    for (path, value) in read_json_documents(root, "contracts/modules", report) {
        let name = display_relative(root, &path);
        let Some(object) = value.as_object() else {
            continue;
        };
        let Some(forbidden_dependencies) = object
            .get("forbidden_dependencies")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };

        for dependency in forbidden_dependencies {
            let Some(module_id) = dependency.as_str() else {
                report.error(format!(
                    "{name}: forbidden_dependencies entries must be strings"
                ));
                continue;
            };
            if !module_id.starts_with("qnc.module.") {
                report.error(format!(
                    "{name}: forbidden dependency '{module_id}' is not a module id"
                ));
                continue;
            }
            if !modules.contains_key(module_id) {
                report.error(format!(
                    "{name}: forbidden dependency '{module_id}' has no module manifest"
                ));
            }
        }
    }
}

fn validate_database_owner_applications(
    root: &Path,
    report: &mut ValidationReport,
    applications: &BTreeMap<String, PathBuf>,
    databases: Vec<(PathBuf, serde_json::Value)>,
) {
    for (path, value) in databases {
        let name = display_relative(root, &path);
        let Some(object) = value.as_object() else {
            continue;
        };
        let Some(owner_application) = object
            .get("owner_application")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if !applications.contains_key(owner_application) {
            report.error(format!(
                "{name}: owner_application '{owner_application}' has no application manifest"
            ));
        }
    }
}

fn read_id_index(
    root: &Path,
    relative_dir: &str,
    id_field: &str,
    report: &mut ValidationReport,
) -> BTreeMap<String, PathBuf> {
    let mut ids = BTreeMap::new();
    let mut seen = BTreeSet::new();

    for (path, value) in read_json_documents(root, relative_dir, report) {
        let name = display_relative(root, &path);
        let Some(id) = value
            .as_object()
            .and_then(|object| object.get(id_field))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };

        if !seen.insert(id.to_string()) {
            report.error(format!("{name}: duplicate {id_field} '{id}'"));
        }
        ids.insert(id.to_string(), path);
    }

    ids
}

fn read_json_documents(
    root: &Path,
    relative_dir: &str,
    report: &mut ValidationReport,
) -> Vec<(PathBuf, serde_json::Value)> {
    let mut files = Vec::new();
    collect_json_files(&root.join(relative_dir), &mut files);

    files
        .into_iter()
        .filter_map(|path| match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<serde_json::Value>(&contents) {
                Ok(value) => Some((path, value)),
                Err(err) => {
                    report.error(format!(
                        "{}: invalid JSON: {err}",
                        display_relative(root, &path)
                    ));
                    None
                }
            },
            Err(err) => {
                report.error(format!(
                    "{}: cannot read JSON: {err}",
                    display_relative(root, &path)
                ));
                None
            }
        })
        .collect()
}

fn read_relative_json_paths(root: &Path, relative_dir: &str) -> BTreeSet<String> {
    let mut files = Vec::new();
    collect_json_files(&root.join(relative_dir), &mut files);
    files
        .into_iter()
        .map(|path| display_relative(root, &path))
        .collect()
}

fn scan_rust_for_hardcoded_shortcuts(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let mut files = Vec::new();
    collect_rs_files(&root.join("crates"), &mut files);
    collect_rs_files(&root.join("apps"), &mut files);
    collect_rs_files(&root.join("tools"), &mut files);

    for file in files {
        let Ok(contents) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in contents.lines().enumerate() {
            if contains_hardcoded_shortcut_pattern(line) {
                report.error(format!(
                    "{}:{} contains a hardcoded shortcut pattern",
                    display_relative(root, &file),
                    index + 1
                ));
            }
        }
    }

    CheckResult::from_report("hardcoded shortcut scanner", report)
}

fn scan_business_app_isolation(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let cargo_packages = cargo_package_index(root);
    for (crate_name, cargo_toml) in &cargo_packages {
        let Some(family) = business_app_family(crate_name) else {
            continue;
        };
        for dependency in collect_transitive_cargo_dependencies(crate_name, &cargo_packages) {
            if dependency == *crate_name {
                continue;
            }
            let Some(dependency_family) = business_app_family(&dependency) else {
                continue;
            };
            if dependency_family != family {
                report.error(format!(
                    "{}: business application crate '{crate_name}' must not depend on '{dependency}'; DB records are the only business link between applications",
                    display_relative(root, cargo_toml)
                ));
            }
        }

        let Some(crate_root) = cargo_toml.parent() else {
            continue;
        };
        let mut files = Vec::new();
        collect_rs_files(&crate_root.join("src"), &mut files);
        for file in files {
            let Ok(contents) = fs::read_to_string(&file) else {
                continue;
            };
            for forbidden in forbidden_business_app_imports(family) {
                if contains_business_app_import(&contents, forbidden) {
                    report.error(format!(
                        "{}: business application family '{family}' must not import '{forbidden}'; use DB contract records only",
                        display_relative(root, &file)
                    ));
                }
            }
        }
    }

    CheckResult::from_report("business app DB-only isolation", report)
}

fn business_app_family(crate_name: &str) -> Option<&'static str> {
    if crate_name.starts_with("qnc-project") {
        Some("project")
    } else if crate_name.starts_with("qnc-ingest") {
        Some("ingest")
    } else if crate_name.starts_with("qnc-media-assist") {
        Some("media_assist")
    } else if crate_name.starts_with("qnc-story") {
        Some("story")
    } else {
        None
    }
}

fn forbidden_business_app_imports(family: &str) -> &'static [&'static str] {
    match family {
        "project" => &["qnc_ingest", "qnc_media_assist", "qnc_story"],
        "ingest" => &["qnc_project", "qnc_media_assist", "qnc_story"],
        "media_assist" => &["qnc_project", "qnc_ingest", "qnc_story"],
        "story" => &["qnc_project", "qnc_ingest", "qnc_media_assist"],
        _ => &[],
    }
}

fn contains_business_app_import(contents: &str, crate_prefix: &str) -> bool {
    for line in contents.lines() {
        let line = line.split_once("//").map(|(code, _)| code).unwrap_or(line);
        let trimmed = line.trim();
        if trimmed.starts_with("use ") && trimmed.contains(crate_prefix) {
            return true;
        }
        if trimmed.starts_with("extern crate ") && trimmed.contains(crate_prefix) {
            return true;
        }
        if trimmed.contains(&format!("{crate_prefix}::")) {
            return true;
        }
    }
    false
}

fn scan_project_app_boundary(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let project_app_root = root.join("apps").join("qnc-project");
    let project_desktop_root = root.join("crates").join("qnc-project-desktop");
    let project_store_root = root.join("crates").join("qnc-project-store");
    let project_adapter_root = root.join("crates").join("qnc-project-desktop-adapter");
    let app_cargo_toml = project_app_root.join("Cargo.toml");
    let desktop_cargo_toml = project_desktop_root.join("Cargo.toml");
    let store_cargo_toml = project_store_root.join("Cargo.toml");
    let adapter_cargo_toml = project_adapter_root.join("Cargo.toml");
    let mut has_transport_resolver_dependency = false;
    let mut has_dir_browser_dependency = false;
    let mut has_keyboard_dependency = false;

    if !project_desktop_root.join("src").join("lib.rs").is_file() {
        report.error("missing crates/qnc-project-desktop/src/lib.rs");
    }
    if !project_store_root.join("src").join("lib.rs").is_file() {
        report.error("missing crates/qnc-project-store/src/lib.rs");
    }

    if let Ok(contents) = fs::read_to_string(&app_cargo_toml) {
        if !contents.contains("qnc-project-desktop") {
            report.error(format!(
                "{}: standalone Project app must depend on qnc-project-desktop",
                display_relative(root, &app_cargo_toml)
            ));
        }
        for dependency in [
            "qnc-project-store",
            "qnc-transport-resolver",
            "qnc-dir-browser",
            "rusqlite",
        ] {
            if contents.contains(dependency) {
                report.error(format!(
                    "{}: standalone Project executable must not depend directly on '{dependency}'",
                    display_relative(root, &app_cargo_toml)
                ));
            }
        }
    }

    if let Ok(contents) = fs::read_to_string(&desktop_cargo_toml) {
        has_dir_browser_dependency = contents.contains("qnc-dir-browser");
        has_keyboard_dependency = contents.contains("qnc-keyboard-shortcut");
        if !contents.contains("qnc-project-store") {
            report.error(format!(
                "{}: Project desktop surface must depend on qnc-project-store through its component boundary",
                display_relative(root, &desktop_cargo_toml)
            ));
        }
    } else {
        report.error(format!(
            "missing {}",
            display_relative(root, &desktop_cargo_toml)
        ));
    }

    if let Ok(contents) = fs::read_to_string(&store_cargo_toml) {
        has_transport_resolver_dependency = contents.contains("qnc-transport-resolver");
        if !contents.contains("rusqlite") || !contents.contains("qnc-db-contract") {
            report.error(format!(
                "{}: Project store must own DB/runtime dependencies",
                display_relative(root, &store_cargo_toml)
            ));
        }
    } else {
        report.error(format!(
            "missing {}",
            display_relative(root, &store_cargo_toml)
        ));
    }

    if let Ok(contents) = fs::read_to_string(&adapter_cargo_toml) {
        if !contents.contains("qnc-project-desktop") {
            report.error(format!(
                "{}: Project desktop adapter must depend on qnc-project-desktop",
                display_relative(root, &adapter_cargo_toml)
            ));
        }
        for dependency in [
            "qnc-project =",
            "../../apps/qnc-project",
            "qnc-project-store",
        ] {
            if contents.contains(dependency) {
                report.error(format!(
                    "{}: Project desktop adapter must not depend directly on standalone Project app or store crate",
                    display_relative(root, &adapter_cargo_toml)
                ));
            }
        }
    } else {
        report.error(format!(
            "missing {}",
            display_relative(root, &adapter_cargo_toml)
        ));
    }

    if let Ok(contents) = fs::read_to_string(project_adapter_root.join("src").join("lib.rs")) {
        for forbidden in ["qnc_project::", "qnc_project_store::"] {
            if contents.contains(forbidden) {
                report.error(format!(
                    "crates/qnc-project-desktop-adapter/src/lib.rs: Project desktop adapter must not import '{forbidden}'"
                ));
            }
        }
    }

    for (cargo_toml, label) in [
        (&app_cargo_toml, "Project app"),
        (&desktop_cargo_toml, "Project desktop"),
        (&store_cargo_toml, "Project store"),
        (&adapter_cargo_toml, "Project desktop adapter"),
    ] {
        let Ok(contents) = fs::read_to_string(cargo_toml) else {
            continue;
        };
        for dependency in [
            "qnc-media-ffmpeg",
            "qnc-broadcast-player",
            "qnc-player",
            "qnc-worker",
        ] {
            if contents.contains(dependency) {
                report.error(format!(
                    "{}: {label} must not depend on media runtime '{dependency}'",
                    display_relative(root, cargo_toml)
                ));
            }
        }
    }
    if !has_transport_resolver_dependency {
        report.error(format!(
            "{}: Project app manifest declares transport-resolver but Cargo runtime dependency is missing",
            display_relative(root, &store_cargo_toml)
        ));
    }
    if !has_dir_browser_dependency {
        report.error(format!(
            "{}: Project app manifest declares dir-browser but Cargo runtime dependency is missing",
            display_relative(root, &desktop_cargo_toml)
        ));
    }
    if !has_keyboard_dependency {
        report.error(format!(
            "{}: Project app manifest declares keyboard-shortcut but Cargo runtime dependency is missing",
            display_relative(root, &desktop_cargo_toml)
        ));
    }

    let mut files = Vec::new();
    collect_rs_files(&project_app_root.join("src"), &mut files);
    collect_rs_files(&project_desktop_root.join("src"), &mut files);
    collect_rs_files(&project_store_root.join("src"), &mut files);
    let mut uses_transport_resolver = false;
    let mut uses_dir_browser = false;
    let mut dispatches_project_shortcuts = false;
    let mut has_project_component = false;
    let mut has_project_action_dispatch = false;
    let mut has_project_location_browser = false;
    for file in files {
        let Ok(contents) = fs::read_to_string(&file) else {
            continue;
        };
        let relative = display_relative(root, &file);
        if relative == "crates/qnc-project-desktop/src/project_component.rs" {
            has_project_component = true;
        }
        if relative == "crates/qnc-project-desktop/src/location_browser.rs"
            && contents.contains("LocationBrowserInput")
            && contents.contains("Računalo")
            && contents.contains("LAN")
            && contents.contains("Internet")
            && contents.contains("BrowserState")
        {
            has_project_location_browser = true;
        }
        if relative == "crates/qnc-project-desktop/src/location_browser.rs" {
            for forbidden in ["confirm_label", "\"Odustani\"", "\"U redu\""] {
                if contents.contains(forbidden) {
                    report.error(format!(
                        "crates/qnc-project-desktop/src/location_browser.rs: confirm/cancel action bar must stay outside browser component, found '{forbidden}'"
                    ));
                }
            }
        }
        if relative == "crates/qnc-project-desktop/src/location_browser.rs"
            && contents.contains("clean_location_path")
        {
            report.error(
                "crates/qnc-project-desktop/src/location_browser.rs: Project UI must use qnc-dir-browser display state instead of private path cleanup"
                    .to_string(),
            );
        }
        if relative == "crates/qnc-project-desktop/src/project_component.rs" {
            for forbidden in [
                "struct DirectoryBrowserEntry",
                "struct DirectoryBrowserListing",
                "DirectoryListRequest",
            ] {
                if contents.contains(forbidden) {
                    report.error(format!(
                        "crates/qnc-project-desktop/src/project_component.rs: Project must use qnc-dir-browser DirectoryBrowserSession instead of private browser type '{forbidden}'"
                    ));
                }
            }
            if !contents.contains("DirectoryBrowserSession") {
                report.error(
                    "crates/qnc-project-desktop/src/project_component.rs: Project component must use the shared qnc-dir-browser session"
                        .to_string(),
                );
            }
        }
        if contents.contains("qnc_transport_resolver::") || contents.contains("ResolverConfig") {
            uses_transport_resolver = true;
        }
        if contents.contains("qnc_dir_browser::") || contents.contains("DirectoryPickRequest") {
            uses_dir_browser = true;
        }
        if contents.contains("action_ids_for_event(\"project\"") {
            dispatches_project_shortcuts = true;
        }
        if contents.contains("enum ProjectAction")
            && contents.contains("fn dispatch_project_action")
            && contents.contains("action.action_id()")
        {
            has_project_action_dispatch = true;
        }
        for (index, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if relative == "crates/qnc-project-desktop/src/app.rs"
                && (trimmed.contains("ProjectStore") || trimmed.contains("self.store"))
            {
                report.error(format!(
                    "{}:{} Project form must send intents through ProjectComponent, not call ProjectStore directly",
                    relative,
                    index + 1
                ));
            }
            if trimmed.contains("rfd::FileDialog") {
                report.error(format!(
                    "{}:{} Project app must use qnc-dir-browser instead of direct rfd FileDialog",
                    display_relative(root, &file),
                    index + 1
                ));
            }
            if relative.starts_with("crates/qnc-project-desktop/")
                && trimmed.contains("pick_directory")
            {
                report.error(format!(
                    "{}:{} Project desktop must use embedded dir.list browser, not OS folder picker",
                    display_relative(root, &file),
                    index + 1
                ));
            }
            if project_app_forbidden_active_code(trimmed) {
                report.error(format!(
                    "{}:{} contains forbidden active media/app workflow code for Project app",
                    display_relative(root, &file),
                    index + 1
                ));
            }
        }
    }
    if !uses_transport_resolver {
        report.error("Project runtime does not use qnc-transport-resolver".to_string());
    }
    if !uses_dir_browser {
        report.error("Project runtime does not use qnc-dir-browser".to_string());
    }
    if !has_project_component {
        report.error(
            "Project app must isolate active form operations in project_component.rs".to_string(),
        );
    }
    if !dispatches_project_shortcuts {
        report.error("Project runtime does not dispatch project keyboard actions".to_string());
    }
    if !has_project_action_dispatch {
        report.error(
            "Project desktop form must dispatch click/keyboard intents through ProjectAction action_id"
                .to_string(),
        );
    }
    if !has_project_location_browser {
        report.error(
            "Project desktop must include embedded location browser with Local/LAN/Internet actions"
                .to_string(),
        );
    }
    validate_project_location_browser_shortcuts(root, &mut report);

    CheckResult::from_report("qnc-project app boundary", report)
}

fn scan_shared_ui_patterns(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let ui_kit = root
        .join("crates")
        .join("qnc-ui-kit")
        .join("src")
        .join("lib.rs");
    match fs::read_to_string(&ui_kit) {
        Ok(contents) => {
            for required in [
                "show_form_action_bar",
                "FormActionBarStyle",
                "FormActionBarResponse",
                "STANDARD_ACTION_BUTTON_WIDTH",
                "toggle_exclusive_panel",
            ] {
                if !contents.contains(required) {
                    report.error(format!(
                        "{}: missing shared UI action bar symbol '{required}'",
                        display_relative(root, &ui_kit)
                    ));
                }
            }
        }
        Err(error) => report.error(format!(
            "{}: cannot read shared UI kit: {error}",
            display_relative(root, &ui_kit)
        )),
    }

    for crate_name in ["qnc-project-desktop", "qnc-ingest-desktop"] {
        let cargo = root.join("crates").join(crate_name).join("Cargo.toml");
        match fs::read_to_string(&cargo) {
            Ok(contents) => {
                if !contents.contains("qnc-ui-kit") {
                    report.error(format!(
                        "{}: {crate_name} must use shared qnc-ui-kit for standard UI patterns",
                        display_relative(root, &cargo)
                    ));
                }
            }
            Err(error) => report.error(format!(
                "{}: cannot read Cargo.toml: {error}",
                display_relative(root, &cargo)
            )),
        }

        let src = root.join("crates").join(crate_name).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src, &mut files);
        for file in files {
            let Ok(contents) = fs::read_to_string(&file) else {
                continue;
            };
            let relative = display_relative(root, &file);
            for forbidden in [
                "FORM_ACTION_BUTTON_W",
                "FORM_ACTION_BUTTON_GAP",
                "fn form_primary_button",
                "fn form_action_button",
                "fn browser_primary_btn",
                "fn browser_action_btn",
            ] {
                if contents.contains(forbidden) {
                    report.error(format!(
                        "{relative}: standard confirm/cancel action bar must come from qnc-ui-kit, found '{forbidden}'"
                    ));
                }
            }
        }
    }

    validate_ingest_browser_uses_shared_action_bar(root, &mut report);

    CheckResult::from_report("shared UI patterns", report)
}

fn validate_ingest_browser_uses_shared_action_bar(root: &Path, report: &mut ValidationReport) {
    let widgets = root
        .join("crates")
        .join("qnc-ingest-desktop")
        .join("src")
        .join("widgets.rs");
    let Ok(contents) = fs::read_to_string(&widgets) else {
        report.error(format!(
            "{}: cannot read Ingest desktop widgets",
            display_relative(root, &widgets)
        ));
        return;
    };

    if !contents.contains("fn render_location_action_bar")
        || !contents.contains("qnc_ui_kit::show_form_action_bar")
    {
        report.error(format!(
            "{}: Ingest browser confirm/cancel bar must use qnc-ui-kit",
            display_relative(root, &widgets)
        ));
    }

    if let Some(browser_body) = source_between(
        &contents,
        "fn render_location_browser",
        "fn render_location_action_bar",
    ) {
        for forbidden in [
            "qnc_ui_kit::show_form_action_bar",
            "INGEST_DIR_CONFIRM",
            "INGEST_DIR_CANCEL",
            "confirm_label",
            "cancel_label",
        ] {
            if browser_body.contains(forbidden) {
                report.error(format!(
                    "{}: Ingest browser view must not own action bar marker '{forbidden}'",
                    display_relative(root, &widgets)
                ));
            }
        }
    } else {
        report.error(format!(
            "{}: cannot locate Ingest location browser/action bar boundary",
            display_relative(root, &widgets)
        ));
    }
}

fn source_between<'a>(contents: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_index = contents.find(start)?;
    let after_start = &contents[start_index..];
    let end_index = after_start.find(end)?;
    Some(&after_start[..end_index])
}

fn validate_project_location_browser_shortcuts(root: &Path, report: &mut ValidationReport) {
    let path = root.join("contracts").join("qnc-keyboard-shortcuts.json");
    let Ok(contents) = fs::read_to_string(&path) else {
        report.error(format!(
            "{}: cannot read Project location browser shortcut catalog",
            display_relative(root, &path)
        ));
        return;
    };
    for action_id in [
        "project_pick_projects_root",
        "project_projects_root_browser_select_source",
        "project_projects_root_browser_open_path",
        "project_projects_root_browser_confirm",
        "project_projects_root_browser_cancel",
        "project_pick_export_dir",
        "project_export_dir_browser_select_source",
        "project_export_dir_browser_open_path",
        "project_export_dir_browser_confirm",
        "project_export_dir_browser_cancel",
    ] {
        if !contents.contains(&format!("\"{action_id}\"")) {
            report.error(format!(
                "contracts/qnc-keyboard-shortcuts.json: missing Project location browser action '{action_id}'"
            ));
        }
    }
}

fn scan_shell_app_boundary(root: &Path) -> CheckResult {
    let mut report = ValidationReport::new();
    let shell_root = root.join("apps").join("qnc-app");
    let cargo_toml = shell_root.join("Cargo.toml");

    if !cargo_toml.is_file() {
        report.error(format!("missing {}", display_relative(root, &cargo_toml)));
    }
    if let Ok(contents) = fs::read_to_string(&cargo_toml) {
        if contents.contains("qnc-project =") {
            report.error(format!(
                "{}: Shell must depend on desktop adapter crates, not full application crates",
                display_relative(root, &cargo_toml)
            ));
        }
        if !contents.contains("qnc-shell-desktop-api") {
            report.error(format!(
                "{}: Shell must use qnc-shell-desktop-api for embedded application hosting",
                display_relative(root, &cargo_toml)
            ));
        }
    }

    let mut files = Vec::new();
    collect_rs_files(&shell_root.join("src"), &mut files);
    for file in files {
        let Ok(contents) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("Command::new") || trimmed.contains(".spawn()") {
                report.error(format!(
                    "{}:{} Shell must host QNC desktop components, not spawn application OS windows",
                    display_relative(root, &file),
                    index + 1
                ));
            }
            if trimmed.contains(".desktop_entry ==") {
                report.error(format!(
                    "{}:{} Shell must dispatch embedded apps through the desktop_entry adapter registry, not app-specific conditionals",
                    display_relative(root, &file),
                    index + 1
                ));
            }
            if trimmed.contains("qnc_project::") {
                report.error(format!(
                    "{}:{} Shell must not import the full Project application crate directly",
                    display_relative(root, &file),
                    index + 1
                ));
            }
            if trimmed.starts_with("use qnc_project::project_store")
                || trimmed.starts_with("use qnc_project::project_advanced")
                || trimmed.starts_with("use qnc_project::layout_contract")
                || trimmed.starts_with("use qnc_project::theme")
                || trimmed.starts_with("use qnc_project::widgets")
                || trimmed.starts_with("use qnc_ingest")
                || trimmed.starts_with("use qnc_story")
                || trimmed.starts_with("use qnc_media_assist")
            {
                report.error(format!(
                    "{}:{} Shell imports private application code",
                    display_relative(root, &file),
                    index + 1
                ));
            }
        }
    }

    CheckResult::from_report("qnc-shell app boundary", report)
}

fn project_app_forbidden_active_code(line: &str) -> bool {
    if line.starts_with("//") {
        return false;
    }
    if line.contains("\"pts_playback_input\"") {
        return false;
    }

    [
        "ffprobe",
        "media_probe",
        "qnc_media_ffmpeg",
        "qnc_filmstrip",
        "filmstrip::",
        "waveform",
        "broadcast_player",
        "playback_",
        "scanner::",
        "ingest::",
        "export::",
    ]
    .iter()
    .any(|needle| line.contains(needle))
}

fn collect_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            out.push(path);
        }
    }
}

fn collect_named_files(dir: &Path, file_name: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_named_files(&path, file_name, out);
        } else if path.file_name().and_then(|name| name.to_str()) == Some(file_name) {
            out.push(path);
        }
    }
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod freeze_status_tests {
    use super::*;

    #[test]
    fn frozen_policy_remains_valid() {
        assert!(validate_project_freeze_status("Status: zamrznuto\n").is_ok());
    }

    #[test]
    fn scoped_thaw_requires_approval_scope_and_relock_record() {
        let current = "Status: odmrznuto samo za odobreni zahvat\n\
            ## Aktivno odobrenje\nKorisnik je izricito potvrdio: odobreni zahvat.\n\
            Odobrenje se odnosi na navedeni opseg.\n\
            Izvan gore navedenog odobrenja vrijedi zamrzavanje.\n\
            Nakon zahvata vratiti status na zamrznuto.";
        assert!(validate_project_freeze_status(current).is_ok());
        for required in [
            "izricito potvrdio:",
            "Odobrenje se odnosi",
            "vratiti status na zamrznuto",
        ] {
            assert!(!validate_project_freeze_status(&current.replace(required, "")).is_ok());
        }
    }

    #[test]
    fn unrestricted_or_ambiguous_thaw_is_rejected() {
        for text in [
            "Status: odmrznuto",
            "Status: odmrznuto samo za odobreni zahvat",
            "Status: zamrznuto\nStatus: odmrznuto",
        ] {
            assert!(!validate_project_freeze_status(text).is_ok());
        }
    }
}
