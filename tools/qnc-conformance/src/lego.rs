//! Target picture rules C1-C13 (docs/89-qnc-target-lego-catalog.md).
//!
//! A form is only a board of public lego pieces; the pieces are public and
//! universal. Every rule counts its violations per key. Known violations live in
//! `tools/qnc-conformance/lego-baseline.json`: a count above the baseline (or a new
//! key) is an error, a count below it is a warning to lower the baseline. The
//! baseline may only shrink.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use qnc_contracts::ValidationReport;

use super::{
    cargo_package_index, collect_json_files, collect_rs_files, display_relative,
    is_test_or_example_path,
};

pub(crate) const BASELINE: &str = "tools/qnc-conformance/lego-baseline.json";

const FORM_CRATES: [&str; 3] = [
    "qnc-ingest-desktop",
    "qnc-editorial-desktop",
    "qnc-project-desktop",
];
const APPLICATION_CRATES: [&str; 3] = [
    "qnc-ingest-application",
    "qnc-editorial-application",
    "qnc-project-application",
];
/// The public port through which a player announces that it prepares or plays.
/// `qnc-ingest-runtime` is its current name until package B makes it neutral.
const PLAYBACK_ACTIVITY: [&str; 2] = ["qnc_playback_activity", "qnc_ingest_runtime"];
const PLAYBACK_ACTIVITY_CRATES: [&str; 2] = ["qnc-playback-activity", "qnc-ingest-runtime"];
const SKIPPED_PACKAGES: [&str; 1] = ["qnc-conformance"];

pub(crate) fn check(root: &Path) -> ValidationReport {
    let mut report = ValidationReport::new();
    let counts = current_counts(root);
    let baseline = match read_baseline(root) {
        Ok(baseline) => baseline,
        Err(error) => {
            report.error(error);
            return report;
        }
    };
    compare(&counts, &baseline, &mut report);
    report
}

/// Current violation counts, printed by `qnc-conformance --print-lego-counts`.
pub(crate) fn current_counts(root: &Path) -> BTreeMap<String, u64> {
    let packages = packages(root);
    let mut counts = BTreeMap::new();
    form_rules(root, &packages, &mut counts);
    public_piece_users(root, &packages, &mut counts);
    application_name_users(&packages, &mut counts);
    preview_host_databases(&packages, &mut counts);
    playback_activity(root, &packages, &mut counts);
    application_size(root, &packages, &mut counts);
    single_readers(root, &packages, &mut counts);
    fallback_paths(root, &packages, &mut counts);
    project_layout_literals(root, &packages, &mut counts);
    counts.retain(|_, count| *count > 0);
    counts
}

fn compare(
    counts: &BTreeMap<String, u64>,
    baseline: &BTreeMap<String, u64>,
    report: &mut ValidationReport,
) {
    for (key, count) in counts {
        let allowed = baseline.get(key).copied().unwrap_or(0);
        if *count > allowed {
            report.error(format!(
                "{key}: {count} (baseline {allowed}); new violations are not allowed, see docs/89-qnc-target-lego-catalog.md"
            ));
        }
    }
    for (key, allowed) in baseline {
        let count = counts.get(key).copied().unwrap_or(0);
        if count < *allowed {
            if count == 0 {
                report.warning(format!("{key}: fixed; remove it from {BASELINE}"));
            } else {
                report.warning(format!("{key}: {count} (baseline {allowed}); lower it in {BASELINE}"));
            }
        }
    }
}

fn read_baseline(root: &Path) -> Result<BTreeMap<String, u64>, String> {
    let path = root.join(BASELINE);
    let text = fs::read_to_string(&path).map_err(|_| format!("{BASELINE} is missing"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("{BASELINE}: {error}"))?;
    let exceptions = value
        .get("exceptions")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| format!("{BASELINE} needs an `exceptions` object"))?;
    exceptions
        .iter()
        .map(|(key, count)| {
            count
                .as_u64()
                .map(|count| (key.clone(), count))
                .ok_or_else(|| format!("{BASELINE}: `{key}` must be a whole number"))
        })
        .collect()
}

struct Package {
    name: String,
    dir: PathBuf,
    runtime_deps: Vec<String>,
}

fn packages(root: &Path) -> BTreeMap<String, Package> {
    let index = cargo_package_index(root);
    index
        .iter()
        .filter(|(name, _)| !SKIPPED_PACKAGES.contains(&name.as_str()))
        .filter_map(|(name, cargo_toml)| {
            let contents = fs::read_to_string(cargo_toml).ok()?;
            Some((
                name.clone(),
                Package {
                    name: name.clone(),
                    dir: cargo_toml.parent()?.to_path_buf(),
                    runtime_deps: runtime_dependency_names(&contents, &index),
                },
            ))
        })
        .collect()
}

/// `[dependencies]` entries that are workspace packages, in both the
/// `name = { ... }` and the dotted `name.workspace = true` form.
fn runtime_dependency_names(contents: &str, index: &BTreeMap<String, PathBuf>) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_dependencies = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, _)) = trimmed.split_once('=') else {
            continue;
        };
        let name = key.trim().split('.').next().unwrap_or("").trim();
        if index.contains_key(name) && !names.iter().any(|known| known == name) {
            names.push(name.to_string());
        }
    }
    names
}

/// Runtime source of one package: test files, examples and `#[cfg(test)]` items removed.
fn runtime_files(root: &Path, package: &Package) -> Vec<(String, String)> {
    let mut files = Vec::new();
    collect_rs_files(&package.dir.join("src"), &mut files);
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            let relative = display_relative(root, &path);
            if is_test_or_example_path(&relative) || relative.contains("/target/") {
                return None;
            }
            let contents = fs::read_to_string(&path).ok()?;
            Some((relative, strip_test_code(&contents)))
        })
        .collect()
}

fn runtime_text(root: &Path, package: &Package) -> String {
    runtime_files(root, package)
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Removes every item annotated with `#[cfg(test)]`, whole files marked
/// `#![cfg(test)]`, and line comments.
fn strip_test_code(contents: &str) -> String {
    if contents.trim_start().starts_with("#![cfg(test)]") {
        return String::new();
    }
    let mut out = String::new();
    let mut lines = contents.lines();
    while let Some(line) = lines.next() {
        if line.trim() != "#[cfg(test)]" {
            if !line.trim_start().starts_with("//") {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        // Skip the annotated item: one line ending in `;`, or a braced block.
        let mut depth = 0i64;
        let mut opened = false;
        for item_line in lines.by_ref() {
            if item_line.trim().is_empty() || item_line.trim_start().starts_with("#[") {
                continue;
            }
            for ch in item_line.chars() {
                match ch {
                    '{' => {
                        depth += 1;
                        opened = true;
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if (opened && depth <= 0) || (!opened && item_line.trim_end().ends_with(';')) {
                break;
            }
        }
    }
    out
}

fn add(counts: &mut BTreeMap<String, u64>, key: String, amount: u64) {
    if amount > 0 {
        *counts.entry(key).or_insert(0) += amount;
    }
}

fn occurrences(text: &str, needles: &[&str]) -> u64 {
    needles
        .iter()
        .map(|needle| text.matches(needle).count() as u64)
        .sum()
}

/// C1-C4: a form is only a board.
fn form_rules(root: &Path, packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    for form in FORM_CRATES {
        let Some(package) = packages.get(form) else {
            continue;
        };
        for (file, text) in runtime_files(root, package) {
            add(counts, format!("C1 form paints primitives|{file}"), occurrences(&text, &["painter()"]));
            add(counts, format!("C2 form colour outside contract|{file}"), occurrences(&text, &["from_rgb"]));
            add(counts, format!("C2 form user text outside contract|{file}"), user_text_literals(&text));
            add(
                counts,
                format!("C3 form builds piece input from domain data|{file}"),
                occurrences(&text, &["CardRow {", "pipeline_statuses", "SaveState::", "ImportStatus"]),
            );
            add(
                counts,
                format!("C4 form owns a loop or key table|{file}"),
                occurrences(
                    &text,
                    &[
                        "egui::Key",
                        "Key::",
                        "consume_egui_action_presses",
                        "egui_shortcut_events",
                        "request_repaint_after",
                        "notify_on_player_change",
                    ],
                ),
            );
        }
    }
}

/// String literals that look like text a user reads: they start with a capital
/// letter, contain a lower-case letter and are not keys, paths or format-only.
fn user_text_literals(text: &str) -> u64 {
    let mut count = 0;
    let mut chars = text.chars().peekable();
    let mut previous = ' ';
    while let Some(ch) = chars.next() {
        if ch != '"' || previous == '\\' || previous == '\'' {
            previous = ch;
            continue;
        }
        let mut literal = String::new();
        let mut escaped = false;
        for next in chars.by_ref() {
            if escaped {
                escaped = false;
                literal.push(next);
                continue;
            }
            match next {
                '\\' => escaped = true,
                '"' => break,
                _ => literal.push(next),
            }
        }
        previous = '"';
        let first = literal.chars().next();
        let looks_like_text = first.is_some_and(char::is_uppercase)
            && literal.chars().any(char::is_lowercase)
            && !["::", "/", "_", "\\"].iter().any(|n| literal.contains(n));
        if looks_like_text {
            count += 1;
        }
    }
    count
}

/// C5: every public piece with a crate has a runtime user.
fn public_piece_users(root: &Path, packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    let mut users: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for package in packages.values() {
        for dep in &package.runtime_deps {
            users.entry(dep.as_str()).or_default().insert(package.name.as_str());
        }
    }
    let mut manifests = Vec::new();
    collect_json_files(&root.join("contracts").join("modules"), &mut manifests);
    for manifest in manifests {
        let Some(stem) = manifest
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".module.json"))
        else {
            continue;
        };
        let crate_name = format!("qnc-{stem}");
        let Some(package) = packages.get(&crate_name) else {
            continue;
        };
        let reserved = fs::read_to_string(&manifest)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| value.get("reserved").and_then(serde_json::Value::as_bool))
            .unwrap_or(false);
        let process_only = package.dir.join("src").join("main.rs").is_file()
            && !package.dir.join("src").join("lib.rs").is_file();
        let used = users.get(crate_name.as_str()).is_some_and(|set| !set.is_empty());
        if !used && !reserved && !process_only {
            add(counts, format!("C5 public piece without a user|{crate_name}"), 1);
        }
    }
}

/// C6: a piece named after an application is used only by that application.
fn application_name_users(packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    for package in packages.values() {
        if package.name.starts_with("qnc-ingest") {
            continue;
        }
        for dep in &package.runtime_deps {
            // The shell hosts every application only through its public desktop adapter (§3).
            let shell_adapter = package.name == "qnc-app" && dep.ends_with("-desktop-adapter");
            if dep.starts_with("qnc-ingest") && !shell_adapter {
                add(counts, format!("C6 application-named piece used elsewhere|{} -> {dep}", package.name), 1);
            }
        }
    }
}

fn runtime_closure(start: &str, packages: &BTreeMap<String, Package>) -> BTreeSet<String> {
    let mut visited = BTreeSet::new();
    let mut stack = vec![start.to_string()];
    while let Some(name) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(package) = packages.get(&name) {
            stack.extend(package.runtime_deps.iter().cloned());
        }
    }
    visited
}

/// C7: the preview and player path never opens a host database.
fn preview_host_databases(packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    let closure = runtime_closure("qnc-source-preview", packages);
    for host_db in ["qnc-media-record-db", "qnc-source-index-db"] {
        if closure.contains(host_db) {
            add(counts, format!("C7 preview reaches a host database|qnc-source-preview -> {host_db}"), 1);
        }
    }
}

/// C8, C9: players announce themselves, generators give way to them.
fn playback_activity(root: &Path, packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    for package in packages.values() {
        let text = runtime_text(root, package);
        if text.contains("ProjectArtifacts::new") && !text.contains("set_playback_priority") {
            add(
                counts,
                format!("C8 artifact generation ignores the player|{}", package.name),
                1,
            );
        }
    }
    for driver in ["qnc-ingest-import-worker", "qnc-ingest-worker"] {
        let Some(package) = packages.get(driver) else {
            continue;
        };
        let text = runtime_text(root, package);
        if !PLAYBACK_ACTIVITY.iter().any(|port| text.contains(port)) {
            add(counts, format!("C8 generator process ignores the player|{driver}"), 1);
        }
    }
    if let Some(preview) = packages.get("qnc-source-preview") {
        let announces = preview
            .runtime_deps
            .iter()
            .any(|dep| PLAYBACK_ACTIVITY_CRATES.contains(&dep.as_str()));
        if !announces {
            add(counts, "C9 preview does not announce playback|qnc-source-preview".into(), 1);
        }
    }
}

/// C10: application layers and forms never grow.
fn application_size(root: &Path, packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    for name in APPLICATION_CRATES.iter().chain(FORM_CRATES.iter()) {
        let Some(package) = packages.get(*name) else {
            continue;
        };
        let lines = runtime_text(root, package)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count() as u64;
        add(counts, format!("C10 size ceiling (runtime lines)|{name}"), lines);
    }
}

/// C11: one catalog reader, one poster path.
fn single_readers(root: &Path, packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    let rules: [(&str, &[&str], &[&str]); 2] = [
        (
            "C11 second catalog reader",
            &["list_summary(", "public_clips"],
            &["qnc-content-store", "qnc-content-read", "qnc-ingest-store"],
        ),
        (
            "C11 second poster path",
            &["ThumbnailBatchService"],
            &["qnc-media-thumbnail", "qnc-clip-posters"],
        ),
    ];
    for package in packages.values() {
        let text = runtime_text(root, package);
        for (rule, needles, owners) in rules {
            if owners.contains(&package.name.as_str()) {
                continue;
            }
            add(counts, format!("{rule}|{}", package.name), occurrences(&text, needles));
        }
    }
}

/// C12: readers have no silent fallback paths.
fn fallback_paths(root: &Path, packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    for package in packages.values() {
        // The Project owner creates the registry; diagnostics logs are not project data.
        if ["qnc-project-store", "qnc-dev-diagnostics"].contains(&package.name.as_str()) {
            continue;
        }
        let text = runtime_text(root, package);
        add(
            counts,
            format!("C12 fallback path under the QNC root|{}", package.name),
            occurrences(&text, &["join(\"data\")"]),
        );
    }
}

/// C13: the project layout comes from the settings.
fn project_layout_literals(root: &Path, packages: &BTreeMap<String, Package>, counts: &mut BTreeMap<String, u64>) {
    for package in packages.values() {
        if package.name == "qnc-work-settings" {
            continue;
        }
        let text = runtime_text(root, package);
        add(
            counts,
            format!("C13 project layout literal|{}", package.name),
            occurrences(&text, &["\"products/"]),
        );
        if ["qnc-ingest-import-worker", "qnc-project-store"].contains(&package.name.as_str()) {
            add(
                counts,
                format!("C13 media folder literal|{}", package.name),
                occurrences(&text, &["\"original\"", "\"proxy\""]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_items_are_removed_but_the_rest_stays() {
        let source = "fn a() {}\n#[cfg(test)]\nmod x;\nfn b() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let _ = 1; }\n}\nfn c() {}\n// painter()\n";
        let stripped = strip_test_code(source);
        assert!(stripped.contains("fn a()"));
        assert!(stripped.contains("fn b()"));
        assert!(stripped.contains("fn c()"));
        assert!(!stripped.contains("mod x"));
        assert!(!stripped.contains("fn t()"));
        assert!(!stripped.contains("painter()"));
    }

    #[test]
    fn user_text_is_counted_and_keys_are_not() {
        let text = r#"label("Uvezi"); key("ingest_import"); path("products/x"); f("Nema diskova."); t("::X")"#;
        assert_eq!(user_text_literals(text), 2);
    }

    #[test]
    fn growth_is_an_error_and_shrinking_is_a_warning() {
        let baseline = BTreeMap::from([("a".to_string(), 2), ("b".to_string(), 1)]);
        let mut report = ValidationReport::new();
        compare(
            &BTreeMap::from([("a".to_string(), 3), ("c".to_string(), 1)]),
            &baseline,
            &mut report,
        );
        assert_eq!(report.errors.len(), 2);
        assert_eq!(report.warnings.len(), 1);

        let mut report = ValidationReport::new();
        compare(&BTreeMap::from([("a".to_string(), 1)]), &baseline, &mut report);
        assert!(report.is_ok());
        assert_eq!(report.warnings.len(), 2);
    }
}
