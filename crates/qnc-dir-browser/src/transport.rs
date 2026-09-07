use crate::{BrowserEntry, BrowserState};
use qnc_source_reader::{EntryKind, SourceReader, SourceReference, MAX_DIRECTORY_ENTRIES};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct BrowserSource {
    pub entry: BrowserEntry,
    pub reader: SourceReader,
}

/// Registered source bindings, independent of any application or confirmation UI.
#[derive(Debug, Clone, Default)]
pub struct TransportBrowserSession {
    sources: Vec<BrowserSource>,
    visible: BTreeMap<String, SourceReference>,
    current: Option<SourceReference>,
    environment: String,
}

impl TransportBrowserSession {
    pub fn new(sources: Vec<BrowserSource>) -> Result<Self, String> {
        let mut ids = std::collections::BTreeSet::new();
        for source in &sources {
            if source.entry.qnc_uri != source.reader.source_uri()
                || !ids.insert(&source.entry.qnc_uri)
            {
                return Err("invalid or duplicate browser source".into());
            }
        }
        Ok(Self {
            sources,
            ..Self::default()
        })
    }

    pub fn roots(&mut self, environment: &str) -> Result<BrowserState, String> {
        if !matches!(environment, "local" | "lan" | "intranet") {
            return Err("invalid source environment".into());
        }
        self.environment = environment.into();
        self.current = None;
        self.visible.clear();
        let mut entries = Vec::new();
        for source in &self.sources {
            let uri = &source.entry.qnc_uri;
            if qnc_contracts::parse_qnc_uri(uri)?.environment == environment {
                self.visible.insert(
                    uri.clone(),
                    source.reader.reference(".").map_err(|e| e.to_string())?,
                );
                entries.push(source.entry.clone());
            }
        }
        Ok(BrowserState {
            roots: true,
            entries,
            ..BrowserState::default()
        })
    }

    pub fn selected(&self, uri: &str) -> Option<SourceReference> {
        self.current.as_ref().filter(|r| r.uri() == uri).cloned()
    }

    pub fn open(&mut self, uri: &str) -> Result<BrowserState, String> {
        let reference = self
            .visible
            .get(uri)
            .cloned()
            .ok_or("unknown browser location")?;
        self.list(reference)
    }

    pub fn parent(&mut self) -> Result<BrowserState, String> {
        let Some(current) = &self.current else {
            return self.roots(&self.environment.clone());
        };
        if current.relative_path() == "." {
            return self.roots(&self.environment.clone());
        }
        let parent = current
            .relative_path()
            .rsplit_once('/')
            .map(|(p, _)| p)
            .unwrap_or(".");
        self.list(SourceReference::new(current.source_uri(), parent).map_err(|e| e.to_string())?)
    }

    fn list(&mut self, reference: SourceReference) -> Result<BrowserState, String> {
        let source = self
            .sources
            .iter()
            .find(|s| s.reader.source_uri() == reference.source_uri())
            .ok_or("unbound browser source")?;
        let listing = source
            .reader
            .list(&reference, MAX_DIRECTORY_ENTRIES)
            .map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        let mut visible = BTreeMap::new();
        for entry in listing
            .entries
            .into_iter()
            .filter(|e| e.kind == EntryKind::Directory && !e.name.starts_with('.'))
        {
            let child = reference
                .descendant(&entry.name)
                .map_err(|e| e.to_string())?;
            visible.insert(child.uri(), child.clone());
            entries.push(BrowserEntry {
                name: entry.name,
                qnc_uri: child.uri(),
                ..BrowserEntry::default()
            });
        }
        entries.sort_by_key(|e| e.name.to_lowercase());
        let path_label = if reference.relative_path() == "." {
            source.entry.name.clone()
        } else {
            format!(
                "{} / {}",
                source.entry.name,
                reference.relative_path().replace('/', " / ")
            )
        };
        let state = BrowserState {
            current_uri: Some(reference.uri()),
            path_label,
            parent_available: true,
            entries,
            ..BrowserState::default()
        };
        self.visible = visible;
        self.current = Some(reference);
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_keeps_registered_identity_and_rejects_unlisted_locations() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("PRIVATE")).unwrap();
        let uri = "qnc://local/source/card-serial";
        let mut browser = TransportBrowserSession::new(vec![BrowserSource {
            entry: BrowserEntry {
                name: "Card".into(),
                qnc_uri: uri.into(),
                serial_number: "serial".into(),
                volume_name: "Camera".into(),
            },
            reader: SourceReader::local(uri, dir.path()).unwrap(),
        }])
        .unwrap();
        assert_eq!(
            browser.roots("local").unwrap().entries[0].serial_number,
            "serial"
        );
        assert!(browser.selected(uri).is_none());
        let root = browser.open(uri).unwrap();
        assert_eq!(browser.selected(uri).unwrap().source_uri(), uri);
        let child = browser.open(&root.entries[0].qnc_uri).unwrap();
        assert_eq!(child.path_label, "Card / PRIVATE");
        assert_eq!(
            browser
                .selected(child.current_uri.as_ref().unwrap())
                .unwrap()
                .source_uri(),
            uri
        );
        assert!(browser.open("qnc://local/source/other").is_err());
        assert_eq!(browser.parent().unwrap().current_uri.as_deref(), Some(uri));
        assert!(browser.roots("lan").unwrap().entries.is_empty());
        assert!(browser.selected(uri).is_none());
    }
}
