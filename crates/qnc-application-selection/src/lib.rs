use qnc_application_catalog::{self as catalog, ApplicationGroup, Catalog, SelectionSnapshot};
use qnc_transport_resolver::ResolverConfig;
use serde_json::Value;
use std::{
    path::Path,
    sync::mpsc::{self, Receiver},
};

#[derive(Clone, Default)]
pub struct ApplicationSelectionView {
    pub groups: Vec<ApplicationGroup>,
    pub required_groups: Vec<String>,
    pub error: Option<String>,
    pub loading: bool,
}

pub struct ApplicationSelection {
    resolver: ResolverConfig,
    uri: String,
    pending: Option<Receiver<Result<Catalog, String>>>,
    catalog: Option<Catalog>,
    ids: Vec<String>,
    initial_settings: Option<Value>,
    error: Option<String>,
}

impl ApplicationSelection {
    pub fn new(root: &Path) -> Self {
        let uri = std::env::var("QNC_APPLICATION_CATALOG_URI")
            .unwrap_or_else(|_| catalog::DEFAULT_URI.into());
        let mut resolver = ResolverConfig::new(root).with_local_binding(
            catalog::DEFAULT_URI,
            root.join("data").join("application-catalog.json"),
        );
        if let (Ok(parsed), Ok(base)) = (
            qnc_contracts::parse_qnc_uri(&uri),
            std::env::var("QNC_APPLICATION_CATALOG_ENDPOINT"),
        ) {
            if let Some(authority) = parsed.authority {
                resolver = match parsed.environment.as_str() {
                    "lan" => resolver.with_lan_authority(authority, base),
                    "intranet" => resolver.with_intranet_authority(authority, base),
                    _ => resolver,
                };
            }
        }
        let mut result = Self {
            resolver,
            uri,
            pending: None,
            catalog: None,
            ids: vec![],
            initial_settings: None,
            error: None,
        };
        result.load_at_startup();
        result
    }

    fn load_at_startup(&mut self) {
        if self.pending.is_some() {
            return;
        }
        self.catalog = None;
        self.error = None;
        let (tx, rx) = mpsc::channel();
        let resolver = self.resolver.clone();
        let uri = self.uri.clone();
        std::thread::spawn(move || {
            let _ = tx.send(catalog::read_uri(&resolver, &uri).map_err(|e| e.to_string()));
        });
        self.pending = Some(rx);
    }

    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.pending else {
            return false;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return true,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Ucitavanje kataloga je prekinuto.".into())
            }
        };
        self.pending = None;
        match result {
            Ok(catalog) => {
                self.ids.retain(|id| {
                    catalog
                        .applications
                        .iter()
                        .any(|app| &app.application_id == id)
                });
                self.catalog = Some(catalog);
                if let Some(settings) = self.initial_settings.take() {
                    self.set_settings(&settings);
                }
            }
            Err(error) => self.error = Some(error),
        }
        false
    }

    pub fn set_settings(&mut self, settings: &Value) {
        self.ids.clear();
        let Some(catalog) = &self.catalog else {
            self.initial_settings = Some(settings.clone());
            return;
        };
        self.initial_settings = None;
        self.error = None;
        if let Some(raw) = settings.pointer("/workspace/application_selection") {
            match serde_json::from_value::<SelectionSnapshot>(raw.clone()) {
                Ok(selection) => {
                    self.ids = selection
                        .applications
                        .into_iter()
                        .filter(|selected| {
                            catalog
                                .applications
                                .iter()
                                .any(|app| app.application_id == selected.application_id)
                        })
                        .map(|app| app.application_id)
                        .collect()
                }
                Err(error) => self.error = Some(format!("Neispravan spremljeni izbor: {error}")),
            }
        } else if let Some(tabs) = settings
            .pointer("/workspace/tabs")
            .and_then(Value::as_array)
        {
            for tab in tabs.iter().filter_map(Value::as_str) {
                if let Some(app) = catalog.applications.iter().find(|app| app.tab_id == tab) {
                    if !self.ids.contains(&app.application_id) {
                        self.ids.push(app.application_id.clone());
                    }
                }
            }
        }
        let required: Vec<_> = catalog
            .applications
            .iter()
            .filter(|app| app.priority_group == "a")
            .collect();
        if required.len() == 1 && !self.ids.contains(&required[0].application_id) {
            self.ids.push(required[0].application_id.clone());
        }
    }

    pub fn view(&self) -> ApplicationSelectionView {
        ApplicationSelectionView {
            required_groups: vec!["a".into()],
            groups: self
                .catalog
                .as_ref()
                .map(|c| catalog::groups(c, &self.ids))
                .unwrap_or_default(),
            error: self.error.clone(),
            loading: self.pending.is_some(),
        }
    }

    pub fn choose(&mut self, group: &str, id: Option<&str>) -> Result<(), String> {
        if group == "a" && id.is_none() {
            return Err("Grupa a mora imati odabranu aplikaciju.".into());
        }
        let catalog = self.catalog.as_ref().ok_or("Katalog nije ucitan.")?;
        catalog::choose_group(catalog, &mut self.ids, group, id).map_err(|e| e.to_string())
    }

    pub fn snapshot(&self) -> Result<SelectionSnapshot, String> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or("Katalog aplikacija nije ucitan.")?;
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.ids.is_empty() {
            return Err("Odaberite barem jednu aplikaciju.".into());
        }
        let snapshot =
            SelectionSnapshot::from_catalog(catalog, &self.ids).map_err(|e| e.to_string())?;
        if !snapshot
            .applications
            .iter()
            .any(|app| app.priority_group == "a")
        {
            return Err("Odaberite aplikaciju iz grupe a.".into());
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ready() -> ApplicationSelection {
        let catalog: Catalog = serde_json::from_value(json!({
            "catalog_id": "qnc.catalog.applications", "schema_version": 1,
            "catalog_uri": catalog::DEFAULT_URI, "selection_policy": "one_per_priority_group",
            "observed_at_unix_ms": 1, "target_os": "windows", "target_cpu": "x86_64",
            "applications": [
                {"application_id":"qnc.first", "tab_id":"first", "label":"First", "priority_group":"a", "system":false,"removable":true,"host_mode":"external_component","desktop_entry":"first","standalone_executable":"first"},
                {"application_id":"qnc.lite", "tab_id":"lite", "label":"Lite", "priority_group":"b", "system":false,"removable":true,"host_mode":"external_component","desktop_entry":"lite","standalone_executable":"lite"},
                {"application_id":"qnc.full", "tab_id":"full", "label":"Full", "priority_group":"b", "system":false,"removable":true,"host_mode":"external_component","desktop_entry":"full","standalone_executable":"full"}
            ], "unavailable": []
        })).unwrap();
        ApplicationSelection {
            resolver: ResolverConfig::new("unused"),
            uri: catalog::DEFAULT_URI.into(),
            pending: None,
            catalog: Some(catalog),
            ids: vec![],
            initial_settings: None,
            error: None,
        }
    }

    #[test]
    fn missing_apps_never_become_choices_or_block_creation_and_seed_is_unchanged() {
        let mut selection = ready();
        let settings = json!({"workspace":{"tabs":["first", "missing"]}});
        let before = settings.clone();
        selection.set_settings(&settings);
        assert_eq!(selection.view().groups.len(), 2);
        assert!(selection
            .view()
            .groups
            .iter()
            .flat_map(|group| &group.choices)
            .all(|choice| !choice.application_id.contains("missing")));
        assert_eq!(selection.snapshot().unwrap().applications.len(), 1);
        assert_eq!(settings, before);
    }

    #[test]
    fn radio_choice_replaces_same_group_and_template_switch_is_independent() {
        let mut selection = ready();
        selection.set_settings(&json!({"workspace":{"tabs":["first", "lite"]}}));
        selection.choose("b", Some("qnc.full")).unwrap();
        assert_eq!(
            selection.snapshot().unwrap().applications[1].application_id,
            "qnc.full"
        );
        selection.set_settings(&json!({"workspace":{"tabs":["lite"]}}));
        assert_eq!(
            selection.snapshot().unwrap().applications[1].application_id,
            "qnc.lite"
        );
        selection.choose("b", None).unwrap();
        assert!(selection.view().groups[1].no_selection);
        assert_eq!(selection.snapshot().unwrap().applications.len(), 1);
    }

    #[test]
    fn group_a_is_required_and_only_an_available_alternative_can_replace_it() {
        let mut selection = ready();
        selection.set_settings(&json!({"workspace":{"tabs":[]}}));
        assert_eq!(
            selection.snapshot().unwrap().applications[0].application_id,
            "qnc.first"
        );
        assert!(selection.choose("a", None).is_err());
        assert!(selection.choose("a", Some("qnc.missing")).is_err());
        let mut alternative = selection.catalog.as_ref().unwrap().applications[0].clone();
        alternative.application_id = "qnc.alternative".into();
        alternative.tab_id = "alternative".into();
        alternative.label = "Alternative".into();
        selection
            .catalog
            .as_mut()
            .unwrap()
            .applications
            .insert(1, alternative);
        selection.choose("a", Some("qnc.alternative")).unwrap();
        assert_eq!(
            selection.snapshot().unwrap().applications[0].application_id,
            "qnc.alternative"
        );
        assert_eq!(selection.snapshot().unwrap().applications.len(), 1);
        assert!(selection.choose("a", None).is_err());
    }

    #[test]
    fn no_catalog_means_no_choices_and_no_save() {
        let mut selection = ready();
        selection.catalog = None;
        selection.set_settings(&json!({"workspace":{"tabs":["first"]}}));
        assert!(selection.view().groups.is_empty());
        assert!(selection.snapshot().is_err());
    }
}
