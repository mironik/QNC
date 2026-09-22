//! Editorial application composition root, shared by the Media Assist and Story
//! groups. It follows the chain of AGENTS.md section 4.1: the active project and
//! its work settings come from the DB (never from a default), the clip list from
//! the public views of the project content, the preview from the universal
//! `qnc-source-preview` component. The one write is a virtual short, through
//! `qnc-virtual-shots`. No scan and no probe. The form only paints
//! `EditorialView` and sends `EditorialIntent`.

mod view;

use std::{
    path::Path,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

use qnc_clip_posters::ClipPosters;
use qnc_content_read::{CatalogSignature, ClipSummary, ContentReader};
use qnc_source_bindings::TransportBindings;
use qnc_source_preview::{PreviewContext, SourcePreview};
use qnc_timeline::TimelineIntent;
use qnc_virtual_short_cards::ParentClip;
use qnc_virtual_short_stills::VirtualShortStillCache;
use qnc_work_settings::{SettingsReader, WorkSettings};

pub use view::{
    action_ids, EditorialClip, EditorialIntent, EditorialShort, EditorialView, LibraryTab,
    MonitorFrame, PreviewView,
};

/// Finds the QNC root (the directory with `AGENTS.md` and the editorial layout
/// contract) starting from the executable and the working directory.
pub fn locate_qnc_root() -> Option<std::path::PathBuf> {
    let mut starts = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        starts.push(exe);
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    for start in starts {
        let mut current = if start.is_file() {
            start.parent().map(std::path::PathBuf::from)
        } else {
            Some(start)
        };
        while let Some(dir) = current {
            if dir.join("AGENTS.md").is_file()
                && dir
                    .join("contracts")
                    .join("ui")
                    .join("editorial.layout.json")
                    .is_file()
            {
                return Some(dir);
            }
            current = dir.parent().map(std::path::PathBuf::from);
        }
    }
    None
}

struct Loaded {
    settings: WorkSettings,
    content: ContentReader,
    signature: CatalogSignature,
    /// `None` when the signature matched what is already shown.
    clips: Option<Vec<ClipSummary>>,
}

type LoadResult = Result<Loaded, String>;

pub struct EditorialApplication {
    view: EditorialView,
    preview: SourcePreview,
    posters: ClipPosters,
    settings_reader: Option<SettingsReader>,
    bindings: Result<TransportBindings, String>,
    load_result: Option<Receiver<LoadResult>>,
    shown_project: Option<String>,
    shown_signature: Option<CatalogSignature>,
    workspace_file: Option<std::path::PathBuf>,
    current_settings: Option<WorkSettings>,
    project_dir: Option<std::path::PathBuf>,
    /// Shot range to paint once the parent clip's timeline is ready.
    pending_shot: Option<PendingShot>,
    short_stills: VirtualShortStillCache,
}

impl Default for EditorialApplication {
    fn default() -> Self {
        Self {
            view: EditorialView::default(),
            preview: SourcePreview::new(),
            posters: ClipPosters::new(),
            settings_reader: None,
            bindings: Err("Izvori medija nisu ucitani.".into()),
            load_result: None,
            shown_project: None,
            shown_signature: None,
            workspace_file: None,
            current_settings: None,
            project_dir: None,
            pending_shot: None,
            short_stills: VirtualShortStillCache::default(),
        }
    }
}

#[derive(Clone)]
struct PendingShot {
    clip_id: String,
    in_frame: u64,
    out_frame: u64,
}

impl EditorialApplication {
    /// Starts from the QNC root. Missing settings end in a controlled error,
    /// never in an invented project.
    pub fn new(root: impl AsRef<Path>) -> Self {
        let mut app = Self::default();
        app.bindings = qnc_source_bindings::load(root.as_ref());
        match SettingsReader::from_root(root.as_ref()) {
            Ok(reader) => {
                app.settings_reader = Some(reader);
                app.load_catalog();
            }
            Err(error) => app.fail(error.to_string()),
        }
        app
    }

    pub fn view(&self) -> &EditorialView {
        &self.view
    }

    /// Text for the shell footer: the last preview error, else the catalog state.
    pub fn footer_status(&self) -> &str {
        if self.view.preview.message.is_empty() {
            &self.view.message
        } else {
            &self.view.preview.message
        }
    }

    pub fn has_player(&self) -> bool {
        self.preview.has_player()
    }

    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.preview.notify_on_change(notify);
    }

    /// Rereads the active project. Cheap when nothing changed: the clips are
    /// loaded again only when the lightweight catalog signature differs.
    pub fn refresh(&mut self) {
        self.load_catalog();
    }

    /// Delay until the next repaint that this surface needs, if any.
    pub fn next_repaint_delay(&self) -> Option<Duration> {
        if let Some(delay) = self.preview.next_repaint_delay() {
            return Some(delay);
        }
        (self.load_result.is_some() || self.posters.has_pending_work())
            .then(|| Duration::from_millis(100))
    }

    fn fail(&mut self, error: String) {
        self.view.loading = false;
        self.view.message = error;
    }

    fn load_catalog(&mut self) {
        if self.load_result.is_some() {
            return;
        }
        let Some(reader) = self.settings_reader.clone() else {
            self.fail("Nema konfiguriranog citaca radnih postavki.".into());
            return;
        };
        let shown_project = self.shown_project.clone();
        let shown_signature = self.shown_signature.clone();
        self.view.loading = shown_project.is_none();
        let (send, receive) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("editorial-catalog".into())
            .spawn(move || {
                let _ = send.send(read_catalog(
                    &reader,
                    shown_project.as_deref(),
                    shown_signature.as_ref(),
                ));
            }) {
            Ok(_) => self.load_result = Some(receive),
            Err(_) => self.fail("Nije moguce pokrenuti citanje projektnog kataloga.".into()),
        }
    }

    /// Applies finished background work and the player state. Returns whether
    /// the view changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.poll_catalog();
        if self.preview.poll() {
            changed = true;
        }
        for poster in self.posters.poll() {
            if let Some(clip) = self
                .view
                .clips
                .iter_mut()
                .find(|clip| clip.clip_id == poster.clip_id)
            {
                clip.thumb_image = Some(poster.image);
                changed = true;
            } else if qnc_virtual_short_cards::apply_poster(
                &mut self.view.shorts,
                &poster.clip_id,
                poster.image,
            ) {
                changed = true;
            }
        }
        self.sync_preview_view();
        changed
    }

    fn sync_preview_view(&mut self) {
        let previous_clip_id = self.view.preview.clip_id.clone();
        let previous_timeline = self.view.preview.timeline;
        self.view.preview = self.preview.view().clone();
        if previous_clip_id == self.view.preview.clip_id {
            self.view.preview.timeline = self
                .view
                .preview
                .timeline
                .preserving_source_marks_from(&previous_timeline);
        } else {
            self.short_stills
                .clear_if_clip_changed(self.view.preview.clip_id.as_deref());
        }
        self.apply_pending_shot();
    }

    fn apply_pending_shot(&mut self) {
        let Some(pending) = self.pending_shot.clone() else {
            return;
        };
        if self.view.preview.clip_id.as_deref() != Some(pending.clip_id.as_str()) {
            return;
        }
        let duration = self.view.preview.timeline.duration_frames;
        if duration < pending.out_frame || self.view.preview.timeline.playhead_frame.is_none() {
            return;
        }
        self.view.preview.timeline.source_in_frame = Some(pending.in_frame.min(duration));
        self.view.preview.timeline.source_out_frame = Some(
            pending
                .out_frame
                .min(duration)
                .max(pending.in_frame.saturating_add(1)),
        );
        self.pending_shot = None;
    }

    fn poll_catalog(&mut self) -> bool {
        let Some(receiver) = self.load_result.as_ref() else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                Err("Citanje projektnog kataloga je prekinuto.".into())
            }
        };
        self.load_result = None;
        self.view.loading = false;
        match result {
            Ok(loaded) => self.apply_loaded(loaded),
            Err(error) => self.fail(error),
        }
        true
    }

    fn apply_loaded(&mut self, loaded: Loaded) {
        self.workspace_file = Some(loaded.content.database_file().to_path_buf());
        let project_id = loaded.settings.project_id.clone();
        let settings = loaded.settings.clone();
        // A different project must never inherit the preceding one's clips or preview.
        if self.shown_project.as_deref() != Some(project_id.as_str()) {
            self.view.clips.clear();
            self.preview.close();
            self.posters.reset();
            self.short_stills.clear();
        }
        self.project_dir = self
            .settings_reader
            .as_ref()
            .and_then(|reader| reader.local_workspace_dir(&loaded.settings).ok().flatten());
        let project_folder =
            self.project_dir
                .clone()
                .map(|dir| qnc_media_thumbnail::ProjectFolder {
                    root_uri: loaded.settings.output_root_uri.clone(),
                    dir,
                });
        if let Ok(bindings) = &self.bindings {
            self.posters.configure(&bindings.sources, project_folder);
        }
        if let Some(clips) = loaded.clips {
            self.view.clips = merge_clips(clips, &self.view.clips);
            self.request_posters();
        }
        match (&self.settings_reader, &self.bindings) {
            (Some(reader), Ok(bindings)) => {
                self.preview.configure(PreviewContext::new(
                    reader.clone(),
                    loaded.settings,
                    loaded.content,
                    bindings.clone(),
                ));
            }
            (_, Err(error)) => self.view.preview.message = error.clone(),
            _ => {}
        }
        self.shown_project = Some(project_id);
        self.shown_signature = Some(loaded.signature);
        self.current_settings = Some(settings);
        self.reload_shorts();
        self.view.message = match self.view.clips.len() {
            0 => "Projekt nema uvezenih klipova.".to_string(),
            count => format!("{count} klipova"),
        };
    }

    /// Asks for the posters the list still lacks; the chosen clip goes first (v5 order).
    fn request_posters(&mut self) {
        self.posters.reset();
        let mut wanted: Vec<(String, String)> = self
            .view
            .clips
            .iter()
            .filter(|clip| clip.thumb_image.is_none())
            .filter_map(|clip| Some((clip.clip_id.clone(), clip.thumb_uri.clone()?)))
            .collect();
        wanted.extend(qnc_virtual_short_cards::poster_requests(&self.view.shorts));
        self.posters.request(wanted, self.view.chosen_clip_id());
    }

    /// Handles one intent from the form. Returns whether the view changed.
    pub fn dispatch(&mut self, intent: EditorialIntent) -> bool {
        let changed = match intent {
            EditorialIntent::PreviewClip(clip_id) => {
                self.view.chosen_shot_id = None;
                self.pending_shot = None;
                self.short_stills.clear_if_clip_changed(Some(&clip_id));
                if self.view.clips.iter().any(|clip| clip.clip_id == clip_id) {
                    self.posters.prioritize(&clip_id);
                    self.preview.open(&clip_id)
                } else {
                    self.view.message = "Klip nije pronadjen.".into();
                    true
                }
            }
            EditorialIntent::PreviewShort(shot_id) => self.open_short(&shot_id),
            EditorialIntent::SwitchLibraryTab(tab) => {
                self.view.library_tab = tab;
                true
            }
            EditorialIntent::Action(action_id) => match action_id.as_str() {
                action_ids::PLAY_PAUSE => self.preview.toggle_play(),
                action_ids::STEP_BACK_FRAME => self.preview.step(-1),
                action_ids::STEP_FORWARD_FRAME => self.preview.step(1),
                action_ids::MARK_IN => {
                    self.view.preview.timeline =
                        self.view.preview.timeline.with_source_in_at_confirmed();
                    if let Err(error) = self.short_stills.capture_in(&self.view.preview) {
                        self.view.preview.message = error;
                    }
                    true
                }
                action_ids::MARK_OUT => {
                    self.view.preview.timeline =
                        self.view.preview.timeline.with_source_out_at_confirmed();
                    if let Err(error) = self.short_stills.capture_out(&self.view.preview) {
                        self.view.preview.message = error;
                    }
                    true
                }
                action_ids::SAVE_VIRTUAL_SHOT => self.save_virtual_shot(),
                _ => false,
            },
            EditorialIntent::Timeline(intent) => match intent {
                TimelineIntent::CueFrame(_) => self.preview.timeline_intent(&intent),
                _ => false,
            },
        };
        self.sync_preview_view();
        changed
    }

    /// Writes one short from the IN/OUT the source timeline is showing.
    fn save_virtual_shot(&mut self) -> bool {
        let Some(clip_id) = self.view.chosen_clip_id().map(str::to_string) else {
            self.view.message = "Odaberi klip.".into();
            return true;
        };
        let Some(clip) = self.view.clips.iter().find(|clip| clip.clip_id == clip_id) else {
            self.view.message = "Klip nije pronadjen.".into();
            return true;
        };
        if !clip.imported {
            self.view.message = format!("Klip '{clip_id}' nije uvezen.");
            return true;
        }
        let Some((in_frame, out_frame)) = self.view.preview.timeline.visible_source_marks() else {
            self.view.message = "IN i OUT nisu potvrdeni na playeru.".into();
            return true;
        };
        let Some(file) = self.workspace_file.clone() else {
            self.view.message = "Projektna baza nije dostupna za upis.".into();
            return true;
        };
        let Some(project_id) = self.shown_project.clone() else {
            self.view.message = "Nema aktivnog projekta.".into();
            return true;
        };
        let name = clip.name.clone();
        match qnc_virtual_shots::save_short(
            &file,
            &project_id,
            &clip_id,
            &name,
            in_frame,
            out_frame,
        ) {
            Ok(shot) => {
                self.store_short_stills(&shot.shot_id, &clip_id, in_frame, out_frame);
                self.reload_shorts();
                self.view.library_tab = LibraryTab::Virtual;
                self.view.chosen_shot_id = Some(shot.shot_id.clone());
                self.view.message = format!("Virtualni kadar {} je spremljen.", shot.shot_id);
            }
            Err(error) => self.view.message = error,
        }
        true
    }

    fn store_short_stills(&mut self, shot_id: &str, clip_id: &str, in_frame: u64, out_frame: u64) {
        let Some(file) = self.workspace_file.as_deref() else {
            return;
        };
        let Some(settings) = self.current_settings.as_ref() else {
            return;
        };
        let Some(project_dir) = self.project_dir.as_deref() else {
            let _ = qnc_virtual_shots::mark_stills_failed(
                file,
                shot_id,
                "Lokalni direktorij projekta nije dostupan.",
            );
            return;
        };
        match self.short_stills.store_for_short(
            project_dir,
            &settings.output_root_uri,
            shot_id,
            clip_id,
            in_frame,
            out_frame,
        ) {
            Ok(stills) => {
                let _ = qnc_virtual_shots::mark_stills_ready(
                    file,
                    shot_id,
                    &stills.in_uri,
                    &stills.out_uri,
                );
            }
            Err(error) => {
                let _ = qnc_virtual_shots::mark_stills_failed(file, shot_id, &error);
            }
        }
    }

    fn open_short(&mut self, shot_id: &str) -> bool {
        let Some(shot) = self
            .view
            .shorts
            .iter()
            .find(|shot| shot.shot_id == shot_id)
            .cloned()
        else {
            self.view.message = "Virtualni kadar nije pronadjen.".into();
            return true;
        };
        if !self
            .view
            .clips
            .iter()
            .any(|clip| clip.clip_id == shot.clip_id)
        {
            self.view.message = "Klip nije pronadjen.".into();
            return true;
        }
        self.view.chosen_shot_id = Some(shot.shot_id);
        self.pending_shot = Some(PendingShot {
            clip_id: shot.clip_id.clone(),
            in_frame: shot.in_frame,
            out_frame: shot.out_frame,
        });
        self.posters.prioritize(&shot.clip_id);
        self.preview.open(&shot.clip_id);
        true
    }

    fn reload_shorts(&mut self) {
        let Some(file) = self.workspace_file.as_deref() else {
            self.view.shorts.clear();
            return;
        };
        match qnc_virtual_shots::list_shorts(file) {
            Ok(rows) => {
                let previous = std::mem::take(&mut self.view.shorts);
                let mut shorts: Vec<EditorialShort> =
                    rows.into_iter().map(|row| self.short_card(row)).collect();
                qnc_virtual_short_cards::preserve_loaded_posters(&mut shorts, &previous);
                self.view.shorts = shorts;
                self.request_posters();
            }
            Err(error) => {
                self.view.shorts.clear();
                self.view.message = error;
            }
        }
    }

    fn short_card(&self, row: qnc_virtual_shots::ShortClip) -> EditorialShort {
        let parent = self
            .view
            .clips
            .iter()
            .find(|clip| clip.clip_id == row.clip_id);
        qnc_virtual_short_cards::build_card(row, parent.map(parent_clip))
    }
}

fn parent_clip(clip: &EditorialClip) -> ParentClip {
    ParentClip {
        duration_seconds: clip.duration_seconds,
        duration_frames: clip.duration_frames,
        import_status: clip.import_status.clone(),
        imported_media_uri: clip.imported_media_uri.clone(),
    }
}

/// The new list of imported clips; a poster that is already loaded for the same address is kept.
fn merge_clips(new: Vec<ClipSummary>, previous: &[EditorialClip]) -> Vec<EditorialClip> {
    new.into_iter()
        .map(|clip| {
            let image = previous
                .iter()
                .find(|old| old.clip_id == clip.clip_id && old.thumb_uri == clip.thumbnail_uri)
                .and_then(|old| old.thumb_image.clone());
            EditorialClip {
                clip_id: clip.clip_id,
                name: clip.name,
                duration_seconds: clip.duration_seconds,
                duration_frames: clip.duration_frames,
                imported: clip.imported,
                thumb_uri: clip.thumbnail_uri,
                thumb_image: image,
                import_status: clip.import_status,
                imported_media_uri: clip.imported_media_uri.unwrap_or_default(),
            }
        })
        .collect()
}

fn read_catalog(
    reader: &SettingsReader,
    shown_project: Option<&str>,
    shown_signature: Option<&CatalogSignature>,
) -> LoadResult {
    let settings = reader.read().map_err(|error| error.to_string())?;
    settings.validate().map_err(|error| error.to_string())?;
    let content = ContentReader::for_project(reader, &settings)?;
    let signature = content.signature()?;
    let unchanged =
        shown_project == Some(settings.project_id.as_str()) && shown_signature == Some(&signature);
    let clips = if unchanged {
        None
    } else {
        Some(content.summaries()?)
    };
    Ok(Loaded {
        settings,
        content,
        signature,
        clips,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: &str, name: &str) -> EditorialClip {
        EditorialClip {
            clip_id: id.into(),
            name: name.into(),
            duration_seconds: 10.0,
            duration_frames: 250,
            imported: true,
            thumb_uri: None,
            thumb_image: None,
            import_status: String::new(),
            imported_media_uri: String::new(),
        }
    }

    #[test]
    fn save_without_a_confirmed_range_does_not_write() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("clip-a", "Mironik")];
        app.view.preview.clip_id = Some("clip-a".into());
        app.dispatch(EditorialIntent::action(action_ids::SAVE_VIRTUAL_SHOT));
        assert!(app.view().message.contains("IN i OUT"));
    }

    #[test]
    fn virtual_tab_is_a_separate_list_and_a_short_duration_is_seconds_and_frames() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("clip-a", "Mironik")];
        app.dispatch(EditorialIntent::SwitchLibraryTab(LibraryTab::Virtual));
        assert_eq!(app.view().library_tab, LibraryTab::Virtual);
        assert!(app.view().shorts.is_empty());
        let label = qnc_virtual_short_cards::build_card(
            qnc_virtual_shots::ShortClip {
                shot_id: "shot-a".into(),
                clip_id: "clip-a".into(),
                in_frame: 0,
                out_frame: 57,
                name: "Mironik 001".into(),
                in_still_uri: None,
                out_still_uri: None,
                still_status: "pending".into(),
            },
            Some(parent_clip(&clip("clip-a", "Mironik"))),
        )
        .duration_label;
        assert_eq!(label, "2:07");
    }

    #[test]
    fn starts_passive_and_empty() {
        let app = EditorialApplication::default();
        assert!(app.view().clips.is_empty());
        assert!(app.view().chosen_clip_id().is_none());
        assert!(!app.has_player());
        assert!(app.next_repaint_delay().is_none());
    }

    #[test]
    fn missing_settings_end_in_a_controlled_error_not_a_default_project() {
        let root =
            std::env::temp_dir().join(format!("qnc_editorial_no_project_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let app = EditorialApplication::new(&root);
        assert!(app.view().clips.is_empty());
        assert!(app.shown_project.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_clip_is_rejected_without_creating_a_player() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi")];
        assert!(app.dispatch(EditorialIntent::PreviewClip("missing".into())));
        assert_eq!(app.view().message, "Klip nije pronadjen.");
        assert!(!app.has_player());
        assert!(app.view().chosen_clip_id().is_none());
    }

    #[test]
    fn choosing_a_clip_without_a_project_reports_it_in_the_footer() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi")];
        assert!(app.dispatch(EditorialIntent::PreviewClip("a".into())));
        assert_eq!(app.view().chosen_clip_id(), Some("a"));
        assert_eq!(app.footer_status(), "Radne postavke projekta nisu ucitane.");
        assert!(!app.has_player());
    }

    #[test]
    fn transport_without_a_player_says_so() {
        let mut app = EditorialApplication::default();
        for action in [
            action_ids::PLAY_PAUSE,
            action_ids::STEP_BACK_FRAME,
            action_ids::STEP_FORWARD_FRAME,
        ] {
            app.dispatch(EditorialIntent::action(action));
            assert_eq!(app.footer_status(), "Broadcast Player nije povezan.");
        }
        app.dispatch(EditorialIntent::Timeline(TimelineIntent::CueFrame(10)));
        assert_eq!(app.footer_status(), "Broadcast Player nije povezan.");
    }

    #[test]
    fn current_clip_label_follows_the_chosen_clip() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi"), clip("b", "Drugi")];
        assert_eq!(app.view().current_clip_label(), None);
        app.view.preview.clip_id = Some("b".into());
        assert_eq!(app.view().current_clip_label(), Some("Drugi"));
    }
}

#[cfg(test)]
mod poster_tests {
    use super::*;

    fn summary(id: &str, poster: Option<&str>) -> ClipSummary {
        ClipSummary {
            clip_id: id.into(),
            name: id.into(),
            duration_seconds: 1.0,
            duration_frames: 25,
            imported: true,
            thumbnail_uri: poster.map(str::to_string),
            import_status: "imported".into(),
            imported_media_uri: None,
        }
    }

    fn loaded_clip(id: &str, poster: &str) -> EditorialClip {
        EditorialClip {
            clip_id: id.into(),
            name: id.into(),
            duration_seconds: 1.0,
            duration_frames: 25,
            imported: true,
            thumb_uri: Some(poster.into()),
            import_status: String::new(),
            imported_media_uri: String::new(),
            thumb_image: Some(std::sync::Arc::new(qnc_image_assets::RgbaImage {
                size: [1, 1],
                pixels: vec![0, 0, 0, 255],
                content_key: 1,
            })),
        }
    }

    #[test]
    fn a_poster_already_loaded_for_the_same_address_is_kept_when_the_list_changes() {
        let previous = vec![loaded_clip("a", "qnc://x/a.jpg")];
        let merged = merge_clips(
            vec![
                summary("a", Some("qnc://x/a.jpg")),
                summary("b", Some("qnc://x/b.jpg")),
            ],
            &previous,
        );
        assert!(merged[0].thumb_image.is_some());
        assert!(merged[1].thumb_image.is_none());
        assert_eq!(merged[1].thumb_uri.as_deref(), Some("qnc://x/b.jpg"));
    }

    #[test]
    fn a_poster_at_another_address_is_loaded_again() {
        let previous = vec![loaded_clip("a", "qnc://x/card/a.jpg")];
        let merged = merge_clips(vec![summary("a", Some("qnc://x/project/a.jpg"))], &previous);
        assert!(
            merged[0].thumb_image.is_none(),
            "the project poster replaces the card poster"
        );
    }

    #[test]
    fn a_clip_without_a_poster_address_keeps_the_placeholder() {
        let merged = merge_clips(vec![summary("a", None)], &[]);
        assert!(merged[0].thumb_uri.is_none() && merged[0].thumb_image.is_none());
    }
}
