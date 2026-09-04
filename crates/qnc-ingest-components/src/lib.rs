use serde::{Deserialize, Serialize};

pub mod action_ids {
    pub const INGEST_SOURCE_KIND_LOCAL: &str = "ingest_source_kind_local";
    pub const INGEST_SOURCE_KIND_LAN: &str = "ingest_source_kind_lan";
    pub const INGEST_SOURCE_KIND_INTERNET: &str = "ingest_source_kind_internet";
    pub const INGEST_DIR_UP: &str = "ingest_dir_up";
    pub const INGEST_DIR_ROOTS: &str = "ingest_dir_roots";
    pub const INGEST_DIR_OPEN: &str = "ingest_dir_open";
    pub const INGEST_DIR_CONFIRM: &str = "ingest_dir_confirm";
    pub const INGEST_DIR_CANCEL: &str = "ingest_dir_cancel";
    pub const INGEST_PREVIEW_FOCUS: &str = "ingest_preview_focus";
    pub const INGEST_CLIP_TOGGLE: &str = "ingest_clip_toggle";
    pub const INGEST_SELECT_ALL: &str = "ingest_select_all";
    pub const INGEST_CLEAR_SELECTION: &str = "ingest_clear_selection";
    pub const INGEST_IMPORT_SELECTED: &str = "ingest_import_selected";
    pub const INGEST_RELOAD: &str = "ingest_reload";
    pub const INGEST_SET_ARCHIVE: &str = "ingest_set_archive";
    pub const INGEST_SET_AI_MINING: &str = "ingest_set_ai_mining";
    pub const INGEST_APPROVE_PROXY_POSTERS: &str = "ingest_approve_proxy_posters";
    pub const INGEST_CUE_FRAME: &str = "ingest_cue_frame";
    pub const INGEST_TOGGLE_AUDIO_LANE: &str = "ingest_toggle_audio_lane";
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STEP_BACK_FRAME: &str = "step_back_frame";
    pub const STEP_FORWARD_FRAME: &str = "step_forward_frame";

    pub const ALL: &[&str] = &[
        INGEST_SOURCE_KIND_LOCAL,
        INGEST_SOURCE_KIND_LAN,
        INGEST_SOURCE_KIND_INTERNET,
        INGEST_DIR_UP,
        INGEST_DIR_ROOTS,
        INGEST_DIR_OPEN,
        INGEST_DIR_CONFIRM,
        INGEST_DIR_CANCEL,
        INGEST_PREVIEW_FOCUS,
        INGEST_CLIP_TOGGLE,
        INGEST_SELECT_ALL,
        INGEST_CLEAR_SELECTION,
        INGEST_IMPORT_SELECTED,
        INGEST_RELOAD,
        INGEST_SET_ARCHIVE,
        INGEST_SET_AI_MINING,
        INGEST_APPROVE_PROXY_POSTERS,
        INGEST_CUE_FRAME,
        INGEST_TOGGLE_AUDIO_LANE,
        PLAY_PAUSE,
        STEP_BACK_FRAME,
        STEP_FORWARD_FRAME,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Local,
    Lan,
    Internet,
}

impl Default for SourceKind {
    fn default() -> Self {
        Self::Local
    }
}

impl SourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Računalo",
            Self::Lan => "LAN",
            Self::Internet => "Internet",
        }
    }

    pub fn action_id(self) -> &'static str {
        match self {
            Self::Local => action_ids::INGEST_SOURCE_KIND_LOCAL,
            Self::Lan => action_ids::INGEST_SOURCE_KIND_LAN,
            Self::Internet => action_ids::INGEST_SOURCE_KIND_INTERNET,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LocationEntry {
    pub name: String,
    pub qnc_uri: String,
    pub serial_number: String,
    pub volume_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipView {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub selected: bool,
    pub imported: bool,
    pub thumb_uri: Option<String>,
    pub thumb_status: ThumbStatus,
}

impl Default for ClipView {
    fn default() -> Self {
        Self {
            clip_id: String::new(),
            name: String::new(),
            duration_seconds: 0.0,
            selected: false,
            imported: false,
            thumb_uri: None,
            thumb_status: ThumbStatus::Missing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThumbStatus {
    Ready,
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestViewModel {
    pub source_kind: SourceKind,
    pub browser_roots: bool,
    pub browser_path_label: String,
    pub browser_parent_available: bool,
    pub browser_entries: Vec<LocationEntry>,
    pub browser_busy: bool,
    pub browser_error: Option<String>,
    pub clips: Vec<ClipView>,
    pub preview_clip_id: Option<String>,
    pub archive_original: bool,
    pub archive_original_available: bool,
    pub ai_mining: bool,
    pub playing: bool,
    pub cue_frame: i64,
    pub command_busy: bool,
    pub message: String,
}

impl Default for IngestViewModel {
    fn default() -> Self {
        Self {
            source_kind: SourceKind::Local,
            browser_roots: true,
            browser_path_label: String::new(),
            browser_parent_available: false,
            browser_entries: Vec::new(),
            browser_busy: false,
            browser_error: None,
            clips: Vec::new(),
            preview_clip_id: None,
            archive_original: false,
            archive_original_available: false,
            ai_mining: false,
            playing: false,
            cue_frame: 0,
            command_busy: false,
            message: "Odaberi izvor.".to_string(),
        }
    }
}

impl IngestViewModel {
    pub fn total_count(&self) -> usize {
        self.clips.len()
    }

    pub fn selected_count(&self) -> usize {
        self.clips.iter().filter(|clip| clip.selected).count()
    }

    pub fn imported_count(&self) -> usize {
        self.clips.iter().filter(|clip| clip.imported).count()
    }

    pub fn pending_count(&self) -> usize {
        self.total_count().saturating_sub(self.imported_count())
    }

    pub fn current_clip_label(&self) -> &str {
        self.preview_clip_id
            .as_deref()
            .and_then(|clip_id| {
                self.clips
                    .iter()
                    .find(|clip| clip.clip_id == clip_id)
                    .map(|clip| clip.name.as_str())
            })
            .unwrap_or("Odaberi klip")
    }

    pub fn status_label(&self) -> String {
        format!(
            "{} odabrano · {} uvezeno · {} ukupno",
            self.selected_count(),
            self.imported_count(),
            self.total_count()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IngestPayload {
    None,
    SourceKind(SourceKind),
    LocationUri(String),
    ClipId(String),
    Bool(bool),
    Frame(i64),
    AudioLane(String),
}

impl Default for IngestPayload {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestIntent {
    pub action_id: String,
    pub payload: IngestPayload,
}

impl IngestIntent {
    pub fn new(action_id: impl Into<String>, payload: IngestPayload) -> Self {
        Self {
            action_id: action_id.into(),
            payload,
        }
    }

    pub fn empty(action_id: impl Into<String>) -> Self {
        Self::new(action_id, IngestPayload::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestDispatchResult {
    pub accepted: bool,
    pub message: Option<String>,
    pub request_repaint: bool,
}

impl IngestDispatchResult {
    fn accepted(message: Option<String>, request_repaint: bool) -> Self {
        Self {
            accepted: true,
            message,
            request_repaint,
        }
    }

    fn rejected(message: impl Into<String>) -> Self {
        Self {
            accepted: false,
            message: Some(message.into()),
            request_repaint: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct IngestComponent {
    view: IngestViewModel,
    dispatch_log: Vec<String>,
}

impl IngestComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &IngestViewModel {
        &self.view
    }

    pub fn dispatch_log(&self) -> &[String] {
        &self.dispatch_log
    }

    pub fn dispatch(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        self.dispatch_log.push(intent.action_id.clone());

        match intent.action_id.as_str() {
            action_ids::INGEST_SOURCE_KIND_LOCAL => {
                self.view.source_kind = SourceKind::Local;
                self.view.browser_roots = true;
                self.view.message = "Odaberi lokalni izvor.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_SOURCE_KIND_LAN => {
                self.view.source_kind = SourceKind::Lan;
                self.view.browser_roots = true;
                self.view.message = "LAN izvor nije povezan u ovom rezu.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_SOURCE_KIND_INTERNET => {
                self.view.source_kind = SourceKind::Internet;
                self.view.browser_roots = true;
                self.view.message = "Internet izvor nije povezan u ovom rezu.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_DIR_CANCEL => {
                self.view.message = "Odabir izvora je otkazan.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_SELECT_ALL => {
                for clip in &mut self.view.clips {
                    clip.selected = true;
                }
                self.view.message = self.view.status_label();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_CLEAR_SELECTION => {
                for clip in &mut self.view.clips {
                    clip.selected = false;
                }
                self.view.message = self.view.status_label();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_CLIP_TOGGLE => match intent.payload {
                IngestPayload::ClipId(clip_id) => {
                    if let Some(clip) = self
                        .view
                        .clips
                        .iter_mut()
                        .find(|clip| clip.clip_id == clip_id)
                    {
                        clip.selected = !clip.selected;
                        self.view.message = self.view.status_label();
                        IngestDispatchResult::accepted(None, true)
                    } else {
                        IngestDispatchResult::rejected("Clip nije pronađen.")
                    }
                }
                _ => IngestDispatchResult::rejected("Nedostaje clip_id."),
            },
            action_ids::INGEST_PREVIEW_FOCUS => match intent.payload {
                IngestPayload::ClipId(clip_id) => {
                    self.view.preview_clip_id = Some(clip_id);
                    IngestDispatchResult::accepted(None, true)
                }
                _ => IngestDispatchResult::rejected("Nedostaje clip_id."),
            },
            action_ids::INGEST_SET_ARCHIVE => match intent.payload {
                IngestPayload::Bool(value) => {
                    self.view.archive_original = value;
                    IngestDispatchResult::accepted(None, true)
                }
                _ => IngestDispatchResult::rejected("Nedostaje bool vrijednost."),
            },
            action_ids::INGEST_SET_AI_MINING => match intent.payload {
                IngestPayload::Bool(value) => {
                    self.view.ai_mining = value;
                    IngestDispatchResult::accepted(None, true)
                }
                _ => IngestDispatchResult::rejected("Nedostaje bool vrijednost."),
            },
            action_ids::PLAY_PAUSE => {
                self.view.playing = !self.view.playing;
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::STEP_BACK_FRAME => {
                self.view.cue_frame = self.view.cue_frame.saturating_sub(1);
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::STEP_FORWARD_FRAME => {
                self.view.cue_frame = self.view.cue_frame.saturating_add(1);
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_CUE_FRAME => match intent.payload {
                IngestPayload::Frame(frame) => {
                    self.view.cue_frame = frame.max(0);
                    IngestDispatchResult::accepted(None, true)
                }
                _ => IngestDispatchResult::rejected("Nedostaje frame."),
            },
            _ => IngestDispatchResult::accepted(
                Some("Akcija je zapisana, komponenta za izvršenje još nije spojena.".to_string()),
                true,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, fs, path::Path};

    #[test]
    fn action_ids_are_unique() {
        let unique = action_ids::ALL.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), action_ids::ALL.len());
    }

    #[test]
    fn component_starts_with_passive_empty_view() {
        let component = IngestComponent::new();
        assert_eq!(component.view().source_kind, SourceKind::Local);
        assert!(component.view().browser_roots);
        assert!(component.view().clips.is_empty());
        assert_eq!(component.dispatch_log().len(), 0);
    }

    #[test]
    fn ingest_ui_crate_has_no_active_dependencies() {
        let ui_src = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate dir parent")
            .join("qnc-ingest-desktop")
            .join("src");
        let files = rust_files(&ui_src);
        assert!(!files.is_empty(), "expected ingest desktop sources");

        let forbidden = [
            "std::fs",
            "rusqlite",
            "std::process::Command",
            "Command::new",
            "ffprobe",
            "ffmpeg",
            "qnc_source_scanner",
            "qnc_media_probe",
            "qnc_filmstrip::",
            "qnc_wave::",
            "qnc_broadcast_player",
            "qnc_project_store",
            "pick_directory",
            "list_directory",
        ];

        for path in files {
            let text = fs::read_to_string(&path).expect("read ingest ui source");
            let scrubbed = strip_line_comments(&text);
            for needle in forbidden {
                assert!(
                    !scrubbed.contains(needle),
                    "{} must not contain active dependency marker {needle}",
                    path.display()
                );
            }
        }
    }

    fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).expect("read source dir") {
            let entry = entry.expect("source entry");
            let path = entry.path();
            if path.is_dir() {
                out.extend(rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
        out
    }

    fn strip_line_comments(text: &str) -> String {
        text.lines()
            .map(|line| line.split_once("//").map(|(code, _)| code).unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
