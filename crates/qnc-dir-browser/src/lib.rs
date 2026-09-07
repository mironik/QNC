use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

mod transport;
pub use transport::{BrowserSource, TransportBrowserSession};

/// Private owner guard against binding a replacement volume to an old media identity.
/// OSes without an exposed volume serial require an explicitly registered source.
pub fn verify_local_volume_serial(path: &Path, expected: &str) -> Result<(), String> {
    if expected.trim().is_empty() {
        return Ok(());
    }
    let path = path
        .canonicalize()
        .map_err(|_| "source volume unavailable")?;
    let volume = list_roots().into_iter().find(|r| {
        r.local_path
            .canonicalize()
            .is_ok_and(|root| path.starts_with(root))
    });
    if let Some(actual) = volume.and_then(|v| v.serial_number) {
        if !actual.eq_ignore_ascii_case(expected) {
            return Err("source volume serial changed; owner binding must be corrected".into());
        }
    }
    Ok(())
}

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
    pub serial_number: Option<String>,
    pub volume_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryListing {
    pub directory: PathBuf,
    pub parent: Option<PathBuf>,
    pub roots: bool,
    pub entries: Vec<DirectoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowserEntry {
    pub name: String,
    pub qnc_uri: String,
    pub serial_number: String,
    pub volume_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowserCrumb {
    pub label: String,
    pub qnc_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowserState {
    pub roots: bool,
    pub path_label: String,
    pub current_uri: Option<String>,
    pub parent_available: bool,
    pub breadcrumbs: Vec<BrowserCrumb>,
    pub entries: Vec<BrowserEntry>,
}

#[derive(Debug, Default)]
pub struct DirectoryBrowserSession {
    current_directory: Option<PathBuf>,
    current_uri: Option<String>,
    parent_directory: Option<PathBuf>,
    bindings: HashMap<String, PathBuf>,
}

impl DirectoryBrowserSession {
    pub fn load_roots(&mut self) -> Result<BrowserState, String> {
        self.current_directory = None;
        self.current_uri = None;
        self.parent_directory = None;
        let listing = list_directory(&DirectoryListRequest {
            directory: PathBuf::new(),
        })?;
        self.map_listing(listing)
    }

    pub fn open_uri(&mut self, uri: &str) -> Result<BrowserState, String> {
        let path = self
            .bindings
            .get(uri)
            .cloned()
            .ok_or_else(|| format!("unknown QNC location URI: {uri}"))?;
        let listing = list_directory(&DirectoryListRequest { directory: path })?;
        self.map_listing(listing)
    }

    pub fn open_private_path(&mut self, path: impl AsRef<Path>) -> Result<BrowserState, String> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return self.load_roots();
        }
        let listing = list_directory(&DirectoryListRequest {
            directory: path.to_path_buf(),
        })?;
        self.map_listing(listing)
    }

    pub fn open_parent(&mut self) -> Result<BrowserState, String> {
        let Some(parent) = self.parent_directory.clone() else {
            return self.load_roots();
        };
        let listing = list_directory(&DirectoryListRequest { directory: parent })?;
        self.map_listing(listing)
    }

    pub fn path_for_uri(&self, uri: &str) -> Option<PathBuf> {
        self.bindings.get(uri).cloned()
    }

    fn map_listing(&mut self, listing: DirectoryListing) -> Result<BrowserState, String> {
        if listing.roots {
            self.current_directory = None;
            self.current_uri = None;
            self.parent_directory = None;
        } else {
            self.current_directory = Some(listing.directory.clone());
            self.current_uri = Some(self.bind_path(&listing.directory));
            self.parent_directory = listing.parent.clone();
        }

        let entries = listing
            .entries
            .iter()
            .map(|entry| BrowserEntry {
                name: display_entry_name(&entry.name),
                qnc_uri: self.bind_path(&entry.local_path),
                serial_number: entry.serial_number.clone().unwrap_or_default(),
                volume_name: entry.volume_name.clone().unwrap_or_default(),
            })
            .collect::<Vec<_>>();

        let path_label = if listing.roots {
            String::new()
        } else {
            display_path_label(&listing.directory)
        };
        let breadcrumbs = if listing.roots {
            Vec::new()
        } else {
            self.breadcrumbs_for_path(&listing.directory)
        };

        Ok(BrowserState {
            roots: listing.roots,
            path_label,
            current_uri: self.current_uri.clone(),
            parent_available: self.parent_directory.is_some(),
            breadcrumbs,
            entries,
        })
    }

    fn breadcrumbs_for_path(&mut self, path: &Path) -> Vec<BrowserCrumb> {
        browser_crumb_paths(path)
            .into_iter()
            .map(|(label, path)| BrowserCrumb {
                label,
                qnc_uri: self.bind_path(&path),
            })
            .collect()
    }

    fn bind_path(&mut self, path: &Path) -> String {
        let normalized = normalize_path_for_identity(path);
        let uri = format!("qnc://local/source/{:016x}", fnv1a64(&normalized));
        self.bindings
            .entry(uri.clone())
            .or_insert_with(|| path.to_path_buf());
        uri
    }
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
            serial_number: None,
            volume_name: None,
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
                let identity = volume_identity(&path);
                roots.push(DirectoryEntry {
                    name: root,
                    local_path: path,
                    is_dir: true,
                    serial_number: identity.serial_number,
                    volume_name: identity.volume_name,
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
            serial_number: None,
            volume_name: None,
        }]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct VolumeIdentity {
    serial_number: Option<String>,
    volume_name: Option<String>,
}

#[cfg(windows)]
fn volume_identity(root: &Path) -> VolumeIdentity {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    let root_wide = root
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut name = vec![0u16; 260];
    let mut serial = 0u32;
    let ok = unsafe {
        GetVolumeInformationW(
            root_wide.as_ptr(),
            name.as_mut_ptr(),
            name.len() as u32,
            &mut serial,
            null_mut(),
            null_mut(),
            null_mut(),
            0,
        )
    };

    if ok == 0 {
        return VolumeIdentity::default();
    }

    let end = name
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(name.len());
    let volume_name = String::from_utf16_lossy(&name[..end]);
    VolumeIdentity {
        serial_number: Some(format!("{serial:08x}")),
        volume_name: if volume_name.trim().is_empty() {
            None
        } else {
            Some(volume_name)
        },
    }
}

#[cfg(not(windows))]
fn volume_identity(_root: &Path) -> VolumeIdentity {
    VolumeIdentity::default()
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

pub fn display_path_label(path: &Path) -> String {
    trim_path_suffix_for_display(&clean_os_path_display(&path.to_string_lossy()))
}

pub fn display_private_path(path: &Path) -> String {
    clean_os_path_display(&path.to_string_lossy())
}

pub fn display_entry_name(name: &str) -> String {
    trim_path_suffix_for_display(&clean_os_path_display(name))
}

fn clean_os_path_display(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else if let Some(rest) = trimmed.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("//?/") {
        rest.to_string()
    } else {
        trimmed.to_string()
    }
}

fn trim_path_suffix_for_display(value: &str) -> String {
    if value == "/" || value == "\\" {
        return value.to_string();
    }
    if is_windows_drive_root(value) {
        return value[..2].to_string();
    }
    value.trim_end_matches(['\\', '/']).to_string()
}

fn is_windows_drive_root(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn browser_crumb_paths(path: &Path) -> Vec<(String, PathBuf)> {
    let clean = clean_os_path_display(&path.to_string_lossy());
    if clean.is_empty() {
        return Vec::new();
    }

    if is_windows_drive_rooted(&clean) {
        let drive = clean[..2].to_string();
        let mut out = vec![(drive.clone(), PathBuf::from(format!("{drive}\\")))];
        let rest = clean[3..].trim_matches(['\\', '/']);
        let mut current = format!("{drive}\\");
        for part in rest.split(['\\', '/']).filter(|part| !part.is_empty()) {
            if !current.ends_with('\\') {
                current.push('\\');
            }
            current.push_str(part);
            out.push((part.to_string(), PathBuf::from(current.clone())));
        }
        return out;
    }

    if clean.starts_with("\\\\") {
        let mut out = Vec::new();
        let mut current = String::from("\\\\");
        for part in clean
            .trim_start_matches('\\')
            .split('\\')
            .filter(|part| !part.is_empty())
        {
            if current != "\\\\" {
                current.push('\\');
            }
            current.push_str(part);
            out.push((part.to_string(), PathBuf::from(current.clone())));
        }
        return out;
    }

    if clean.starts_with('/') {
        let mut out = vec![("/".to_string(), PathBuf::from("/"))];
        let mut current = String::from("/");
        for part in clean
            .trim_start_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
        {
            if !current.ends_with('/') {
                current.push('/');
            }
            current.push_str(part);
            out.push((part.to_string(), PathBuf::from(current.clone())));
        }
        return out;
    }

    let mut out = Vec::new();
    let mut current = String::new();
    for part in clean.split(['\\', '/']).filter(|part| !part.is_empty()) {
        if !current.is_empty() {
            current.push(std::path::MAIN_SEPARATOR);
        }
        current.push_str(part);
        out.push((part.to_string(), PathBuf::from(current.clone())));
    }
    out
}

fn is_windows_drive_rooted(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn normalize_path_for_identity(path: &Path) -> String {
    clean_os_path_display(&path.to_string_lossy())
        .replace('\\', "/")
        .to_lowercase()
}

fn fnv1a64(value: &str) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
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
    fn browser_session_exposes_qnc_uri_state() {
        let mut session = DirectoryBrowserSession::default();
        let state = session.load_roots().expect("roots");

        assert!(state.roots);
        assert!(state
            .entries
            .iter()
            .all(|entry| entry.qnc_uri.starts_with("qnc://local/source/")));
        assert!(state
            .entries
            .iter()
            .all(|entry| !entry.name.contains("\\\\?\\")));
    }

    #[test]
    fn private_start_path_returns_browser_state_with_current_uri() {
        let root = temp_root("session");
        fs::create_dir_all(root.join("Child")).expect("child dir");

        let mut session = DirectoryBrowserSession::default();
        let state = session.open_private_path(&root).expect("state");

        assert!(!state.roots);
        assert!(state.current_uri.is_some());
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.name == "Child"
                    && entry.qnc_uri.starts_with("qnc://local/source/"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn browser_breadcrumbs_use_qnc_uri_not_raw_path() {
        let root = temp_root("crumbs");
        let nested = root.join("A").join("B");
        fs::create_dir_all(&nested).expect("nested dir");

        let mut session = DirectoryBrowserSession::default();
        let state = session.open_private_path(&nested).expect("state");

        assert!(!state.breadcrumbs.is_empty());
        assert!(state
            .breadcrumbs
            .iter()
            .all(|crumb| crumb.qnc_uri.starts_with("qnc://local/source/")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn display_labels_hide_windows_extended_prefix() {
        assert_eq!(display_entry_name("\\\\?\\G:\\"), "G:");
        assert_eq!(display_path_label(Path::new("\\\\?\\G:\\")), "G:");
        assert_eq!(display_private_path(Path::new("\\\\?\\G:\\")), "G:\\");
        assert_eq!(display_entry_name("/"), "/");
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
