use super::*;
use serde_json::json;

struct Fixture {
    root: tempfile::TempDir,
    registrations: PathBuf,
    executables: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let registrations = root.path().join("registrations");
        let executables = root.path().join("executables");
        let output = root.path().join("catalog.json");
        fs::create_dir(&registrations).unwrap();
        fs::create_dir(&executables).unwrap();
        Self {
            root,
            registrations,
            executables,
            output,
        }
    }

    fn app(&self, name: &str, group: &str, enabled: bool, installed: bool) {
        let directory = self.registrations.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("qnc-app.json"),
            serde_json::to_vec(&json!({
                "application_id": format!("qnc.{name}"),
                "tab_id": name,
                "label": name,
                "enabled": enabled,
                "system": false,
                "removable": true,
                "order": 99,
                "priority_group": group,
                "host_mode": "embedded_public_api",
                "desktop_entry": format!("entry_{name}"),
                "standalone_executable": name
            }))
            .unwrap(),
        )
        .unwrap();
        if installed {
            let path = executable_path(&self.executables, name);
            // Presence fixture, intentionally never launched as an application.
            fs::write(&path, "executable-presence fixture").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
    }

    fn refresh(&self) -> Result<Catalog> {
        refresh(
            &self.registrations,
            &self.executables,
            &self.output,
            DEFAULT_URI,
        )
    }
}

#[test]
fn includes_only_enabled_installed_registrations() {
    let f = Fixture::new();
    f.app("variant", "b", true, true);
    f.app("missing", "c", true, false);
    f.app("disabled", "d", false, true);
    fs::create_dir(f.registrations.join("unregistered_source_code")).unwrap();
    fs::write(
        f.registrations.join("unregistered_source_code/Cargo.toml"),
        "[package]",
    )
    .unwrap();
    let catalog = f.refresh().unwrap();
    assert_eq!(catalog.applications.len(), 1);
    assert_eq!(catalog.applications[0].application_id, "qnc.variant");
    assert_eq!(catalog.unavailable.len(), 2);
    assert_eq!(
        catalog.unavailable[0].reason,
        UnavailableReason::MissingExecutable
    );
    assert_eq!(catalog.unavailable[1].reason, UnavailableReason::Disabled);
}

#[test]
fn groups_sort_alphabetically_and_variants_share_group() {
    let f = Fixture::new();
    f.app("third", "d", true, true);
    f.app("standard", "b", true, true);
    f.app("first", "a", true, true);
    f.app("extended", "b", true, true);
    let catalog = f.refresh().unwrap();
    let groups: Vec<_> = catalog
        .applications
        .iter()
        .map(|app| app.priority_group.as_str())
        .collect();
    assert_eq!(groups, ["a", "b", "b", "d"]);
    assert_eq!(catalog.applications[1].application_id, "qnc.extended");
}

#[test]
fn selection_claims_group_and_rejects_another_variant() {
    let f = Fixture::new();
    f.app("short", "b", true, true);
    f.app("standard", "b", true, true);
    f.app("extended", "b", true, true);
    let catalog = f.refresh().unwrap();
    assert!(select(&catalog, &["qnc.short"]).is_ok());
    assert!(select(&catalog, &["qnc.short", "qnc.standard"])
        .unwrap_err()
        .to_string()
        .contains("Group b"));
    assert!(select(&catalog, &["qnc.extended", "qnc.short"]).is_err());
    // A different template (or a deselected variant) does not inherit a reservation.
    assert!(select(&catalog, &["qnc.standard"]).is_ok());
}

#[test]
fn selection_uses_group_order_not_click_order_and_allows_gaps() {
    let f = Fixture::new();
    for (name, group) in [("first", "a"), ("third", "c"), ("fourth", "d")] {
        f.app(name, group, true, true);
    }
    let catalog = f.refresh().unwrap();
    let selected = select(&catalog, &["qnc.fourth", "qnc.first", "qnc.third"]).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|app| app.priority_group.as_str())
            .collect::<Vec<_>>(),
        ["a", "c", "d"]
    );
}

#[test]
fn selection_rejects_unavailable_unknown_and_duplicate_ids() {
    let f = Fixture::new();
    f.app("present", "a", true, true);
    f.app("missing", "b", true, false);
    f.app("disabled", "c", false, true);
    let catalog = f.refresh().unwrap();
    for ids in [
        vec!["qnc.missing"],
        vec!["qnc.disabled"],
        vec!["qnc.unknown"],
        vec!["qnc.present", "qnc.present"],
    ] {
        assert!(select(&catalog, &ids).is_err());
    }
    assert!(select(&catalog, &[]).unwrap().is_empty());
}

#[test]
fn selection_is_read_only_and_does_not_modify_published_file() {
    let f = Fixture::new();
    f.app("variant", "b", true, true);
    let catalog = f.refresh().unwrap();
    let before = fs::read(&f.output).unwrap();
    select(&catalog, &["qnc.variant"]).unwrap();
    assert_eq!(before, fs::read(&f.output).unwrap());
}

#[test]
fn removed_binary_disappears_from_available_snapshot_on_refresh() {
    let f = Fixture::new();
    f.app("variant", "b", true, true);
    f.refresh().unwrap();
    fs::remove_file(executable_path(&f.executables, "variant")).unwrap();
    f.refresh().unwrap();
    let catalog = read_catalog(&f.output).unwrap();
    assert!(catalog.applications.is_empty());
    assert_eq!(
        catalog.unavailable[0].reason,
        UnavailableReason::MissingExecutable
    );
}

#[test]
fn removed_registration_does_not_survive_refresh() {
    let f = Fixture::new();
    f.app("variant", "b", true, true);
    f.refresh().unwrap();
    fs::remove_file(f.registrations.join("variant/qnc-app.json")).unwrap();
    let catalog = f.refresh().unwrap();
    assert!(catalog.applications.is_empty());
    assert!(catalog.unavailable.is_empty());
}

#[test]
fn malformed_registration_preserves_previous_snapshot() {
    let f = Fixture::new();
    f.app("variant", "b", true, true);
    f.refresh().unwrap();
    let before = fs::read(&f.output).unwrap();
    fs::write(f.registrations.join("variant/qnc-app.json"), "{").unwrap();
    assert!(f.refresh().is_err());
    assert_eq!(fs::read(&f.output).unwrap(), before);
}

#[test]
fn missing_input_is_error_not_a_successful_empty_catalog() {
    let f = Fixture::new();
    f.refresh().unwrap();
    let before = fs::read(&f.output).unwrap();
    assert!(refresh(
        &f.root.path().join("missing"),
        &f.executables,
        &f.output,
        DEFAULT_URI
    )
    .is_err());
    assert_eq!(fs::read(&f.output).unwrap(), before);
}

#[test]
fn malformed_groups_are_rejected_without_fallback_to_numeric_order() {
    for group in ["", "1", "ab", "B", " ", "../"] {
        let f = Fixture::new();
        f.app("variant", group, true, true);
        assert!(f.refresh().is_err(), "accepted {group:?}");
        assert!(!f.output.exists());
    }
}

#[test]
fn executable_must_be_a_neutral_basename() {
    for basename in ["../app", "C:\\app", "/usr/app", "app.exe", "app --flag"] {
        let f = Fixture::new();
        f.app("variant", "b", true, true);
        let path = f.registrations.join("variant/qnc-app.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["standalone_executable"] = json!(basename);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(f.refresh().is_err(), "accepted {basename:?}");
    }
}

#[test]
fn duplicate_registered_id_or_tab_is_rejected() {
    for key in ["application_id", "tab_id"] {
        let f = Fixture::new();
        f.app("first", "a", true, true);
        f.app("second", "b", true, true);
        let path = f.registrations.join("second/qnc-app.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value[key] = json!(if key == "application_id" {
            "qnc.first"
        } else {
            "first"
        });
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(f.refresh().is_err());
    }
}

#[test]
fn empty_files_and_directories_are_not_executables() {
    let f = Fixture::new();
    f.app("empty", "a", true, true);
    fs::write(executable_path(&f.executables, "empty"), "").unwrap();
    f.app("directory", "b", true, false);
    fs::create_dir(executable_path(&f.executables, "directory")).unwrap();
    let catalog = f.refresh().unwrap();
    assert!(catalog.applications.is_empty());
    assert!(catalog
        .unavailable
        .iter()
        .all(|app| app.reason == UnavailableReason::NotExecutable));
}

#[cfg(unix)]
#[test]
fn unix_requires_execute_permission() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.app("variant", "b", true, true);
    fs::set_permissions(
        executable_path(&f.executables, "variant"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(f.refresh().unwrap().applications.is_empty());
}

#[test]
fn all_three_deployment_uris_use_the_same_artifact_contract() {
    let f = Fixture::new();
    f.app("variant", "b", true, true);
    for uri in [
        DEFAULT_URI,
        "qnc://lan/studio/catalog/applications",
        "qnc://intranet/studio/catalog/applications",
    ] {
        let catalog = discover(&f.registrations, &f.executables, uri).unwrap();
        assert_eq!(catalog.catalog_uri, uri);
        assert_eq!(catalog.applications.len(), 1);
        let json = serde_json::to_string(&catalog).unwrap();
        assert!(!json.contains(&f.root.path().to_string_lossy().replace('\\', "\\\\")));
        assert!(!json.contains("local_path"));
    }
}

#[test]
fn raw_paths_and_wrong_resource_uris_are_rejected() {
    for uri in [
        "C:\\catalog.json",
        "/tmp/catalog.json",
        "qnc://local/db/applications",
        "qnc://lan/../catalog/applications",
        "qnc://intranet/studio/catalog/../applications",
    ] {
        assert!(validate_uri(uri).is_err(), "accepted {uri:?}");
    }
}

#[test]
fn publication_refuses_unrelated_file_or_another_catalog_uri() {
    let f = Fixture::new();
    fs::write(&f.output, "user document").unwrap();
    assert!(f.refresh().is_err());
    assert_eq!(fs::read_to_string(&f.output).unwrap(), "user document");
    fs::remove_file(&f.output).unwrap();
    let catalog = f.refresh().unwrap();
    let other = Catalog {
        catalog_uri: "qnc://lan/studio/catalog/applications".into(),
        ..catalog
    };
    assert!(publish(&other, &f.output).is_err());
    assert_eq!(read_catalog(&f.output).unwrap().catalog_uri, DEFAULT_URI);
}

#[test]
fn reader_rejects_invalid_identity_schema_policy_and_order() {
    let f = Fixture::new();
    f.app("first", "a", true, true);
    f.app("second", "b", true, true);
    let mut catalog = f.refresh().unwrap();
    catalog.applications.swap(0, 1);
    assert!(validate_catalog(&catalog).is_err());
    catalog.applications.swap(0, 1);
    catalog.schema_version += 1;
    assert!(validate_catalog(&catalog).is_err());
    catalog.schema_version = SCHEMA_VERSION;
    catalog.selection_policy = "any".into();
    assert!(validate_catalog(&catalog).is_err());
    catalog.selection_policy = "one_per_priority_group".into();
    catalog.catalog_id = "qnc.catalog.other".into();
    assert!(validate_catalog(&catalog).is_err());
}

#[test]
fn read_only_inspection_never_creates_a_missing_file() {
    let f = Fixture::new();
    assert!(read_catalog(&f.output).is_err());
    assert!(!f.output.exists());
}

#[test]
fn publisher_has_a_valid_module_and_artifact_contract() {
    let manifest = include_str!("../../../contracts/modules/application-catalog.module.json");
    let report = qnc_contracts::validate_module_manifest_json("application-catalog", manifest);
    assert!(report.is_ok(), "{:?}", report.errors);
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("../../../catalogs/applications/contract.json")).unwrap();
    assert_eq!(contract["catalog_id"], CATALOG_ID);
    assert_eq!(contract["schema_version"], SCHEMA_VERSION);
    assert_eq!(contract["selection_policy"], "one_per_priority_group");
    let cargo = include_str!("../Cargo.toml");
    for forbidden in ["qnc-project", "qnc-ingest", "eframe", "rusqlite"] {
        assert!(!cargo.contains(forbidden));
    }
}
