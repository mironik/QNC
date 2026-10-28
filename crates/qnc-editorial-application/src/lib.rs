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

use qnc_active_project_read::{ActiveProjectChange, ActiveProjectReader, ShownProject};
use qnc_clip_posters::ClipPosters;
use qnc_content_read::{CatalogSignature, ClipSummary, ContentReader};
pub use qnc_panel_focus::{FocusLayout, Panel, PanelFocus};
use qnc_program_segments::SourcePick;
use qnc_source_bindings::TransportBindings;
use qnc_source_preview::{PreviewContext, SourcePreview};
use qnc_work_settings::WorkSettings;

pub use view::{
    action_ids, EditorialClip, EditorialIntent, EditorialShort, EditorialView, LibraryTab,
    MonitorFrame, PreviewView,
};

/// Finds the QNC root: the one finder of the diagnostics block, never a copy of it.
pub use qnc_dev_diagnostics::resolve_qnc_root as locate_qnc_root;

struct Loaded {
    settings: WorkSettings,
    signature: CatalogSignature,
    /// `None` when the signature matched what is already shown.
    clips: Option<Vec<ClipSummary>>,
    /// Read with the catalog, off the form thread (external audit 2026-10-17): the project
    /// folder, the content and story bases and the artifact reader of the project.
    project_dir: Option<std::path::PathBuf>,
    targets: Result<(qnc_content_store::ContentTarget, qnc_db_broker::ProjectDbTarget), String>,
    artifacts: Option<std::sync::Arc<dyn qnc_timeline_assets::TimelineArtifactRead>>,
    /// The preview's context, built here too: taking it reads no database.
    preview: Option<PreviewContext>,
}

type LoadResult = Result<Loaded, String>;

pub struct EditorialApplication {
    view: EditorialView,
    preview: SourcePreview,
    posters: ClipPosters,
    active_project_reader: Option<ActiveProjectReader>,
    bindings: Result<TransportBindings, String>,
    load_result: Option<Receiver<LoadResult>>,
    shown_project: Option<String>,
    shown_signature: Option<CatalogSignature>,
    content_target: Option<(qnc_content_store::ContentTarget, qnc_db_broker::ProjectDbTarget)>,
    current_settings: Option<WorkSettings>,
    project_dir: Option<std::path::PathBuf>,
    /// IN/OUT a clip opens with (a short, an edited segment) once its timeline is ready.
    pending_marks: qnc_pending_marks::PendingMarks,
    trim_stills: qnc_trim_stills::TrimStills,
    short_stills: qnc_shot_stills::ShotStills,
    pool_shots: qnc_virtual_shots::PoolShotsReader,
    short_save: qnc_short_save::ShortSave,
    segments: qnc_program_segments::ProgramSegments,
    wrap: qnc_wrap_session::WrapSession,
    focus: PanelFocus,
}

impl Default for EditorialApplication {
    fn default() -> Self {
        Self {
            view: EditorialView::default(),
            preview: SourcePreview::new(),
            posters: ClipPosters::new(),
            active_project_reader: None,
            bindings: Err(qnc_ui_text::text("story.msg.sources_not_loaded").into()),
            load_result: None,
            shown_project: None,
            shown_signature: None,
            content_target: None,
            current_settings: None,
            project_dir: None,
            pending_marks: qnc_pending_marks::PendingMarks::new(),
            trim_stills: qnc_trim_stills::TrimStills::new(),
            short_stills: Default::default(),
            pool_shots: Default::default(),
            short_save: Default::default(),
            segments: qnc_program_segments::ProgramSegments::new(),
            wrap: qnc_wrap_session::WrapSession::new(),
            focus: PanelFocus::new(),
        }
    }
}

impl EditorialApplication {
    /// Starts from the QNC root with the Tab walk of its form. Missing settings end in a
    /// controlled error, never in an invented project.
    pub fn new(root: impl AsRef<Path>, focus: FocusLayout) -> Self {
        let mut app = Self { focus: PanelFocus::with_layout(focus), ..Self::default() };
        app.bindings = qnc_source_bindings::load(root.as_ref());
        app.preview.wake_worker_from(root.as_ref());
        match ActiveProjectReader::from_root(root.as_ref()) {
            Ok(reader) => {
                app.active_project_reader = Some(reader);
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
    pub fn footer_status(&self) -> qnc_status_line::Status<'_> {
        // The story's answer first (live 2026-10-18: a refused segment said why only under
        // the player's message, so nothing seemed to happen); an export's with its file.
        let preview = qnc_status_line::Status::plain(&self.view.preview.message);
        qnc_status_line::first(&[self.view.segments.status(), preview], &self.view.message)
    }

    pub fn has_player(&self) -> bool {
        self.preview.has_player()
    }

    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.preview.notify_on_change(notify);
    }

    /// Shown: rereads the active project, cheap when nothing changed (the clips are
    /// loaded again only when the lightweight catalog signature differs). Hidden: the
    /// player is closed so it neither plays nor reads media behind another surface.
    pub fn set_active(&mut self, active: bool) {
        if !active {
            return self.preview.close();
        }
        self.load_catalog();
    }

    /// Delay until the next repaint that this surface needs, if any.
    pub fn next_repaint_delay(&self) -> Option<Duration> {
        let busy = self.load_result.is_some() || self.posters.has_pending_work() || self.trim_stills.loading();
        let busy = busy || self.segments.has_pending_work() || self.segments.reading() || self.focus.busy();
        // A written story or an edit picture shows within a frame or two.
        self.preview.next_repaint_delay().or_else(|| busy.then(|| Duration::from_millis(15))).or_else(|| self.focus.working().then(|| Duration::from_millis(250)))
    }

    fn fail(&mut self, error: String) {
        self.view.loading = false;
        self.view.message = error;
    }

    fn load_catalog(&mut self) {
        if self.load_result.is_some() {
            return;
        }
        let Some(reader) = self.active_project_reader.clone() else {
            self.fail(qnc_ui_text::text("ingest.msg.no_settings_reader_configured").into());
            return;
        };
        let shown_project = self.shown_project.clone();
        let shown_signature = self.shown_signature.clone();
        let bindings = self.bindings.clone().ok();
        self.view.loading = shown_project.is_none();
        let (send, receive) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("editorial-catalog".into())
            .spawn(move || {
                let _ = send.send(read_catalog(&reader, shown_project, shown_signature, bindings));
            }) {
            Ok(_) => self.load_result = Some(receive),
            Err(_) => self.fail(qnc_ui_text::text("story.msg.catalog_read_failed").into()),
        }
    }

    /// Applies finished background work and the player state. Returns whether
    /// the view changed.
    pub fn poll(&mut self, pointed: Option<&str>) -> bool {
        let mut changed = self.poll_catalog();
        if qnc_dev_diagnostics::timed_block("poll-preview", || self.preview.poll_pointed(pointed)) {
            changed = true;
        }
        let (applied, others) = self.posters.poll_into(&mut self.view.clips);
        changed |= applied;
        for poster in others {
            changed |= qnc_virtual_short_cards::apply_poster(&mut self.view.shorts, &poster.clip_id, poster.image);
        }
        if self.short_stills.take_published() { self.reload_shorts() } // a poster was written
        changed |= self.pool_shots.take().map(|rows| self.apply_shorts(rows)).is_some();
        changed |= self.short_save.poll().map(|saved| self.short_saved(saved)).is_some();
        self.sync_preview_view();
        changed
    }

    fn sync_preview_view(&mut self) {
        if qnc_dev_diagnostics::timed_block("poll-segments", || self.segments.poll()) { self.store_short_stills(None) } // posters of the covers just saved
        let previous_clip_id = self.view.preview.clip_id.clone();
        let previous_timeline = self.view.preview.timeline;
        self.view.preview = qnc_dev_diagnostics::timed_block("poll-preview-copy", || self.preview.view().clone());
        let assets = std::mem::take(&mut self.view.preview.assets);
        let posters = self.view.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.thumb_image.as_deref()));
        self.view.preview.assets = qnc_timeline_assets::with_clip_poster(assets, posters);
        if previous_clip_id == self.view.preview.clip_id {
            let timeline = self.view.preview.timeline.preserving_source_marks_from(&previous_timeline);
            self.view.preview.timeline = timeline;
        } else {
            self.short_stills.clear_if_clip_changed(self.view.preview.clip_id.as_deref());
        }
        if let Some((clip_id, in_frame, out_frame, edge)) = self.segments.take_source_request().or_else(|| self.focus.take_source_request().map(|(c, i, o)| (c, i, o, None))) {
            self.open_marked(&clip_id, (in_frame, out_frame), edge); // Edit of a segment or its edge, a statement
        }
        self.pending_marks.apply(self.view.preview.clip_id.as_deref(), &mut self.view.preview.timeline);
        // The still of IN and OUT is the picture the monitor shows there, however the mark got there.
        self.short_stills.follow(self.preview.shown_clip_picture(), (self.view.preview.timeline.source_in_frame, self.view.preview.timeline.source_out_frame));
        let timebase = self.preview.player_view().source_timebase();
        let timeline = self.view.preview.timeline;
        let source = self.focus.source_timeline_mut(); // the one source timeline: lanes, channels, timecode
        self.view.source_timeline = qnc_dev_diagnostics::timed_block("poll-source-sync", || source.sync(self.view.preview.clip_id.as_deref(), self.content_target.as_ref().map(|targets| &targets.0), &mut self.preview).clone());
        self.segments.set_source(SourcePick::new(
            self.view.chosen_clip_id(),
            self.view.current_clip_label(),
            self.view.preview.timeline.visible_source_marks(),
            (timeline.source_in_frame, timeline.duration_frames),
            timebase.map(|timebase| (timebase.fps_num, timebase.fps_den)),
        ).with_lanes(source.kept_lanes(), source.holds_enter()).with_taken(timeline.taken_source_mark()));
        // Source and Wrap stay apart (v5): the Wrap timeline plays the program in the same player.
        let seek = self.segments.take_seek();
        let total = self.segments.view().total_frames;
        let changed = self.segments.take_program_changed(); // the program opens again
        changed.then(|| self.reload_shorts()); // a cover writes its B-roll shot
        let confirmed = self.segments.sync_frame(self.preview.program_frame(), self.preview.clip_frame()); // Sync follows the source
        if let Some(sync) = self.segments.sync_source() {
            let timeline = self.view.preview.timeline;
            self.view.preview.timeline = SourcePreview::with_sync_source(timeline, sync);
        }
        match self.wrap.apply(seek, total, changed) {
            Some(r) => _ = self.preview.show_program_frame(r.frame(), r.opens()),
            None => self.wrap.follow_program(confirmed),
        }
        self.segments.set_playhead(Some(self.wrap.playhead()));
        self.view.focus = qnc_dev_diagnostics::timed_block("poll-focus", || self.focus.poll((&mut self.preview, &mut self.wrap, &mut self.segments, &mut self.view.preview.timeline, &mut self.view.clips)));
        if self.view.focus.transcript.all_done { self.view.library_tab = LibraryTab::Transcript } // all asked are transcribed (user 2026-10-28)
        self.view.segments = qnc_dev_diagnostics::timed_block("poll-segments-view", || self.segments.view_with_waves(self.view.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.duration_frames, clip.thumb_image.as_ref()))));
        qnc_dev_diagnostics::timed_block("poll-trim", || self.trim_stills.fill(&mut self.view.segments, self.preview.still_locator(), self.preview.shown_program_picture().and_then(|s| Some((self.segments.stored_picture_at(s.frame)?, s))))); // edit pictures, the monitor's first
    }


    fn poll_catalog(&mut self) -> bool {
        let Some(receiver) = self.load_result.as_ref() else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                Err(qnc_ui_text::text("story.msg.catalog_read_stopped").into())
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
        let project_id = loaded.settings.project_id.clone();
        let settings = loaded.settings.clone();
        // A different project must never inherit the preceding one's clips or preview.
        if self.shown_project.as_deref() != Some(project_id.as_str()) {
            self.view.clips.clear();
            self.preview.close();
            self.posters.reset();
            self.short_stills.clear();
        }
        self.project_dir = loaded.project_dir.clone();
        let project_folder =
            self.project_dir
                .clone()
                .map(|dir| qnc_media_thumbnail::ProjectFolder {
                    root_uri: loaded.settings.output_root_uri.clone(),
                    dir,
                });
        if let Ok(bindings) = &self.bindings {
            self.posters
                .configure(&bindings.readers(), project_folder);
        }
        if let Some(clips) = loaded.clips {
            self.view.clips = merge_clips(clips, &self.view.clips);
            self.request_posters();
        }
        match (&self.active_project_reader, &self.bindings) {
            (Some(_), Ok(_)) => {
                self.content_target = match loaded.targets {
                    Ok(targets) => {
                        self.segments.configure(targets.1.clone(), &project_id, loaded.artifacts, Some(targets.0.clone()));
                        Some(targets)
                    }
                    Err(error) => {
                        self.view.message = error;
                        None
                    }
                };
                if let Some(context) = loaded.preview {
                    self.preview.configure(context);
                }
            }
            (_, Err(error)) => self.view.preview.message = error.clone(),
            _ => {}
        }
        self.shown_project = Some(project_id);
        self.shown_signature = Some(loaded.signature);
        self.current_settings = Some(settings);
        self.reload_shorts();
        self.view.message = match self.view.clips.len() {
            0 => qnc_ui_text::text("story.msg.no_imported_clips").to_string(),
            count => qnc_ui_text::fill("story.msg.clip_count", &[("count", &count.to_string())]),
        };
    }

    /// Asks for the posters the list still lacks; the chosen clip goes first (v5 order).
    fn request_posters(&mut self) {
        let mut wanted = qnc_media_thumbnail::missing(&self.view.clips);
        wanted.extend(qnc_virtual_short_cards::poster_requests(&self.view.shorts));
        self.posters.request(wanted, self.view.chosen_clip_id());
    }

    /// Handles one intent from the form. Returns whether the view changed.
    pub fn dispatch(&mut self, intent: EditorialIntent) -> bool {
        let changed = match intent {
            EditorialIntent::PreviewClip(clip_id) => {
                let _ = self.focus.chose_clip(&mut self.wrap); // the keyboard stays on the pool after a click
                self.view.chosen_shot_id = None;
                self.pending_marks.clear();
                self.short_stills.clear_if_clip_changed(Some(&clip_id));
                if self.view.clips.iter().any(|clip| clip.clip_id == clip_id) {
                    self.posters.prioritize(&clip_id);
                    self.preview.open(&clip_id)
                } else {
                    self.view.message = qnc_ui_text::text("ingest.msg.clip_not_found").into();
                    true
                }
            }
            // The short opens with its IN/OUT, then the keyboard is where the choice left it.
            EditorialIntent::PreviewShort(shot_id) => self.open_short(&shot_id) && self.focus.chose_clip(&mut self.wrap),
            EditorialIntent::Action(event) => {
                let pieces = (&mut self.preview, &mut self.wrap, &mut self.segments, &mut self.view.preview.timeline);
                let action_id = event.action_id.clone();
                match self.focus.route(event, pieces) {
                    Some(changed) => changed, // the action acts on its panel
                    None => self.application_action(&action_id),
                }
            }
        };
        self.sync_preview_view();
        changed
    }

    /// The actions of the application itself: library tabs and saving a short.
    fn application_action(&mut self, action_id: &str) -> bool {
        let picked = self.focus.take_pick(); // an arrow on the pool tabs, Enter on a dock button
        let action_id = picked.as_deref().unwrap_or(action_id);
        match action_id {
            tab if LibraryTab::from_action(tab).is_some() => {
                self.view.library_tab = LibraryTab::from_action(tab).unwrap_or_default();
                true
            }
            action_ids::SAVE_VIRTUAL_SHOT if self.segments.sync_holds_enter() || self.segments.marks_used_by_cover(self.view.preview.clip_id.as_deref(), self.view.preview.timeline.source_in_frame) => true,
            action_ids::SAVE_VIRTUAL_SHOT => {
                let target = self.content_target.as_ref().map(|targets| &targets.1);
                let marks = self.view.preview.timeline.visible_source_marks();
                if let Err(message) = self.short_save.start(self.view.chosen_clip_id(), &self.view.clips, marks, target, self.shown_project.as_deref()) {
                    self.view.message = message;
                }
                true
            }
            _ => false,
        }
    }

    /// A short is saved (piece `qnc-short-save`): its stills, the SubClip tab shows it.
    fn short_saved(&mut self, saved: Result<qnc_short_save::Saved, String>) {
        let saved = match saved {
            Ok(saved) => saved,
            Err(error) => return self.view.message = error,
        };
        self.store_short_stills(Some((&saved.shot_id, &saved.clip_id, saved.marks)));
        self.reload_shorts();
        self.view.library_tab = LibraryTab::Virtual;
        self.view.message = qnc_ui_text::fill("story.msg.subclip_saved", &[("shot", &saved.shot_id)]);
        self.view.chosen_shot_id = Some(saved.shot_id);
    }

    /// The stills of a saved short (`Some`) or the posters of the covers just saved.
    fn store_short_stills(&mut self, short: Option<(&str, &str, (u64, u64))>) {
        let (Some((_, target)), Some(settings)) = (&self.content_target, &self.current_settings) else {
            return;
        };
        let place = qnc_shot_stills::Place { target, settings, project_dir: self.project_dir.as_deref(), locator: self.preview.still_locator() };
        let covers = if short.is_none() { self.segments.take_created_cover_shots() } else { Vec::new() };
        self.short_stills.store(place, short, covers);
    }

    fn open_short(&mut self, shot_id: &str) -> bool {
        let Some(shot) = self
            .view
            .shorts
            .iter()
            .find(|shot| shot.shot_id == shot_id)
            .cloned()
        else {
            self.view.message = qnc_ui_text::text("story.msg.subclip_not_found").into();
            return true;
        };
        if !self
            .view
            .clips
            .iter()
            .any(|clip| clip.clip_id == shot.clip_id)
        {
            self.view.message = qnc_ui_text::text("ingest.msg.clip_not_found").into();
            return true;
        }
        self.view.chosen_shot_id = Some(shot.shot_id);
        self.open_marked(&shot.clip_id, (shot.in_frame, shot.out_frame), None)
    }

    /// Opens a clip in the source view with IN/OUT (a short, an edited segment).
    fn open_marked(&mut self, clip_id: &str, marks: (u64, u64), edge: Option<bool>) -> bool {
        self.focus.to_source(&mut self.wrap);
        let at = self.pending_marks.set(clip_id, marks, edge);
        self.posters.prioritize(clip_id);
        self.preview.open_at(clip_id, at) || self.preview.cue(at) // the clip already shown: the playhead to IN
    }

    fn reload_shorts(&mut self) {
        let Some((_, target)) = self.content_target.as_ref() else { return self.view.shorts.clear() };
        self.pool_shots.request(target); // read off the form thread
    }

    fn apply_shorts(&mut self, rows: Result<Vec<qnc_virtual_shots::ShortClip>, String>) {
        match rows {
            Ok(rows) => {
                self.view.shorts = qnc_virtual_short_cards::cards(rows, &self.view.clips, &self.view.shorts);
                self.request_posters();
            }
            Err(error) => { self.view.shorts.clear(); self.view.message = error }
        }
    }
}

/// The new list of imported clips; a poster that is already loaded for the same address is kept.
fn merge_clips(new: Vec<ClipSummary>, previous: &[EditorialClip]) -> Vec<EditorialClip> {
    let mut clips: Vec<EditorialClip> = new.into_iter().map(EditorialClip::from_summary).collect();
    qnc_media_thumbnail::keep_loaded(&mut clips, previous);
    clips
}

fn read_catalog(
    reader: &ActiveProjectReader,
    shown_project: Option<String>,
    shown_signature: Option<CatalogSignature>,
    bindings: Option<TransportBindings>,
) -> LoadResult {
    let shown = shown_project
        .zip(shown_signature)
        .map(|(project_id, catalog_signature)| ShownProject {
            project_id,
            catalog_signature,
        });
    let change = reader
        .compare(shown.as_ref())
        .map_err(|error| error.to_string())?;
    let (snapshot, signature, unchanged) = match change {
        ActiveProjectChange::Same(snapshot, signature) => (snapshot, signature, true),
        ActiveProjectChange::ProjectChanged(snapshot, signature)
        | ActiveProjectChange::SignatureChanged(snapshot, signature) => {
            (snapshot, signature, false)
        }
        ActiveProjectChange::NoActiveProject => {
            return Err(qnc_ui_text::text("story.msg.no_active_project_in_db").into());
        }
    };
    let (settings, work) = (reader.settings_reader(), &snapshot.settings);
    let content = ContentReader::for_project(settings, work)?;
    let clips = if unchanged { None } else { Some(content.summaries()?) };
    let project_dir = settings.local_workspace_dir(work).ok().flatten();
    let story = qnc_db_broker::ProjectDbTarget::for_project(settings, work);
    let targets = qnc_content_store::ContentTarget::for_project(settings, work).and_then(|target| Ok((target, story?)));
    let artifacts = qnc_content_artifacts::timeline_artifact_reader(settings, work).ok();
    let preview = bindings.map(|bindings| PreviewContext::new(settings.clone(), work.clone(), content, bindings));
    Ok(Loaded { settings: snapshot.settings, signature, clips, project_dir, targets, artifacts, preview })
}

#[cfg(test)]
mod app_tests;
#[cfg(test)]
mod poster_tests;
