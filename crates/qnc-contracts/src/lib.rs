use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub fn merge(&mut self, other: ValidationReport) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QncUri {
    pub environment: String,
    pub authority: Option<String>,
    pub resource_kind: String,
    pub resource_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardCatalogSummary {
    pub version: i64,
    pub active_preset: String,
    pub action_count: usize,
    pub preset_count: usize,
}

pub const FORBIDDEN_MODULE_KEYS: &[&str] = &[
    "allowed_applications",
    "allowed_apps",
    "allowed_modules",
    "required_modules",
    "consumers",
];

pub const REQUIRED_MODULE_FIELDS: &[&str] = &[
    "module_id",
    "module_name",
    "module_version",
    "module_kind",
    "capabilities",
    "input_contract_version",
    "output_contract_version",
    "supported_os",
    "supported_cpu",
    "transport_protocol_version",
    "forbidden_calls",
    "forbidden_dependencies",
    "state_policy",
    "database_write_policy",
];

pub const REQUIRED_APPLICATION_FIELDS: &[&str] = &[
    "application_id",
    "application_name",
    "application_version",
    "application_kind",
    "lifecycle",
    "owned_database_contracts",
    "read_database_contracts",
    "owned_artifacts",
    "module_dependencies",
    "capabilities",
    "supported_os",
    "supported_cpu",
    "transport_protocol_version",
    "keyboard_shortcut_contract",
    "ui_layout_reference_contract",
    "ui_layout_contracts",
    "workflow_forbidden_operations",
    "workflow_forbidden_capabilities",
    "status_contract",
    "error_contract",
];

pub const REQUIRED_UI_LAYOUT_FIELDS: &[&str] = &[
    "layout_id",
    "layout_version",
    "mirror_policy",
    "font_policy",
    "allowed_deviation_policy",
    "qnc_v4_reference",
    "forbidden_changes_without_approval",
];

pub fn parse_qnc_uri(input: &str) -> Result<QncUri, String> {
    let input = input.trim();
    if looks_like_raw_os_path(input) {
        return Err(format!("raw OS path is not a public QNC URI: {input}"));
    }

    let Some(rest) = input.strip_prefix("qnc://") else {
        return Err(format!("QNC URI must start with qnc://: {input}"));
    };

    let parts: Vec<&str> = rest.split('/').collect();
    if parts.iter().any(|part| part.is_empty()) {
        return Err(format!("QNC URI has an empty path segment: {input}"));
    }

    match parts.first().copied() {
        Some("local") => {
            if parts.len() < 3 {
                return Err(format!("local QNC URI requires kind and id: {input}"));
            }
            Ok(QncUri {
                environment: "local".to_string(),
                authority: None,
                resource_kind: parts[1].to_string(),
                resource_id: parts[2..].join("/"),
            })
        }
        Some("lan") | Some("intranet") => {
            if parts.len() < 4 {
                return Err(format!(
                    "LAN/intranet QNC URI requires authority, kind and id: {input}"
                ));
            }
            Ok(QncUri {
                environment: parts[0].to_string(),
                authority: Some(parts[1].to_string()),
                resource_kind: parts[2].to_string(),
                resource_id: parts[3..].join("/"),
            })
        }
        Some(environment) => Err(format!("unsupported QNC environment '{environment}'")),
        None => Err("empty QNC URI".to_string()),
    }
}

pub fn looks_like_raw_os_path(value: &str) -> bool {
    if value.starts_with("qnc://") {
        return false;
    }

    let bytes = value.as_bytes();
    let windows_drive = bytes.len() >= 3
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
        && bytes[0].is_ascii_alphabetic();

    windows_drive || value.starts_with("\\\\") || value.starts_with('/') || value.starts_with("~/")
}

pub fn validate_module_manifest_json(name: &str, contents: &str) -> ValidationReport {
    let mut report = ValidationReport::new();
    let value = match serde_json::from_str::<Value>(contents) {
        Ok(value) => value,
        Err(err) => {
            report.error(format!("{name}: invalid JSON: {err}"));
            return report;
        }
    };

    let Some(object) = value.as_object() else {
        report.error(format!("{name}: module manifest must be a JSON object"));
        return report;
    };

    for field in REQUIRED_MODULE_FIELDS {
        if !object.contains_key(*field) {
            report.error(format!("{name}: missing required module field '{field}'"));
        }
    }

    for key in FORBIDDEN_MODULE_KEYS {
        if object.contains_key(*key) {
            report.error(format!(
                "{name}: module manifest must not hardcode module consumers with '{key}'"
            ));
        }
    }

    require_non_empty_string(&mut report, name, object, "module_id");
    require_non_empty_string(&mut report, name, object, "module_name");
    require_non_empty_array(&mut report, name, object, "capabilities");
    require_non_empty_array(&mut report, name, object, "supported_os");
    require_non_empty_array(&mut report, name, object, "supported_cpu");
    require_array(&mut report, name, object, "forbidden_calls");
    require_array(&mut report, name, object, "forbidden_dependencies");
    require_enum(
        &mut report,
        name,
        object,
        "module_kind",
        &[
            "library",
            "in_process_plugin",
            "out_of_process_helper",
            "network_service",
            "ui_widget",
        ],
    );
    require_enum(
        &mut report,
        name,
        object,
        "state_policy",
        &["stateless", "session_local", "owned_by_calling_application"],
    );
    require_enum(
        &mut report,
        name,
        object,
        "database_write_policy",
        &[
            "no_db_writes",
            "public_db_owner_write_adapter_only",
            "public_db_write_transports_only",
            "public_ingest_content_write_transport_only",
            "narrow_project_close_write_adapter_only",
            "returns_artifact_to_public_write_transport",
        ],
    );
    require_supported_targets(&mut report, name, object);

    if object.get("module_id").and_then(Value::as_str) == Some("qnc.module.timeline") {
        for (field, expected) in [
            ("module_kind", "ui_widget"),
            ("state_policy", "stateless"),
            ("playback_state_source", "broadcast_player"),
            ("intent_policy", "emit_only"),
            ("database_write_policy", "no_db_writes"),
        ] {
            require_enum(&mut report, name, object, field, &[expected]);
        }
        for call in [
            "playback.clock.own",
            "playback.state.own",
            "playback.position.fallback",
            "playback.execute",
            "application.db.read",
            "application.db.write",
            "filmstrip.generate",
            "wave.generate",
            "ffprobe",
            "media.probe.full",
        ] {
            if !object
                .get("forbidden_calls")
                .and_then(Value::as_array)
                .is_some_and(|calls| calls.iter().any(|value| value.as_str() == Some(call)))
            {
                report.error(format!("{name}: passive timeline must forbid '{call}'"));
            }
        }
    }

    report
}

pub fn validate_application_manifest_json(name: &str, contents: &str) -> ValidationReport {
    let mut report = ValidationReport::new();
    let value = match serde_json::from_str::<Value>(contents) {
        Ok(value) => value,
        Err(err) => {
            report.error(format!("{name}: invalid JSON: {err}"));
            return report;
        }
    };

    let Some(object) = value.as_object() else {
        report.error(format!(
            "{name}: application manifest must be a JSON object"
        ));
        return report;
    };

    for field in REQUIRED_APPLICATION_FIELDS {
        if !object.contains_key(*field) {
            report.error(format!(
                "{name}: missing required application field '{field}'"
            ));
        }
    }

    for key in FORBIDDEN_MODULE_KEYS {
        if object.contains_key(*key) {
            report.error(format!(
                "{name}: application manifest must use module_dependencies, not '{key}'"
            ));
        }
    }

    require_non_empty_string(&mut report, name, object, "application_id");
    require_non_empty_string(&mut report, name, object, "application_name");
    require_non_empty_array(&mut report, name, object, "module_dependencies");
    require_non_empty_array(&mut report, name, object, "capabilities");
    require_non_empty_array(&mut report, name, object, "supported_os");
    require_non_empty_array(&mut report, name, object, "supported_cpu");
    require_non_empty_array(&mut report, name, object, "ui_layout_contracts");
    require_array(&mut report, name, object, "workflow_forbidden_operations");
    require_array(&mut report, name, object, "workflow_forbidden_capabilities");
    require_supported_targets(&mut report, name, object);
    validate_application_db_uris(&mut report, name, object, "owned_database_contracts");
    validate_application_db_uris(&mut report, name, object, "read_database_contracts");

    report
}

pub fn validate_keyboard_catalog_json(
    name: &str,
    contents: &str,
) -> Result<KeyboardCatalogSummary, ValidationReport> {
    let mut report = ValidationReport::new();
    let value = match serde_json::from_str::<Value>(contents) {
        Ok(value) => value,
        Err(err) => {
            report.error(format!("{name}: invalid JSON: {err}"));
            return Err(report);
        }
    };

    let Some(object) = value.as_object() else {
        report.error(format!("{name}: keyboard catalog must be a JSON object"));
        return Err(report);
    };

    let version = object.get("version").and_then(Value::as_i64).unwrap_or(-1);
    if version < 1 {
        report.error(format!(
            "{name}: keyboard catalog requires numeric version >= 1"
        ));
    }

    let active_preset = object
        .get("active_preset")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if active_preset.is_empty() {
        report.error(format!("{name}: keyboard catalog requires active_preset"));
    }

    let actions = object.get("actions").and_then(Value::as_object);
    let presets = object.get("presets").and_then(Value::as_object);

    let action_count = actions.map_or(0, |actions| actions.len());
    let preset_count = presets.map_or(0, |presets| presets.len());

    if action_count == 0 {
        report.error(format!("{name}: keyboard catalog requires actions"));
    }
    if preset_count == 0 {
        report.error(format!("{name}: keyboard catalog requires presets"));
    }

    if let Some((actions, presets)) = actions.zip(presets) {
        if !presets.contains_key(&active_preset) {
            report.error(format!(
                "{name}: active preset '{active_preset}' is not defined"
            ));
        }

        for action_id in actions.keys() {
            if action_id.trim().is_empty() {
                report.error(format!("{name}: keyboard action id must not be empty"));
            }
        }

        for (preset_id, preset) in presets {
            let Some(preset_object) = preset.as_object() else {
                report.error(format!("{name}: preset '{preset_id}' must be an object"));
                continue;
            };

            for (scope, bindings_by_action) in preset_object {
                if scope == "name" || scope == "description" {
                    continue;
                }

                let Some(scope_object) = bindings_by_action.as_object() else {
                    report.error(format!(
                        "{name}: preset '{preset_id}' scope '{scope}' must be an object"
                    ));
                    continue;
                };

                for (action_id, bindings) in scope_object {
                    if !actions.contains_key(action_id) {
                        report.error(format!(
                            "{name}: preset '{preset_id}' scope '{scope}' references unknown action '{action_id}'"
                        ));
                    }

                    let Some(binding_array) = bindings.as_array() else {
                        report.error(format!(
                            "{name}: bindings for '{action_id}' must be an array"
                        ));
                        continue;
                    };

                    if binding_array.is_empty() {
                        report.warning(format!("{name}: bindings for '{action_id}' are empty"));
                    }

                    for (index, binding) in binding_array.iter().enumerate() {
                        validate_key_binding(
                            &mut report,
                            name,
                            preset_id,
                            scope,
                            action_id,
                            index,
                            binding,
                        );
                    }
                }
            }
        }
    }

    let summary = KeyboardCatalogSummary {
        version,
        active_preset,
        action_count,
        preset_count,
    };

    if report.is_ok() {
        Ok(summary)
    } else {
        Err(report)
    }
}

pub fn validate_ui_layout_reference_doc(name: &str, contents: &str) -> ValidationReport {
    let mut report = ValidationReport::new();
    for needle in ["qnc_v4", "doslovno", "UI", "layout"] {
        if !contents.contains(needle) {
            report.error(format!(
                "{name}: UI/layout contract must contain '{needle}'"
            ));
        }
    }
    report
}

pub fn validate_ui_layout_contract_json(name: &str, contents: &str) -> ValidationReport {
    let mut report = ValidationReport::new();
    let value = match serde_json::from_str::<Value>(contents) {
        Ok(value) => value,
        Err(err) => {
            report.error(format!("{name}: invalid JSON: {err}"));
            return report;
        }
    };

    let Some(object) = value.as_object() else {
        report.error(format!("{name}: UI layout contract must be a JSON object"));
        return report;
    };

    for field in REQUIRED_UI_LAYOUT_FIELDS {
        if !object.contains_key(*field) {
            report.error(format!(
                "{name}: missing required UI layout field '{field}'"
            ));
        }
    }

    require_non_empty_string(&mut report, name, object, "layout_id");
    require_non_empty_string(&mut report, name, object, "layout_version");
    require_non_empty_array(&mut report, name, object, "qnc_v4_reference");
    require_non_empty_array(
        &mut report,
        name,
        object,
        "forbidden_changes_without_approval",
    );

    require_exact_string(&mut report, name, object, "mirror_policy", "literal_qnc_v4");
    require_exact_string(
        &mut report,
        name,
        object,
        "font_policy",
        "single_qnc_font_system",
    );
    require_exact_string(
        &mut report,
        name,
        object,
        "allowed_deviation_policy",
        "explicit_approval_required",
    );

    if let Some(layout_id) = object.get("layout_id").and_then(Value::as_str) {
        if !layout_id.starts_with("qnc.ui.") {
            report.error(format!(
                "{name}: layout_id '{layout_id}' must start with qnc.ui."
            ));
        }
    }

    if let Some(application_id) = object.get("application_id").and_then(Value::as_str) {
        if !application_id.starts_with("qnc.") || application_id.starts_with("qnc.module.") {
            report.error(format!(
                "{name}: application_id '{application_id}' must be an application id"
            ));
        }
    }

    if let Some(references) = object.get("qnc_v4_reference").and_then(Value::as_array) {
        for reference in references {
            let Some(reference) = reference.as_str() else {
                report.error(format!("{name}: qnc_v4_reference entries must be strings"));
                continue;
            };
            if reference.trim().is_empty() {
                report.error(format!("{name}: qnc_v4_reference entry must not be empty"));
            }
            if looks_like_raw_os_path(reference) || reference.contains('\\') {
                report.error(format!(
                    "{name}: qnc_v4_reference must be repository-relative and OS-neutral: {reference}"
                ));
            }
        }
    }

    report
}

pub fn contains_hardcoded_shortcut_pattern(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("//") {
        return false;
    }

    let mut patterns = ["Ctrl", "Cmd", "Command", "Alt", "Shift"]
        .into_iter()
        .map(|modifier| format!("{modifier}+"))
        .collect::<Vec<_>>();
    patterns.extend([
        format!("{}{}", "egui::", "Key::"),
        format!("{}{}", "keyboard_shortcut", "("),
        format!("{}{}", "shortcut_chord", "("),
    ]);

    patterns.iter().any(|pattern| trimmed.contains(pattern))
}

fn require_supported_targets(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
) {
    require_array_contains(report, name, object, "supported_os", "windows");
    require_array_contains(report, name, object, "supported_os", "linux");
    require_array_contains(report, name, object, "supported_os", "macos");
    require_array_contains(report, name, object, "supported_cpu", "x86_64");
    require_array_contains(report, name, object, "supported_cpu", "aarch64");
}

fn require_non_empty_string(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
) {
    if object
        .get(field)
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        report.error(format!(
            "{name}: field '{field}' must be a non-empty string"
        ));
    }
}

fn require_array(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
) {
    if object.get(field).and_then(Value::as_array).is_none() {
        report.error(format!("{name}: field '{field}' must be an array"));
    }
}

fn require_non_empty_array(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
) {
    match object.get(field).and_then(Value::as_array) {
        Some(values) if !values.is_empty() => {}
        Some(_) => report.error(format!("{name}: field '{field}' must not be empty")),
        None => report.error(format!("{name}: field '{field}' must be an array")),
    }
}

fn require_array_contains(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) {
    let contains = object
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values
                .iter()
                .any(|value| value.as_str().is_some_and(|value| value == expected))
        });

    if !contains {
        report.error(format!("{name}: field '{field}' must contain '{expected}'"));
    }
}

fn require_enum(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
    allowed: &[&str],
) {
    let Some(value) = object.get(field).and_then(Value::as_str) else {
        report.error(format!("{name}: field '{field}' must be a string"));
        return;
    };

    if !allowed.contains(&value) {
        report.error(format!(
            "{name}: field '{field}' value '{value}' is not allowed"
        ));
    }
}

fn require_exact_string(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) {
    let Some(value) = object.get(field).and_then(Value::as_str) else {
        report.error(format!("{name}: field '{field}' must be a string"));
        return;
    };

    if value != expected {
        report.error(format!(
            "{name}: field '{field}' must be exactly '{expected}'"
        ));
    }
}

fn validate_application_db_uris(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
) {
    let Some(values) = object.get(field).and_then(Value::as_array) else {
        report.error(format!("{name}: field '{field}' must be an array"));
        return;
    };

    for value in values {
        let Some(uri) = value.as_str() else {
            report.error(format!("{name}: field '{field}' entries must be strings"));
            continue;
        };
        if let Err(err) = parse_qnc_uri(uri) {
            report.error(format!("{name}: invalid DB URI in '{field}': {err}"));
        }
    }
}

fn validate_key_binding(
    report: &mut ValidationReport,
    name: &str,
    preset_id: &str,
    scope: &str,
    action_id: &str,
    index: usize,
    binding: &Value,
) {
    let Some(object) = binding.as_object() else {
        report.error(format!(
            "{name}: binding {preset_id}/{scope}/{action_id}[{index}] must be an object"
        ));
        return;
    };

    let has_code = object.get("code").and_then(Value::as_str).is_some();
    let has_key = object.get("key").and_then(Value::as_str).is_some();
    if !has_code && !has_key {
        report.error(format!(
            "{name}: binding {preset_id}/{scope}/{action_id}[{index}] needs 'code' or 'key'"
        ));
    }

    for modifier in ["shift", "ctrl", "ctrlKey", "alt"] {
        if object
            .get(modifier)
            .is_some_and(|value| !value.is_boolean())
        {
            report.error(format!(
                "{name}: binding {preset_id}/{scope}/{action_id}[{index}] modifier '{modifier}' must be bool"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qnc_uris() {
        let uri = parse_qnc_uri("qnc://lan/storage-a/media/source_123/clip_456/original")
            .expect("valid QNC URI");
        assert_eq!(uri.environment, "lan");
        assert_eq!(uri.authority.as_deref(), Some("storage-a"));
        assert_eq!(uri.resource_kind, "media");
        assert_eq!(uri.resource_id, "source_123/clip_456/original");
    }

    #[test]
    fn rejects_raw_os_paths_as_public_uri() {
        assert!(parse_qnc_uri(r"C:\media\clip001.mp4").is_err());
        assert!(parse_qnc_uri("/mnt/media/clip001.mp4").is_err());
        assert!(parse_qnc_uri(r"\\server\share\clip001.mp4").is_err());
    }

    #[test]
    fn validates_qnc_v4_keyboard_catalog_copy() {
        let catalog = include_str!("../../../contracts/qnc-keyboard-shortcuts.json");
        let summary =
            validate_keyboard_catalog_json("qnc-keyboard-shortcuts.json", catalog).unwrap();
        assert_eq!(summary.version, 1);
        assert_eq!(summary.active_preset, "default");
        assert!(summary.action_count >= 40);
        assert!(summary.preset_count >= 1);
    }

    #[test]
    fn rejects_module_consumer_allowlist() {
        let report = validate_module_manifest_json(
            "bad.module.json",
            r#"{
                "module_id": "qnc.module.bad",
                "module_name": "bad",
                "module_version": "0.1.0",
                "module_kind": "library",
                "capabilities": ["bad.run"],
                "allowed_applications": ["qnc.ingest"],
                "input_contract_version": "0.1.0",
                "output_contract_version": "0.1.0",
                "supported_os": ["windows", "linux", "macos"],
                "supported_cpu": ["x86_64", "aarch64"],
                "transport_protocol_version": "0.1.0",
                "forbidden_calls": [],
                "forbidden_dependencies": [],
                "state_policy": "stateless",
                "database_write_policy": "no_db_writes"
            }"#,
        );

        assert!(!report.is_ok());
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("allowed_applications")));
    }

    #[test]
    fn validates_passive_timeline_contract() {
        let report = validate_module_manifest_json(
            "timeline.module.json",
            include_str!("../../../contracts/modules/timeline.module.json"),
        );
        assert!(report.is_ok(), "{:?}", report.errors);
    }

    #[test]
    fn rejects_timeline_playback_ownership_and_execution() {
        let original: Value = serde_json::from_str(include_str!(
            "../../../contracts/modules/timeline.module.json"
        ))
        .unwrap();
        for (field, invalid) in [
            ("module_kind", "network_service"),
            ("state_policy", "owned_by_calling_application"),
            ("state_policy", "session_local"),
            ("playback_state_source", "local_fallback"),
            ("intent_policy", "execute"),
            ("database_write_policy", "owner_application_only"),
        ] {
            let mut value = original.clone();
            value[field] = Value::String(invalid.into());
            let report = validate_module_manifest_json("timeline", &value.to_string());
            assert!(!report.is_ok(), "accepted {field}={invalid}");
            assert!(report.errors.iter().any(|error| error.contains(field)));
        }
        for field in ["playback_state_source", "intent_policy"] {
            let mut value = original.clone();
            value.as_object_mut().unwrap().remove(field);
            assert!(!validate_module_manifest_json("timeline", &value.to_string()).is_ok());
        }
    }

    #[test]
    fn rejects_removed_timeline_dependency_boundaries() {
        let original: Value = serde_json::from_str(include_str!(
            "../../../contracts/modules/timeline.module.json"
        ))
        .unwrap();
        for call in original["forbidden_calls"].as_array().unwrap() {
            let mut value = original.clone();
            value["forbidden_calls"]
                .as_array_mut()
                .unwrap()
                .retain(|item| item != call);
            let report = validate_module_manifest_json("timeline", &value.to_string());
            assert!(!report.is_ok(), "accepted removal of {call}");
        }
    }

    #[test]
    fn validates_ui_layout_contract() {
        let report = validate_ui_layout_contract_json(
            "project.layout.json",
            r#"{
                "layout_id": "qnc.ui.project",
                "layout_version": "0.1.0",
                "application_id": "qnc.project",
                "mirror_policy": "literal_qnc_v4",
                "font_policy": "single_qnc_font_system",
                "allowed_deviation_policy": "explicit_approval_required",
                "qnc_v4_reference": ["qnc-app/src/project/screen.rs"],
                "forbidden_changes_without_approval": ["redesign"]
            }"#,
        );

        assert!(report.is_ok(), "{:?}", report.errors);
    }

    #[test]
    fn rejects_raw_paths_in_ui_layout_references() {
        let report = validate_ui_layout_contract_json(
            "bad.layout.json",
            r#"{
                "layout_id": "qnc.ui.bad",
                "layout_version": "0.1.0",
                "mirror_policy": "literal_qnc_v4",
                "font_policy": "single_qnc_font_system",
                "allowed_deviation_policy": "explicit_approval_required",
                "qnc_v4_reference": ["C:\\Users\\miron\\Projects\\qnc_v4\\qnc-app\\src\\app.rs"],
                "forbidden_changes_without_approval": ["redesign"]
            }"#,
        );

        assert!(!report.is_ok());
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("OS-neutral")));
    }
}
