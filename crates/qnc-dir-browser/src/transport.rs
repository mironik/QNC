use crate::{BrowserEntry, BrowserState};
use qnc_source_reader::{EntryKind, SourceReader, SourceReference, MAX_DIRECTORY_ENTRIES};
use std::{collections::BTreeMap, fmt, sync::Arc};

#[derive(Clone)]
pub struct BrowserSource {
    pub entry: BrowserEntry,
    private_local_root: Option<std::path::PathBuf>,
    connect: Arc<dyn Fn() -> Result<SourceReader, String> + Send + Sync>,
}

impl fmt::Debug for BrowserSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserSource")
            .field("entry", &self.entry)
            .finish_non_exhaustive()
    }
}

impl BrowserSource {
    /// The owner opens and validates only the requested source, on each navigation.
    /// No volume handles or network connections are retained by the browser.
    pub fn new(
        entry: BrowserEntry,
        connect: impl Fn() -> Result<SourceReader, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            entry,
            private_local_root: None,
            connect: Arc::new(connect),
        }
    }

    pub fn with_private_local_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.private_local_root = Some(root.into());
        self
    }
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
            SourceReference::new(&source.entry.qnc_uri, ".").map_err(|e| e.to_string())?;
            if !ids.insert(&source.entry.qnc_uri) {
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
                    SourceReference::new(uri, ".").map_err(|e| e.to_string())?,
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

    pub fn selected_private_local_path(&self, uri: &str) -> Option<std::path::PathBuf> {
        let current = self.current.as_ref().filter(|r| r.uri() == uri)?;
        let source = self
            .sources
            .iter()
            .find(|source| source.entry.qnc_uri == current.source_uri())?;
        let root = source.private_local_root.as_ref()?;
        if current.relative_path() == "." {
            Some(root.clone())
        } else {
            Some(root.join(current.relative_path()))
        }
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
            .find(|s| s.entry.qnc_uri == reference.source_uri())
            .ok_or("unbound browser source")?;
        let reader = (source.connect)().map_err(|e| format!("{}: {e}", reference.source_uri()))?;
        if reader.source_uri() != reference.source_uri() {
            return Err("browser source binding mismatch".into());
        }
        let listing = reader
            .list(&reference, MAX_DIRECTORY_ENTRIES)
            .map_err(|e| format!("{}: {e}", reference.uri()))?;
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
    fn disconnected_source_is_isolated_and_reconnected_on_next_navigation() {
        for environment in ["local", "lan/test", "intranet/test"] {
            let fixture = tempfile::tempdir().unwrap();
            let path = fixture.path().join("card");
            let uri = format!("qnc://{environment}/source/card");
            let other_uri = format!("qnc://{environment}/source/offline");
            let opened = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let count = opened.clone();
            let root = path.clone();
            let source_uri = uri.clone();
            let mut browser = TransportBrowserSession::new(vec![
                BrowserSource::new(
                    BrowserEntry {
                        name: "Card".into(),
                        qnc_uri: uri.clone(),
                        ..Default::default()
                    },
                    move || {
                        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        SourceReader::local(&source_uri, &root).map_err(|e| e.to_string())
                    },
                ),
                BrowserSource::new(
                    BrowserEntry {
                        name: "Offline".into(),
                        qnc_uri: other_uri.clone(),
                        ..Default::default()
                    },
                    || Err("offline".into()),
                ),
            ])
            .unwrap();
            let environment = environment.split('/').next().unwrap();
            assert_eq!(browser.roots(environment).unwrap().entries.len(), 2);
            assert_eq!(opened.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert!(browser.open(&other_uri).unwrap_err().contains(&other_uri));
            assert!(browser.open(&uri).is_err());
            std::fs::create_dir(&path).unwrap();
            assert!(browser.open(&uri).unwrap().entries.is_empty());
            // A disconnected root does not remain pinned in a cached reader.
            std::fs::remove_dir(&path).unwrap();
            assert!(browser.parent().unwrap().roots);
            assert!(browser.open(&uri).is_err());
            std::fs::create_dir(&path).unwrap();
            std::fs::create_dir(path.join("PRIVATE")).unwrap();
            assert_eq!(browser.open(&uri).unwrap().entries[0].name, "PRIVATE");
            assert_eq!(opened.load(std::sync::atomic::Ordering::SeqCst), 4);
        }
    }

    #[test]
    fn connector_cannot_replace_registered_source_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_path_buf();
        let uri = "qnc://local/source/expected";
        let mut browser = TransportBrowserSession::new(vec![BrowserSource::new(
            BrowserEntry {
                qnc_uri: uri.into(),
                ..Default::default()
            },
            move || {
                SourceReader::local("qnc://local/source/wrong", &path).map_err(|e| e.to_string())
            },
        )])
        .unwrap();
        browser.roots("local").unwrap();
        assert_eq!(
            browser.open(uri).unwrap_err(),
            "browser source binding mismatch"
        );
        assert!(browser.selected(uri).is_none());
    }

    #[test]
    fn navigation_keeps_registered_identity_and_rejects_unlisted_locations() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("PRIVATE")).unwrap();
        let uri = "qnc://local/source/card-serial";
        let path = dir.path().to_path_buf();
        let mut browser = TransportBrowserSession::new(vec![BrowserSource::new(
            BrowserEntry {
                name: "Card".into(),
                qnc_uri: uri.into(),
                serial_number: "serial".into(),
                volume_name: "Camera".into(),
            },
            move || SourceReader::local(uri, &path).map_err(|e| e.to_string()),
        )])
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

    #[test]
    fn private_local_path_is_owner_side_only_for_selected_local_binding() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("card");
        let child = root.join("Sub");
        std::fs::create_dir_all(&child).unwrap();
        let uri = "qnc://local/source/card";
        let source_uri = uri.to_string();
        let source_root = root.clone();
        let mut browser = TransportBrowserSession::new(vec![BrowserSource::new(
            BrowserEntry {
                name: "Card".into(),
                qnc_uri: uri.into(),
                ..Default::default()
            },
            move || SourceReader::local(&source_uri, &source_root).map_err(|e| e.to_string()),
        )
        .with_private_local_root(root.clone())])
        .unwrap();

        browser.roots("local").unwrap();
        let opened = browser.open(uri).unwrap();
        assert!(opened
            .entries
            .iter()
            .all(|entry| !entry.qnc_uri.contains('\\')));
        assert_eq!(browser.selected_private_local_path(uri), Some(root.clone()));
        let child_uri = opened.entries[0].qnc_uri.clone();
        browser.open(&child_uri).unwrap();
        assert_eq!(browser.selected_private_local_path(&child_uri), Some(child));
    }
}
