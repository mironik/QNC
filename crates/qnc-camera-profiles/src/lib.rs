//! Camera profiles (user 2026-10-23: "a block that reads the camera's records and compares them
//! with text files in which the procedure for that camera is written; the initial data are text,
//! the application reads them and writes them permanently into the database").
//!
//! One JSON file per camera model under `catalogs/cameras` is the source. [`sync`] reads them,
//! keeps every valid one in the camera database of this computer (`data/qnc-cameras.db`, of which
//! this block is the only writer) and gives the profiles the database now holds. A file with an
//! error is not taken; the last valid record of its profile stays. [`recognise`] finds the camera
//! of a card's records. This block reads no media and no card: the records are given as text.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MODULE_ID: &str = "qnc.module.camera-profiles";
pub const FORMAT_VERSION: u32 = 1;
/// Where the text profiles are, under the QNC root.
pub const PROFILE_DIR: &str = "catalogs/cameras";
/// The camera database of this computer, under the QNC root.
pub const DATABASE: &str = "data/qnc-cameras.db";
/// The tag of a clip record that names the profile it was read with (`<id>@<version>`).
pub const RECORD_TAG: &str = "qnc.camera:profile";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub format_version: u32,
    pub id: String,
    pub version: u32,
    pub manufacturer: String,
    pub model: String,
    /// The profile of a card of this record format whose camera no profile recognises.
    #[serde(default)]
    pub default_for_reader: bool,
    /// The record format reader (a block that knows the format, not the camera).
    pub reader: String,
    /// The card layouts of the camera catalog the camera writes.
    pub layouts: Vec<String>,
    pub recognise: Vec<Rule>,
    pub procedure: Procedure,
    #[serde(default)]
    pub notes: String,
}

/// An element attribute in the card's records that names the camera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub element: String,
    pub attribute: String,
    #[serde(default)]
    pub equals: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Procedure {
    pub metadata: MetadataRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataRule {
    /// The card record is the final record when it states what a probe would; else one probe.
    DeclaredWhenComplete,
    /// Always one probe in Ingest.
    ProbeOnce,
}

impl Profile {
    /// `<id>@<version>`, as written into a clip record.
    pub fn stamp(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != FORMAT_VERSION {
            return Err(format!("format_version {} is not {FORMAT_VERSION}", self.format_version));
        }
        let id_ok = !self.id.is_empty() && self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err(format!("id '{}' must be lowercase letters, digits and '-'", self.id));
        }
        if self.version == 0 || self.manufacturer.trim().is_empty() || self.reader.trim().is_empty() {
            return Err("version, manufacturer and reader are required".into());
        }
        if self.default_for_reader != self.recognise.is_empty() {
            return Err("a default profile recognises nothing; any other needs at least one rule".into());
        }
        if !self.default_for_reader && self.model.trim().is_empty() {
            return Err("model is required".into());
        }
        for rule in &self.recognise {
            if rule.element.trim().is_empty() || rule.attribute.trim().is_empty() || rule.equals.is_some() == rule.prefix.is_some() {
                return Err(format!("rule on {}/@{} needs an element, an attribute and exactly one of equals or prefix", rule.element, rule.attribute));
            }
        }
        Ok(())
    }

    /// Whether a rule matches one of `records` (XML texts of the card).
    fn matches(&self, records: &[&str]) -> bool {
        self.recognise.iter().any(|rule| {
            records.iter().any(|text| {
                attribute_values(text, &rule.element, &rule.attribute).iter().any(|value| match (&rule.equals, &rule.prefix) {
                    (Some(equals), _) => value == equals,
                    (_, Some(prefix)) => value.starts_with(prefix.as_str()),
                    _ => false,
                })
            })
        })
    }
}

/// The camera of a card's `records` for the format `reader`: the recognised profile, else the
/// format's default (`recognised` false).
pub fn recognise<'a>(profiles: &'a [Profile], reader: &str, records: &[&str]) -> Option<(&'a Profile, bool)> {
    let of_reader = || profiles.iter().filter(move |p| p.reader == reader);
    of_reader()
        .find(|p| !p.default_for_reader && p.matches(records))
        .map(|p| (p, true))
        .or_else(|| of_reader().find(|p| p.default_for_reader).map(|p| (p, false)))
}

/// The values of `attribute` on every `element` of an XML text (local names; a text that is
/// not well formed gives what was read before the error).
fn attribute_values(text: &str, element: &str, attribute: &str) -> Vec<String> {
    use xml::reader::{EventReader, XmlEvent};
    let mut values = Vec::new();
    for event in EventReader::from_str(text) {
        match event {
            Ok(XmlEvent::StartElement { name, attributes, .. }) if name.local_name == element => {
                values.extend(attributes.into_iter().filter(|a| a.name.local_name == attribute).map(|a| a.value));
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    values
}

/// What [`sync`] found.
#[derive(Debug, Default)]
pub struct Synced {
    /// The profiles the database now holds.
    pub profiles: Vec<Profile>,
    /// Profiles written now (new or changed files).
    pub written: Vec<String>,
    /// Files that were not taken, with why.
    pub refused: Vec<String>,
}

/// Reads the text profiles of `root` into its camera database and gives what the database holds.
pub fn sync(root: &Path) -> Result<Synced, String> {
    let (files, mut refused) = read_dir(&root.join(PROFILE_DIR));
    let database = root.join(DATABASE);
    if let Some(parent) = database.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut conn = Connection::open(&database).map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS camera_profiles (
            id TEXT PRIMARY KEY, version INTEGER NOT NULL, file TEXT NOT NULL,
            text_hash TEXT NOT NULL, profile_json TEXT NOT NULL, written_at_unix_ms INTEGER NOT NULL)",
    )
    .map_err(|e| e.to_string())?;
    let mut ids = std::collections::BTreeSet::new();
    let mut written = Vec::new();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for (file, text, profile) in files {
        if !ids.insert(profile.id.clone()) {
            refused.push(format!("{}: the id '{}' is already taken by another file", file.display(), profile.id));
            continue;
        }
        let hash = hash_of(&text);
        let same: bool = tx
            .query_row("SELECT text_hash = ?2 FROM camera_profiles WHERE id = ?1", params![profile.id, hash], |row| row.get(0))
            .unwrap_or(false);
        if same {
            continue;
        }
        let json = serde_json::to_string(&profile).map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT OR REPLACE INTO camera_profiles (id, version, file, text_hash, profile_json, written_at_unix_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![profile.id, profile.version, file.to_string_lossy(), hash, json, now_ms()],
        )
        .map_err(|e| e.to_string())?;
        written.push(profile.stamp());
    }
    tx.commit().map_err(|e| e.to_string())?;
    let profiles = load(&conn)?;
    Ok(Synced { profiles, written, refused })
}

/// The valid profiles of a folder of text profiles (read only, nothing is written), and the
/// files refused with why.
pub fn read_profiles(dir: &Path) -> (Vec<Profile>, Vec<String>) {
    let (found, refused) = read_dir(dir);
    (found.into_iter().map(|(_, _, profile)| profile).collect(), refused)
}

/// The profiles the camera database of `root` holds (read only).
pub fn stored(root: &Path) -> Result<Vec<Profile>, String> {
    let conn = Connection::open_with_flags(root.join(DATABASE), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
    load(&conn)
}

fn load(conn: &Connection) -> Result<Vec<Profile>, String> {
    let mut stmt = conn.prepare("SELECT profile_json FROM camera_profiles ORDER BY id").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
    rows.map(|json| serde_json::from_str(&json.map_err(|e| e.to_string())?).map_err(|e: serde_json::Error| e.to_string())).collect()
}

/// The valid profiles of `dir` (recursive `*.json`) with their texts, and the files refused.
fn read_dir(dir: &Path) -> (Vec<(PathBuf, String, Profile)>, Vec<String>) {
    let (mut found, mut refused) = (Vec::new(), Vec::new());
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
                let read = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| {
                    let profile: Profile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                    profile.validate()?;
                    Ok((text, profile))
                });
                match read {
                    Ok((text, profile)) => found.push((path, text, profile)),
                    Err(error) => refused.push(format!("{}: {error}", path.display())),
                }
            }
        }
    }
    (found, refused)
}

/// A stable fingerprint of a text (FNV-1a 64).
fn hash_of(text: &str) -> String {
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests;
