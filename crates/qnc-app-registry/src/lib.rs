//! The registry of QNC applications (`apps/*/qnc-app.json`): loaded, validated and
//! ordered by priority group (moved unchanged out of `qnc-app`, user rule 2026-09-30:
//! everything is a block). Data only: it hosts and starts nothing.

use std::{
    collections::BTreeSet,
    fs,
    path::Path,
};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppManifest {
    pub application_id: String,
    pub tab_id: String,
    pub label: String,
    pub enabled: bool,
    pub system: bool,
    pub removable: bool,
    pub priority_group: String,
    pub host_mode: String,
    pub desktop_entry: String,
    pub standalone_executable: Option<String>,
}

impl AppManifest {
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let manifest = serde_json::from_str::<Self>(&contents)
            .map_err(|error| format!("invalid {}: {error}", path.display()))?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    fn validate(&self, path: &Path) -> Result<(), String> {
        let mut errors = Vec::new();
        for (field, value) in [
            ("application_id", &self.application_id),
            ("tab_id", &self.tab_id),
            ("label", &self.label),
            ("host_mode", &self.host_mode),
            ("desktop_entry", &self.desktop_entry),
        ] {
            if value.trim().is_empty() {
                errors.push(format!("{field} is empty"));
            }
        }
        if !self.application_id.starts_with("qnc.")
            || self.application_id.starts_with("qnc.module.")
        {
            errors.push(format!(
                "application_id '{}' is not a QNC application id",
                self.application_id
            ));
        }
        if !matches!(
            self.host_mode.as_str(),
            "embedded_public_api" | "external_component"
        ) {
            errors.push(format!("unsupported host_mode '{}'", self.host_mode));
        }
        if self.system && self.removable {
            errors.push("system app registry entries cannot be removable".to_string());
        }
        match &self.standalone_executable {
            Some(executable) if !executable.trim().is_empty() => {}
            _ => errors.push(
                "standalone_executable is required because every QNC application must run outside the shell desktop".to_string(),
            ),
        }
        if self.priority_group.len() != 1 || !self.priority_group.as_bytes()[0].is_ascii_lowercase()
        {
            errors.push("priority_group must be a single letter a-z".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("{}: {}", path.display(), errors.join("; ")))
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppRegistry {
    pub entries: Vec<AppManifest>,
}

impl AppRegistry {
    pub fn load(root: &Path) -> Result<Self, String> {
        let apps_dir = root.join("apps");
        let entries = fs::read_dir(&apps_dir)
            .map_err(|error| format!("cannot read {}: {error}", apps_dir.display()))?;

        let mut manifests = Vec::new();
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("cannot read {} entry: {error}", apps_dir.display()))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("qnc-app.json");
            if !manifest_path.is_file() {
                continue;
            }

            let manifest = AppManifest::load(&manifest_path)?;
            if manifest.enabled {
                manifests.push(manifest);
            }
        }

        let mut seen_tabs = BTreeSet::new();
        let mut seen_apps = BTreeSet::new();
        for manifest in &manifests {
            if !seen_tabs.insert(manifest.tab_id.clone()) {
                return Err(format!("duplicate app tab_id '{}'", manifest.tab_id));
            }
            if !seen_apps.insert(manifest.application_id.clone()) {
                return Err(format!(
                    "duplicate app application_id '{}'",
                    manifest.application_id
                ));
            }
        }

        manifests.sort_by(|a, b| {
            a.priority_group
                .cmp(&b.priority_group)
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| a.application_id.cmp(&b.application_id))
        });

        if manifests.is_empty() {
            return Err(format!(
                "no enabled QNC app manifests found in {}",
                apps_dir.display()
            ));
        }

        Ok(Self { entries: manifests })
    }

    pub fn first_tab_id(&self) -> Option<String> {
        self.entries.first().map(|entry| entry.tab_id.clone())
    }

    pub fn find(&self, tab_id: &str) -> Option<&AppManifest> {
        self.entries.iter().find(|entry| entry.tab_id == tab_id)
    }

    pub fn entries(&self) -> &[AppManifest] {
        &self.entries
    }

    pub fn summary(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                format!(
                    "{}:{}:{}",
                    entry.application_id, entry.tab_id, entry.desktop_entry
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, path::PathBuf, process, time::SystemTime};

    #[test]
    fn app_registry_requires_standalone_executable() {
        let root = temp_root("qnc_shell_registry_standalone");
        write_app_manifest(
            &root,
            "qnc-project",
            r#"{
                "application_id": "qnc.project",
                "tab_id": "project",
                "label": "Project",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "a",
                "host_mode": "embedded_public_api",
                "desktop_entry": "qnc_project"
            }"#,
        );

        let error = AppRegistry::load(&root).expect_err("standalone executable is required");
        assert!(error.contains("standalone_executable is required"));
        let _ = fs::remove_dir_all(root);
    }


    #[test]
    fn app_registry_loads_enabled_manifests_in_order() {
        let root = temp_root("qnc_shell_registry_order");
        write_app_manifest(
            &root,
            "qnc-story",
            r#"{
                "application_id": "qnc.story",
                "tab_id": "storyboard",
                "label": "Story",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "c",
                "host_mode": "external_component",
                "desktop_entry": "qnc_story",
                "standalone_executable": "qnc-story"
            }"#,
        );
        write_app_manifest(
            &root,
            "qnc-project",
            r#"{
                "application_id": "qnc.project",
                "tab_id": "project",
                "label": "Project",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "a",
                "host_mode": "embedded_public_api",
                "desktop_entry": "qnc_project",
                "standalone_executable": "qnc-project"
            }"#,
        );

        let registry = AppRegistry::load(&root).expect("registry");
        assert_eq!(
            registry
                .entries()
                .iter()
                .map(|entry| entry.tab_id.as_str())
                .collect::<Vec<_>>(),
            ["project", "storyboard"]
        );
        assert_eq!(registry.first_tab_id().as_deref(), Some("project"));
        let _ = fs::remove_dir_all(root);
    }


    #[test]
    fn app_registry_ignores_disabled_manifests() {
        let root = temp_root("qnc_shell_registry_disabled");
        write_app_manifest(
            &root,
            "qnc-project",
            r#"{
                "application_id": "qnc.project",
                "tab_id": "project",
                "label": "Project",
                "enabled": true,
                "system": true,
                "removable": false,
                "priority_group": "a",
                "host_mode": "embedded_public_api",
                "desktop_entry": "qnc_project",
                "standalone_executable": "qnc-project"
            }"#,
        );
        write_app_manifest(
            &root,
            "qnc-ingest",
            r#"{
                "application_id": "qnc.ingest",
                "tab_id": "ingest",
                "label": "Ingest",
                "enabled": false,
                "system": true,
                "removable": false,
                "priority_group": "b",
                "host_mode": "external_component",
                "desktop_entry": "qnc_ingest",
                "standalone_executable": "qnc-ingest"
            }"#,
        );

        let registry = AppRegistry::load(&root).expect("registry");
        assert_eq!(registry.entries().len(), 1);
        assert_eq!(registry.entries()[0].application_id, "qnc.project");
        let _ = fs::remove_dir_all(root);
    }


    fn temp_root(prefix: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "{}_{}_{}",
            prefix,
            process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn write_app_manifest(root: &Path, app_dir: &str, contents: &str) {
        let app_root = root.join("apps").join(app_dir);
        fs::create_dir_all(&app_root).expect("app dir");
        fs::write(app_root.join("qnc-app.json"), contents).expect("app manifest");
    }
}
