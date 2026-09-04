use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryPickRequest {
    pub start_dir: Option<PathBuf>,
    pub fallback_dir: PathBuf,
}

impl DirectoryPickRequest {
    pub fn new(fallback_dir: impl Into<PathBuf>) -> Self {
        Self {
            start_dir: None,
            fallback_dir: fallback_dir.into(),
        }
    }

    pub fn with_start_dir(mut self, start_dir: impl Into<PathBuf>) -> Self {
        self.start_dir = Some(start_dir.into());
        self
    }

    fn initial_dir(&self) -> &Path {
        self.start_dir
            .as_deref()
            .filter(|path| path.is_dir())
            .unwrap_or(&self.fallback_dir)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectorySelection {
    pub local_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryListRequest {
    pub directory: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub local_path: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryListing {
    pub directory: PathBuf,
    pub parent: Option<PathBuf>,
    pub roots: bool,
    pub entries: Vec<DirectoryEntry>,
}

pub fn pick_directory(request: &DirectoryPickRequest) -> Option<DirectorySelection> {
    rfd::FileDialog::new()
        .set_directory(request.initial_dir())
        .pick_folder()
        .map(|local_path| DirectorySelection { local_path })
}

pub fn list_directory(request: &DirectoryListRequest) -> Result<DirectoryListing, String> {
    if request.directory.as_os_str().is_empty() {
        return Ok(DirectoryListing {
            directory: PathBuf::new(),
            parent: None,
            roots: true,
            entries: list_roots(),
        });
    }

    let path = normalize_list_path(&request.directory)?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("path not accessible: {error}"))?;
    if !canonical.is_dir() {
        return Err("not a directory".to_string());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&canonical).map_err(|error| format!("read_dir failed: {error}"))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }
        let path = entry.path();
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if !metadata.is_dir() || is_hidden_or_system(&name, &metadata) {
            continue;
        }
        entries.push(DirectoryEntry {
            name,
            local_path: path,
            is_dir: true,
        });
    }
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(DirectoryListing {
        parent: parent_path(&canonical),
        directory: canonical,
        roots: false,
        entries,
    })
}

pub fn list_roots() -> Vec<DirectoryEntry> {
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        for letter in b'A'..=b'Z' {
            let root = format!("{}:\\", letter as char);
            let path = PathBuf::from(&root);
            if path.is_dir() {
                roots.push(DirectoryEntry {
                    name: root,
                    local_path: path,
                    is_dir: true,
                });
            }
        }
        roots
    }
    #[cfg(not(windows))]
    {
        vec![DirectoryEntry {
            name: "/".to_string(),
            local_path: PathBuf::from("/"),
            is_dir: true,
        }]
    }
}

fn normalize_list_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("empty path".to_string());
    }
    if path.to_string_lossy().contains('\0') {
        return Err("invalid path".to_string());
    }
    if !path.is_absolute() {
        return Err("path must be absolute".to_string());
    }
    Ok(path.to_path_buf())
}

fn parent_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    #[cfg(windows)]
    {
        let display = parent.to_string_lossy();
        if display.len() == 2 && display.as_bytes()[1] == b':' {
            return Some(PathBuf::from(format!("{display}\\")));
        }
    }
    Some(parent.to_path_buf())
}

fn is_hidden_or_system(name: &str, metadata: &fs::Metadata) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        let attrs = metadata.file_attributes();
        attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn request_uses_existing_start_dir() {
        let root = temp_root("start");
        let start = root.join("selected");
        fs::create_dir_all(&start).expect("start dir");
        let request = DirectoryPickRequest::new(root.clone()).with_start_dir(start.clone());

        assert_eq!(request.initial_dir(), start.as_path());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn request_falls_back_when_start_dir_is_missing() {
        let root = temp_root("fallback");
        fs::create_dir_all(&root).expect("root dir");
        let request = DirectoryPickRequest::new(root.clone()).with_start_dir(root.join("missing"));

        assert_eq!(request.initial_dir(), root.as_path());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn empty_path_returns_roots() {
        let listing = list_directory(&DirectoryListRequest {
            directory: PathBuf::new(),
        })
        .expect("roots");

        assert!(listing.roots);
        assert!(listing.parent.is_none());
        assert!(!listing.entries.is_empty());
    }

    #[test]
    fn lists_only_visible_directories_without_media_scan() {
        let root = temp_root("list");
        fs::create_dir_all(root.join("B_dir")).expect("b dir");
        fs::create_dir_all(root.join("a_dir")).expect("a dir");
        fs::create_dir_all(root.join(".hidden")).expect("hidden dir");
        fs::write(root.join("clip.mp4"), "").expect("file");
        let listing = list_directory(&DirectoryListRequest {
            directory: root.clone(),
        })
        .expect("listing");

        assert_eq!(
            listing
                .entries
                .iter()
                .map(|entry| (entry.name.as_str(), entry.is_dir))
                .collect::<Vec<_>>(),
            [("a_dir", true), ("B_dir", true)]
        );
        assert!(!listing.roots);
        assert_eq!(
            listing.directory,
            root.canonicalize().expect("canonical root")
        );
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "qnc_dir_browser_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }
}
