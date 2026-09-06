use crate::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectedApplication {
    pub application_id: String,
    pub tab_id: String,
    pub label: String,
    pub priority_group: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionSnapshot {
    pub schema_version: u32,
    pub catalog_uri: String,
    pub observed_at_unix_ms: u64,
    pub applications: Vec<SelectedApplication>,
}

impl SelectionSnapshot {
    pub fn validate(&self) -> Result<()> {
        validate_uri(&self.catalog_uri)?;
        if self.schema_version != 1 || self.observed_at_unix_ms == 0 {
            return Err("Invalid application selection version/observation".into());
        }
        let mut ids = HashSet::new();
        let mut tabs = HashSet::new();
        let mut previous = None;
        for app in &self.applications {
            identity(&app.application_id, &app.label, &app.priority_group)?;
            token(&app.tab_id, true)?;
            if !ids.insert(&app.application_id)
                || !tabs.insert(&app.tab_id)
                || previous.is_some_and(|p: &str| p >= app.priority_group.as_str())
            {
                return Err(
                    "Selection must have unique apps/tabs and strictly ascending groups".into(),
                );
            }
            previous = Some(app.priority_group.as_str());
        }
        Ok(())
    }

    pub fn from_catalog(catalog: &Catalog, ids: &[String]) -> Result<Self> {
        let refs: Vec<_> = ids.iter().map(String::as_str).collect();
        let applications = select(catalog, &refs)?
            .into_iter()
            .map(|app| SelectedApplication {
                application_id: app.application_id.clone(),
                tab_id: app.tab_id.clone(),
                label: app.label.clone(),
                priority_group: app.priority_group.clone(),
            })
            .collect();
        Ok(Self {
            schema_version: 1,
            catalog_uri: catalog.catalog_uri.clone(),
            observed_at_unix_ms: catalog.observed_at_unix_ms,
            applications,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ApplicationChoice {
    pub application_id: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct ApplicationGroup {
    pub priority_group: String,
    pub choices: Vec<ApplicationChoice>,
    pub no_selection: bool,
}

pub fn groups(catalog: &Catalog, ids: &[String]) -> Vec<ApplicationGroup> {
    let mut result = Vec::<ApplicationGroup>::new();
    for app in &catalog.applications {
        if result
            .last()
            .is_none_or(|group| group.priority_group != app.priority_group)
        {
            result.push(ApplicationGroup {
                priority_group: app.priority_group.clone(),
                choices: vec![],
                no_selection: true,
            });
        }
        let group = result.last_mut().expect("group inserted");
        let selected = ids.contains(&app.application_id);
        group.no_selection &= !selected;
        group.choices.push(ApplicationChoice {
            application_id: app.application_id.clone(),
            label: app.label.clone(),
            selected,
        });
    }
    // An invalid old template must not paint two radio buttons as selected.
    for group in &mut result {
        if group
            .choices
            .iter()
            .filter(|choice| choice.selected)
            .count()
            > 1
        {
            for choice in &mut group.choices {
                choice.selected = false;
            }
        }
    }
    result
}

pub fn choose_group(
    catalog: &Catalog,
    ids: &mut Vec<String>,
    group: &str,
    id: Option<&str>,
) -> Result<()> {
    validate_catalog(catalog)?;
    let members: Vec<_> = catalog
        .applications
        .iter()
        .filter(|app| app.priority_group == group)
        .collect();
    if members.is_empty() {
        return Err("Group is not available".into());
    }
    if let Some(id) = id {
        if !members.iter().any(|app| app.application_id == id) {
            return Err("Application is not available in this group".into());
        }
    }
    let mut next = ids.clone();
    next.retain(|id| !members.iter().any(|app| &app.application_id == id));
    if let Some(id) = id {
        next.push(id.to_owned());
    }
    *ids = next;
    Ok(())
}
