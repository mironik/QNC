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
use qnc_virtual_short_cards::ParentClip;
use qnc_virtual_short_stills::VirtualShortStillCache;
use qnc_work_settings::WorkSettings;

pub use view::{
    action_ids, EditorialClip, EditorialIntent, EditorialShort, EditorialView, LibraryTab,
    MonitorFrame, PreviewView,
};

/// Finds the QNC root: the one finder of the diagnostics block, never a copy of it.
pub use qnc_dev_diagnostics::locate_qnc_root;

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
    short_stills: VirtualShortStillCache,
    pool_shots: qnc_virtual_shots::PoolShotsReader,
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
            short_stills: VirtualShortStillCache::default(),
            pool_shots: Default::default(),
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
    pub fn footer_status(&self) -> &str {
        // The story's answer first (live 2026-10-18: a refused segment said why only under
        // the player's message, so nothing seemed to happen).
        [&self.view.segments.message, &self.view.preview.message]
            .into_iter()
            .find(|message| !message.is_empty())
            .unwrap_or(&self.view.message)
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
        self.preview.next_repaint_delay().or_else(|| busy.then(|| Duration::from_millis(15)))
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
        if self.preview.poll_pointed(pointed) {
            changed = true;
        }
        for poster in self.posters.poll() {
            let clips = &mut self.view.clips;
            changed |= match clips.iter_mut().find(|clip| clip.clip_id == poster.clip_id) {
                Some(clip) => {
                    clip.thumb_image = Some(poster.image);
                    true
                }
                None => qnc_virtual_short_cards::apply_poster(
                    &mut self.view.shorts,
                    &poster.clip_id,
                    poster.image,
                ),
            };
        }
        changed |= self.pool_shots.take().map(|rows| self.apply_shorts(rows)).is_some();
        self.sync_preview_view();
        changed
    }

    fn sync_preview_view(&mut self) {
        if self.segments.poll() { self.store_short_stills(None) } // posters of the covers just saved
        let previous_clip_id = self.view.preview.clip_id.clone();
        let previous_timeline = self.view.preview.timeline;
        self.view.preview = self.preview.view().clone();
        let assets = std::mem::take(&mut self.view.preview.assets);
        let posters = self.view.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.thumb_image.as_deref()));
        self.view.preview.assets = qnc_timeline_assets::with_clip_poster(assets, posters);
        if previous_clip_id == self.view.preview.clip_id {
            let timeline = self.view.preview.timeline.preserving_source_marks_from(&previous_timeline);
            self.view.preview.timeline = timeline;
        } else {
            self.short_stills.clear_if_clip_changed(self.view.preview.clip_id.as_deref());
        }
        if let Some((clip_id, in_frame, out_frame, edge)) = self.segments.take_source_request() {
            self.open_marked(&clip_id, (in_frame, out_frame), edge); // Edit of a segment, or its edge
        }
        self.pending_marks.apply(self.view.preview.clip_id.as_deref(), &mut self.view.preview.timeline);
        // The still of IN and OUT is the picture the monitor shows there, however the mark got there.
        self.short_stills.follow(self.preview.shown_clip_picture(), (self.view.preview.timeline.source_in_frame, self.view.preview.timeline.source_out_frame));
        let timebase = self.preview.player_view().source_timebase();
        let timeline = self.view.preview.timeline;
        let source = self.focus.source_timeline_mut(); // the one source timeline: lanes, channels, timecode
        self.view.source_timeline = source.sync(self.view.preview.clip_id.as_deref(), self.content_target.as_ref().map(|targets| &targets.0)).clone();
        self.segments.set_source(SourcePick::new(
            self.view.chosen_clip_id(),
            self.view.current_clip_label(),
            self.view.preview.timeline.visible_source_marks(),
            (timeline.source_in_frame, timeline.duration_frames),
            timebase.map(|timebase| (timebase.fps_num, timebase.fps_den)),
        ).with_lanes(source.kept_lanes(), source.holds_enter()));
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
        self.view.focus = self.focus.poll((&mut self.preview, &mut self.wrap, &mut self.segments, &mut self.view.preview.timeline));
        self.view.segments = self.segments.view_with_waves(self.view.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.duration_frames, clip.thumb_image.as_ref())));
        self.trim_stills.fill(&mut self.view.segments, self.preview.still_locator(), self.preview.shown_program_picture().and_then(|s| Some((self.segments.stored_picture_at(s.frame)?, s)))); // edit pictures, the monitor's first
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

    /// The actions of the application itself: library tabs, IN/OUT with their
    /// stills and saving a short.
    fn application_action(&mut self, action_id: &str) -> bool {
        let picked = self.focus.take_pick(); // an arrow on the pool tabs, Enter on a dock button
        let action_id = picked.as_deref().unwrap_or(action_id);
        match action_id {
            tab if LibraryTab::from_action(tab).is_some() => {
                self.view.library_tab = LibraryTab::from_action(tab).unwrap_or_default();
                true
            }
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
            action_ids::SAVE_VIRTUAL_SHOT if self.segments.sync_holds_enter() || self.segments.marks_used_by_cover(self.view.preview.clip_id.as_deref(), self.view.preview.timeline.source_in_frame) => true,
            action_ids::SAVE_VIRTUAL_SHOT => self.save_virtual_shot(),
            _ => false,
        }
    }

    /// Writes one short from the IN/OUT the source timeline is showing.
    fn save_virtual_shot(&mut self) -> bool {
        let Some(clip_id) = self.view.chosen_clip_id().map(str::to_string) else {
            self.view.message = qnc_ui_text::text("story.msg.pick_clip").into();
            return true;
        };
        let Some(clip) = self.view.clips.iter().find(|clip| clip.clip_id == clip_id) else {
            self.view.message = qnc_ui_text::text("ingest.msg.clip_not_found").into();
            return true;
        };
        if !clip.imported {
            self.view.message = qnc_ui_text::fill("story.msg.clip_not_imported", &[("clip", &clip_id)]);
            return true;
        }
        let Some((in_frame, out_frame)) = self.view.preview.timeline.visible_source_marks() else {
            self.view.message = qnc_ui_text::text("story.msg.marks_not_confirmed").into();
            return true;
        };
        let Some((_, target)) = self.content_target.clone() else {
            self.view.message = qnc_ui_text::text("story.msg.db_not_writable").into();
            return true;
        };
        let Some(project_id) = self.shown_project.clone() else {
            self.view.message = qnc_ui_text::text("story.msg.no_active_project").into();
            return true;
        };
        let name = clip.name.clone();
        match qnc_virtual_shots::save_short_now(
            &target,
            &project_id,
            &clip_id,
            &name,
            in_frame,
            out_frame,
        ) {
            Ok(shot) => {
                self.store_short_stills(Some((&shot.shot_id, &clip_id, (in_frame, out_frame))));
                self.reload_shorts();
                self.view.library_tab = LibraryTab::Virtual;
                self.view.chosen_shot_id = Some(shot.shot_id.clone());
                self.view.message = qnc_ui_text::fill("story.msg.subclip_saved", &[("shot", &shot.shot_id)]);
            }
            Err(error) => self.view.message = error,
        }
        true
    }

    /// The stills of a saved short (`Some`) or the posters of the covers just saved.
    fn store_short_stills(&mut self, short: Option<(&str, &str, (u64, u64))>) {
        let (Some((_, target)), Some(settings)) = (&self.content_target, &self.current_settings) else {
            return;
        };
        let place = qnc_shot_stills::Place { target, settings, project_dir: self.project_dir.as_deref() };
        let stored = match short {
            Some((shot, clip, marks)) => place.short(&self.short_stills, shot, clip, marks),
            None => place.covers(&self.short_stills, self.segments.take_created_cover_shots()),
        };
        if let Err(error) = stored {
            self.view.preview.message = error;
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
        self.preview.open_at(clip_id, at);
        true
    }

    fn reload_shorts(&mut self) {
        let Some((_, target)) = self.content_target.as_ref() else { return self.view.shorts.clear() };
        self.pool_shots.request(target); // read off the form thread
    }

    fn apply_shorts(&mut self, rows: Result<Vec<qnc_virtual_shots::ShortClip>, String>) {
        match rows {
            Ok(rows) => {
                let mut shorts: Vec<EditorialShort> = rows.into_iter().map(|row| self.short_card(row)).collect();
                qnc_virtual_short_cards::preserve_loaded_posters(&mut shorts, &self.view.shorts);
                self.view.shorts = shorts;
                self.request_posters();
            }
            Err(error) => { self.view.shorts.clear(); self.view.message = error }
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
            ..Default::default()
        }
    }

    #[test]
    fn save_without_a_confirmed_range_does_not_write() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("clip-a", "Mironik")];
        app.view.preview.clip_id = Some("clip-a".into());
        app.dispatch(EditorialIntent::action(action_ids::SAVE_VIRTUAL_SHOT));
        assert!(app.view().message == qnc_ui_text::text("story.msg.marks_not_confirmed"));
    }

    #[test]
    fn virtual_tab_is_a_separate_list_and_a_short_duration_is_seconds_and_frames() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("clip-a", "Mironik")];
        app.dispatch(EditorialIntent::action("editorial_tab_virtual"));
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
                b_roll: false,
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
        let app = EditorialApplication::new(&root, FocusLayout::default());
        assert!(app.view().clips.is_empty());
        assert!(app.shown_project.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_clip_is_rejected_without_creating_a_player() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi")];
        assert!(app.dispatch(EditorialIntent::PreviewClip("missing".into())));
        assert_eq!(app.view().message, qnc_ui_text::text("ingest.msg.clip_not_found"));
        assert!(!app.has_player());
        assert!(app.view().chosen_clip_id().is_none());
    }

    #[test]
    fn choosing_a_clip_without_a_project_reports_it_in_the_footer() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi")];
        assert!(app.dispatch(EditorialIntent::PreviewClip("a".into())));
        assert_eq!(app.view().chosen_clip_id(), Some("a"));
        assert_eq!(app.footer_status(), qnc_ui_text::text("player.work_settings_project_loaded"));
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
            assert_eq!(app.footer_status(), qnc_ui_text::text("player.broadcast_player_connected"));
        }
        app.dispatch(EditorialIntent::action(qnc_action_event::ActionEvent::key("timeline_cue_frame").with_value(10u64).at("source_timeline")));
        assert_eq!(app.footer_status(), qnc_ui_text::text("player.broadcast_player_connected"));
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
            ..Default::default()
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
