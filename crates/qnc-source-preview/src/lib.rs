//! Source preview session for one clip: it prepares the Broadcast Player from
//! the stored media record, keeps the confirmed monitor picture and timeline
//! projection, and reads the published filmstrip and waveform artifacts. It is a
//! public component: any form or application can embed it, and it knows none of
//! them. Read-only: no scan, no probe, no write to any database.
//!
//! The chain follows AGENTS.md section 4.1: work settings of the active project,
//! project content (public views), media record, player launch.

mod content;
mod still_media;
pub use qnc_still_locator::{LocatedPicture, StillLocator};

use std::{sync::Arc, time::{Duration, Instant}};

use qnc_content_read::ContentReader;
use qnc_player_client::{Action, Player, View as PlayerView};
use qnc_player_input::{InputReader, PlayerContentRead, ProgramInput};
use qnc_player_launcher::SourceTransportBinding;
pub use qnc_program_input::SoundDraft;
use qnc_source_bindings::{SourceBinding, TransportBindings};
use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_timeline_assets::{
    SourceTimelineAssets, TimelineArtifactRead, TimelineAssetContext, TimelineAssetReader,
};
use qnc_work_settings::{ProductArea, SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.source-preview";
pub const VERSION: &str = "0.1.0";

/// A picture of the story program on the monitor.
#[derive(Debug, Clone)]
pub struct ShownPicture {
    /// The program frame it shows.
    pub frame: u64,
    /// Which picture of the session it is (output generation, sequence).
    pub sequence: (u64, u64),
    pub size: [usize; 2],
    pub rgba: Arc<[u8]>,
}

/// Everything a form needs to paint the preview.
#[derive(Clone)]
pub struct PreviewView {
    pub clip_id: Option<String>,
    pub monitor_frame: Option<MonitorFrame>,
    pub video_visible: bool,
    pub monitor_message: Option<String>,
    pub timeline: TimelineProjection,
    pub assets: SourceTimelineAssets,
    pub playing: bool,
    /// Controlled error text of the last failed command; empty otherwise.
    pub message: String,
    /// The level (dBFS) of A1 and A2 at the player's output, for a meter (user 2026-10-16).
    pub output_levels: [Option<f32>; 2],
}

impl PreviewView {
    /// What the monitor shows now.
    pub fn monitor(&self) -> MonitorState {
        MonitorState {
            frame: self.monitor_frame.clone(),
            video_visible: self.video_visible,
            message: self.monitor_message.clone(),
        }
    }
}

pub use qnc_monitor::{MonitorFrame, MonitorState, MONITOR_AUTO, MONITOR_ON};

impl Default for PreviewView {
    fn default() -> Self {
        Self {
            clip_id: None,
            monitor_frame: None,
            video_visible: false,
            monitor_message: None,
            timeline: TimelineProjection::default(),
            assets: SourceTimelineAssets::empty(),
            playing: false,
            message: String::new(),
            output_levels: [None, None],
        }
    }
}

/// The project a preview works in. Where the clips and artifacts come from is decided
/// by the readers it carries, so any form over any content database can use it.
#[derive(Clone)]
pub struct PreviewContext {
    reader: SettingsReader,
    settings: WorkSettings,
    sources: Vec<SourceBinding>,
    player_content: Arc<dyn PlayerContentRead>,
    artifacts: Option<Arc<dyn TimelineArtifactRead>>,
    notice: Option<String>,
    /// Where this preview says a player works: resolved with the context, so taking the
    /// context on the form thread reads no database (external audit 2026-10-17).
    activity_target: Option<qnc_content_store::ContentTarget>,
}

impl PreviewContext {
    /// Any content database: the caller supplies the readers.
    pub fn with_readers(
        reader: SettingsReader,
        settings: WorkSettings,
        sources: Vec<SourceBinding>,
        player_content: Arc<dyn PlayerContentRead>,
        artifacts: Option<Arc<dyn TimelineArtifactRead>>,
    ) -> Self {
        let activity_target = qnc_content_store::ContentTarget::for_project(&reader, &settings).ok();
        Self {
            reader,
            settings,
            sources,
            player_content,
            artifacts,
            notice: None,
            activity_target,
        }
    }

    /// The project content views (`qnc-content-read`).
    pub fn new(
        reader: SettingsReader,
        settings: WorkSettings,
        content: ContentReader,
        bindings: TransportBindings,
    ) -> Self {
        let (artifacts, notice) = match reader.local_workspace_dir(&settings) {
            Ok(Some(project_dir)) => {
                let artifacts: Arc<dyn TimelineArtifactRead> = Arc::new(content::ArtifactReader {
                    content: content.clone(),
                    filmstrip_root_uri: settings.product_uri(ProductArea::Filmstrip),
                    filmstrip_dir: settings.product_local_dir(&project_dir, ProductArea::Filmstrip),
                });
                (Some(artifacts), None)
            }
            Ok(None) => (
                None,
                Some(qnc_ui_text::text("player.timeline_artifacts_have_local_filmstrip_binding").to_string()),
            ),
            Err(error) => (None, Some(error.to_string())),
        };
        let target = qnc_content_store::ContentTarget::for_project(&reader, &settings);
        let activity_target = target.clone().ok();
        let player_content = Arc::new(content::PlayerContent { target });
        Self {
            reader,
            settings,
            sources: bindings.sources,
            player_content,
            artifacts,
            notice,
            activity_target,
        }
    }

    pub fn project_id(&self) -> &str {
        &self.settings.project_id
    }
}

/// How long after a frame step, cue or Play the user still counts as working.
const HANDS_ON: std::time::Duration = std::time::Duration::from_secs(3);

/// The output levels (A1, A2) while a sound is drafted on a Wrap row: none. A whole output's
/// level changes every segment or cover on that lane (live 2026-10-22: the A2 level of one
/// cover was heard on the whole program), so the program opens with the draft itself; with no
/// draft the outputs go back to as recorded.
fn draft_output_level(draft: Option<&SoundDraft>, _stored: Option<&SoundDraft>) -> Option<[u16; 2]> {
    draft.is_none().then_some([100, 100])
}

/// Wakes the background worker (`qnc-worker-wake`); run off the form's thread.
#[derive(Clone)]
pub struct RecordWaker(Arc<dyn Fn() + Send + Sync>);

impl RecordWaker {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(wake))
    }
}

impl std::fmt::Debug for RecordWaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecordWaker")
    }
}

/// How often a clip waiting for its record is opened again (its record may be final now).
const RECORD_RETRY: std::time::Duration = std::time::Duration::from_millis(250);

/// How often a clip waiting for its record wakes the worker again.
const WAKE_AGAIN: std::time::Duration = std::time::Duration::from_secs(5);

pub struct SourcePreview {
    view: PreviewView,
    context: Option<PreviewContext>,
    player: Option<Player>,
    player_view: PlayerView,
    timeline_assets: TimelineAssetReader,
    /// When the artifacts of the shown clip are read again while the background worker
    /// still makes them (one read a second; every form the same, user 2026-10-17).
    assets_again_at: Option<Instant>,
    /// The clip the media pool pointed at last (its assets asked once).
    pointed: qnc_pool_pointer::Follow,
    play_when_ready: bool,
    /// The last frame step, cue or Play of the user: the machine is theirs a while after.
    touched: Option<std::time::Instant>,
    /// A pause was sent and the player still says it plays: a second pause would play again.
    pausing: bool,
    /// The Sync window whose story A1 the clip session should carry, the one it was launched
    /// with, and whether the bed was started (user 2026-10-20).
    bed_wanted: Option<(u64, u64)>,
    bed_launched: Option<(u64, u64)>,
    bed_playing: bool,
    /// The clip whose record is still completed, since when and the last try.
    record_wait: Option<(String, std::time::Instant, std::time::Instant)>,
    /// What wakes the background worker that completes the record a shown clip waits for
    /// (given by the module that composes this preview; it knows where QNC is).
    record_waker: Option<RecordWaker>,
    /// A cue sent and not yet confirmed, when it was sent, and the latest frame
    /// asked meanwhile: a fast scrub or drag sends one cue at a time, latest wins,
    /// so the player command queue never fills.
    cue_in_flight: Option<(u64, std::time::Instant)>,
    cue_next: Option<u64>,
    /// The frame last asked for, until the player confirms it: the next arrow press
    /// counts from it even when the cue in flight was released on its timeout.
    cue_asked: Option<u64>,
    /// The clip and frame the player was last asked to prepare: where a clip opened
    /// again before the player confirmed a position stays (live 2026-10-16).
    opening_at: Option<(String, u64)>,
    /// Every preview tells the project database while its player prepares or plays,
    /// so background generators of any process give way.
    activity: qnc_playback_activity::PlaybackReporter,
    activity_target: Option<qnc_content_store::ContentTarget>,
    /// The player plays a story program; the view keeps the source clip.
    program: bool,
    /// The channels of the shown clip heard on output 1 and 2 (A1, A2), when chosen, and
    /// their levels in percent (0: level 0, nothing heard).
    lead: Option<(String, [u16; 2], [u16; 2])>,
    /// The sound of a segment or cover drafted on the Wrap row, heard in the program.
    program_draft: Option<SoundDraft>,
    /// The frame asked for while the player was still being prepared (a clip or the
    /// program): cued once it is ready, never by opening it again (live 2026-10-16: the wheel
    /// over the Wrap rows opened the program on every step; audit 2026-10-20: a click on the
    /// source timeline while the clip prepared was lost).
    cue_waiting: Option<u64>,
    /// The program was opened again with the draft (another channel): Escape opens it
    /// again from the story, a level no longer goes to the output.
    program_draft_reopened: bool,
    /// The gains in dB that bring the program's A1 and A2 to the project's loudness targets,
    /// while normalising is on (user 2026-10-24); the player's output level takes them.
    normalize: Option<[f64; 2]>,
    /// The level of outputs 1 and 2 of the program in percent before normalising (a drafted
    /// level), and what the player was last given.
    program_levels: [u16; 2],
    levels_sent: Option<[u16; 2]>,
}

/// The most a normalising gain gives or takes in dB (the output level holds up to 400 %).
const NORMALIZE_MAX_DB: f64 = 12.0;

impl Default for SourcePreview {
    fn default() -> Self {
        Self {
            view: PreviewView::default(),
            context: None,
            player: None,
            player_view: PlayerView::default(),
            timeline_assets: TimelineAssetReader::default(),
            assets_again_at: None,
            pointed: qnc_pool_pointer::Follow::default(),
            play_when_ready: false,
            touched: None,
            pausing: false,
            bed_wanted: None,
            bed_launched: None,
            bed_playing: false,
            record_wait: None,
            record_waker: None,
            cue_in_flight: None,
            cue_next: None,
            cue_asked: None,
            opening_at: None,
            activity: qnc_playback_activity::PlaybackReporter::new(),
            activity_target: None,
            program: false,
            lead: None,
            program_draft: None,
            cue_waiting: None,
            program_draft_reopened: false,
            normalize: None,
            program_levels: [100, 100],
            levels_sent: None,
        }
    }
}

impl SourcePreview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Monitor Auto or On (the switch under the monitor), one choice for every monitor
    /// (`qnc-monitor`); false for any other action.
    pub fn set_monitor_mode(&mut self, action_id: &str) -> bool {
        qnc_monitor::choose(action_id)
    }

    pub fn view(&self) -> &PreviewView {
        &self.view
    }

    /// The player state as the player reports it, for forms that paint it themselves.
    pub fn player_view(&self) -> &PlayerView {
        &self.player_view
    }

    /// A play command is waiting for the prepared session.
    pub fn play_when_ready(&self) -> bool {
        self.play_when_ready
    }

    /// Uses a player the caller already made instead of creating one on `open`.
    pub fn attach_player(&mut self, player: Player) {
        self.player = Some(player);
    }

    /// The message of the last failed command; it is handed over once.
    pub fn take_message(&mut self) -> String {
        std::mem::take(&mut self.view.message)
    }

    pub fn has_player(&self) -> bool {
        self.player.is_some()
    }

    pub fn notify_on_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        if let Some(player) = &self.player {
            player.notify_on_change(notify);
        }
    }

    /// Delay until the next repaint this session needs, if any.
    pub fn next_repaint_delay(&self) -> Option<Duration> {
        if self.player.is_some() && (self.player_view.preparing || self.player_view.playing()) {
            return self.player_view.source_frame_interval();
        }
        self.timeline_assets.loading().then(|| Duration::from_millis(15))
    }

    /// Sets the project. A different project never inherits the previous clip.
    pub fn configure(&mut self, context: PreviewContext) {
        let changed = self
            .context
            .as_ref()
            .map(PreviewContext::project_id)
            .is_some_and(|old| old != context.project_id());
        if changed {
            self.close();
            self.timeline_assets.reset();
        }
        if let Some(artifacts) = &context.artifacts {
            self.timeline_assets.configure(TimelineAssetContext {
                project_id: context.settings.project_id.clone(),
                reader: artifacts.clone(),
            });
        }
        if let Some(notice) = &context.notice {
            self.view.message = notice.clone();
        }
        self.activity_target = context.activity_target.clone();
        self.context = Some(context);
    }

    fn report_activity(&mut self) {
        // Editing frame by frame counts as working too: background work (the prerender)
        // reading the card meanwhile made the trim pictures 4 s slow (2026-10-12).
        let editing = self.touched.is_some_and(|at| at.elapsed() < HANDS_ON);
        // A clip waiting for its record is not playing: the background completion of that
        // record waits while a player works, so counting the wait as work kept both waiting
        // for ever (2026-10-12: a new project on the originals never played).
        let playing = self.play_when_ready || self.player_view.preparing || self.player_view.playing();
        let level = match (self.waits_for_record(), playing, editing) {
            (true, _, _) => qnc_playback_activity::PlaybackLevel::Idle,
            (false, true, _) => qnc_playback_activity::PlaybackLevel::Playing,
            (false, false, true) => qnc_playback_activity::PlaybackLevel::Editing,
            (false, false, false) => qnc_playback_activity::PlaybackLevel::Idle,
        };
        self.activity.update_level(self.activity_target.as_ref(), level);
    }

    /// Cuts the current session and clears everything shown.
    pub fn close(&mut self) {
        (self.cue_in_flight, self.cue_next, self.cue_asked, self.cue_waiting) = (None, None, None, None);
        self.play_when_ready = false;
        self.program = false;
        self.levels_sent = None;
        if let Some(player) = &self.player {
            player.close();
        }
        self.player_view = PlayerView::default();
        self.view = PreviewView {
            assets: SourceTimelineAssets::empty(),
            ..PreviewView::default()
        };
        self.report_activity();
    }

    /// Rereads the published filmstrip/wave artifacts for the shown clip, on a thread
    /// of their own; `poll` shows them when they come.
    pub fn refresh_assets(&mut self) -> bool {
        if let Some(clip_id) = self.view.clip_id.clone() {
            self.timeline_assets.request(&clip_id, true);
        }
        false
    }

    /// The filmstrip or wave of the shown clip is still missing (the background worker
    /// makes it): it is read again, once a second, on a thread of its own.
    fn read_missing_assets(&mut self) {
        let Some(clip_id) = self.view.clip_id.clone() else {
            return;
        };
        let assets = &self.view.assets;
        // A coarse wave is read again until the wave of every frame replaces it.
        let complete = assets.clip_id == clip_id
            && assets.filmstrip_background.is_some()
            && assets.wave.as_ref().is_some_and(|wave| !wave.coarse);
        let now = Instant::now();
        if complete || self.timeline_assets.loading() || self.assets_again_at.is_some_and(|at| now < at) {
            return;
        }
        self.assets_again_at = Some(now + Duration::from_secs(1));
        self.timeline_assets.request(&clip_id, true);
    }

    /// Clips gone from the project: their artifacts are not shown again.
    pub fn forget_assets(&mut self, clip_ids: &[String]) {
        self.timeline_assets.remove_clips(clip_ids);
    }

    /// Shows the artifacts read for the clip on screen.
    fn take_assets(&mut self) -> bool {
        let mut changed = false;
        for (assets, _) in self.timeline_assets.take_loaded() {
            if Some(&assets.clip_id) == self.view.clip_id.as_ref() && assets != self.view.assets {
                self.view.assets = assets;
                changed = true;
            }
        }
        changed
    }

    /// Prepares the preview of `clip_id`: cuts the old session first, even when
    /// the new clip cannot be prepared. Returns whether the view changed.
    pub fn open(&mut self, clip_id: &str) -> bool {
        self.open_at(clip_id, 0)
    }

    /// Like [`Self::open`], but the session prepares `first_frame` as its first
    /// picture instead of frame 0. An already open clip is not reopened; while the
    /// player holds a story program (Wrap, Sync) the clip is opened again.
    pub fn open_at(&mut self, clip_id: &str, first_frame: u64) -> bool {
        if !self.program
            && self.view.clip_id.as_deref() == Some(clip_id)
            && self.player_view.error.is_none()
            && (self.player_view.preparing || self.player_view.reply.is_some())
        {
            return false;
        }
        self.prepare_clip(clip_id, first_frame)
    }

    /// The channels of the shown clip heard on output 1 (A1) and 2 (A2) (user rule
    /// 2026-10-01) and their levels (1: as recorded, 0: level 0; user 2026-10-14): the
    /// clip is prepared again on its confirmed frame. In a story program the choice waits
    /// for the clip.
    pub fn hear_channels(&mut self, ((a1, a2), (l1, l2)): ((u16, u16), (f32, f32))) -> bool {
        let Some(clip_id) = self.view.clip_id.clone() else {
            return false;
        };
        let percent = |level: f32| (level.clamp(0.0, 4.0) * 100.0).round() as u16;
        let lead = Some((clip_id.clone(), [a1, a2], [percent(l1), percent(l2)]));
        if self.lead == lead {
            return false;
        }
        let same_channels = self.lead.as_ref().is_some_and(|(id, channels, _)| *id == clip_id && *channels == [a1, a2]);
        self.lead = lead;
        if self.program {
            return false;
        }
        // Only the level changed: the player takes it at once, nothing is prepared again
        // (live 2026-10-16: every level step stopped the sound).
        if same_channels && self.set_output_levels([percent(l1), percent(l2)]) {
            return true;
        }
        // The player decodes every channel; a channel chosen is a route, heard at once,
        // playing or not (user 2026-10-19: "the player plays all channels, the outputs are
        // only the chosen ones"; reopening blanked the monitor and the HDMI output).
        if self.set_output_routes([a1, a2]) && self.set_output_levels([percent(l1), percent(l2)]) {
            return true;
        }
        // No player that takes routes (none open yet): it opens on these channels.
        self.reopen_with_lead(&clip_id)
    }

    /// The shown clip opened again where it is, on the chosen channels, and played.
    fn reopen_with_lead(&mut self, clip_id: &str) -> bool {
        // The frame shown or asked for, not the start: a channel changed again before the
        // player confirmed a position reopens where it was (live 2026-10-16).
        let pending = self.cue_next.or(self.cue_in_flight.map(|(frame, _)| frame)).or(self.cue_asked);
        let opening = self.opening_at.as_ref().filter(|(id, _)| id == clip_id).map(|(_, frame)| *frame);
        let frame = pending.or_else(|| self.player_view.confirmed_source_frame()).or(opening).unwrap_or(0);
        let playing = self.player_view.playing() || self.play_when_ready;
        let prepared = self.prepare_clip(clip_id, frame);
        self.play_when_ready = playing;
        prepared
    }

    fn prepare_clip(&mut self, clip_id: &str, first_frame: u64) -> bool {
        self.close();
        self.opening_at = Some((clip_id.to_string(), first_frame));
        self.view.clip_id = Some(clip_id.to_string());
        if self.lead.as_ref().is_some_and(|(id, ..)| id != clip_id) {
            self.lead = None; // another clip starts on its own channels
        }
        let lead = self.lead.as_ref().map(|(_, lead, levels)| (*lead, *levels));
        // Whoever completes records in the background takes this clip first.
        self.activity.clip(self.activity_target.as_ref(), clip_id);
        let Some(context) = self.context.clone() else {
            self.view.message = qnc_ui_text::text("player.work_settings_project_loaded").into();
            return true;
        };
        // Published artifacts are read again on every choice: they may have
        // appeared since the clip was last shown.
        // Read on a thread of their own (the click never waits on the database or the
        // filmstrip decode); what the cache has shows meanwhile.
        self.view.assets = self
            .timeline_assets
            .request(clip_id, true)
            .unwrap_or_else(|| SourceTimelineAssets::empty_for(clip_id));

        if self.player.is_none() {
            match Player::new() {
                Ok(player) => self.player = Some(player),
                Err(error) => {
                    self.view.message = error;
                    return true;
                }
            }
        }
        let Some(player) = &self.player else {
            return true;
        };
        let clip_id = clip_id.to_string();
        (self.cue_in_flight, self.cue_next, self.cue_asked) = (None, None, None);
        let bed = self.bed_wanted;
        (self.bed_launched, self.bed_playing) = (bed, false);
        player.prepare_at(first_frame, move || {
            let sources = transport_bindings(&context)?;
            let executable = qnc_player_launcher::sibling_executable("qnc-broadcast-player")?;
            let input = InputReader::with_content_reader(
                context.reader.clone(),
                context.player_content.clone(),
            )
            .load(&context.settings.workspace_db_uri, &clip_id)
            .map_err(|error| error.to_string())?;
            let input = match lead {
                Some((lead, levels)) => input
                    .with_lead_channels(&lead)
                    .and_then(|input| input.with_lead_levels(&levels))
                    .map_err(|error| error.to_string())?,
                None => input,
            };
            // The story's A1 under the Sync window: a clip without it still plays (the bed is
            // only what the Sync hears).
            let reader = InputReader::with_content_reader(context.reader.clone(), context.player_content.clone());
            let bed = bed.and_then(|window| {
                let target = qnc_db_broker::ProjectDbTarget::for_project(&context.reader, &context.settings).ok()?;
                let loaded = qnc_program_input::sound_bed_loader(target, context.settings.project_id.clone(), window)(&reader, &context.settings.workspace_db_uri);
                loaded.map_err(|error| qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("sync-bed not built: {error}"))).ok()
            });
            qnc_player_launcher::prepare_launch_with_sync_bed(input, bed, &sources, executable)
        });
        self.apply_player_view(player.view());
        true
    }

    /// A key of the montage (a trim, a marker, a slot) counts as editing for the next
    /// seconds too, as a step does (2026-10-13: the prerender read the card while a
    /// slot edge was moved with the arrows).
    pub fn hands_on(&mut self) {
        self.touched = Some(std::time::Instant::now());
    }

    /// The picture the monitor shows from the story program (not a Sync window), with its
    /// program frame (user 2026-10-13: the trim zoom takes the picture at the cut from
    /// the monitor instead of decoding it again).
    pub fn shown_program_picture(&self) -> Option<ShownPicture> {
        let picture = self.player_view.picture.as_ref()?;
        let header = &picture.header;
        (header.source_id.starts_with("program:") && !header.source_id.contains(":window:")).then(|| ShownPicture {
            frame: header.frame,
            sequence: (header.output_generation, header.sequence),
            size: [header.width as usize, header.height as usize],
            rgba: picture.rgba.clone(),
        })
    }

    /// The picture of a clip on the monitor (not of the story), with its clip and frame.
    pub fn shown_clip_picture(&self) -> Option<(String, ShownPicture)> {
        let picture = self.player_view.picture.as_ref()?;
        let header = &picture.header;
        (!header.source_id.starts_with("program:")).then(|| {
            let sequence = (header.output_generation, header.sequence);
            let size = [header.width as usize, header.height as usize];
            (header.source_id.to_string(), ShownPicture { frame: header.frame, sequence, size, rgba: picture.rgba.clone() })
        })
    }

    /// Pauses a playing player; false when it does not play.
    pub fn pause(&mut self) -> bool {
        self.play_when_ready = false;
        let send = self.player_view.playing() && !self.pausing;
        self.pausing |= send;
        send && self.send(Action::TogglePlayPause)
    }

    pub fn toggle_play(&mut self) -> bool {
        self.touched = Some(std::time::Instant::now());
        self.cue_asked = None; // Play moves the playhead: arrows count from where it stops
        self.pausing = false;
        let Some(player) = &self.player else {
            self.view.message = qnc_ui_text::text("player.broadcast_player_connected").into();
            return true;
        };
        let playback = player.view();
        // A clip whose record is still completed plays as soon as it is (user 2026-10-18:
        // Space on such a clip was refused).
        if self.waits_for_record() || playback.error.as_deref().is_some_and(qnc_player_input::is_incomplete_media) {
            self.play_when_ready = true;
            return true;
        }
        if !playback.playing() && !playback.can_start_playback() && playback.error.is_none() {
            // Not ready yet: start as soon as the prepared session is.
            self.play_when_ready = true;
            return true;
        }
        self.send(Action::TogglePlayPause)
    }

    /// One frame per arrow press, counted from the frame last asked for, so presses
    /// made while the player still prepares the previous one are not lost; the
    /// latest frame goes to the player (as a scrub does).
    pub fn step(&mut self, frames: i64) -> bool {
        let timeline = &self.view.timeline;
        let pending = self.cue_next.or(self.cue_in_flight.map(|(frame, _)| frame)).or(self.cue_asked);
        let base = pending.or_else(|| self.player_view.confirmed_source_frame());
        let (Some(base), true) = (base, timeline.duration_frames > 1) else {
            return self.send(Action::Step(frames));
        };
        let last = timeline.range_start_frame + timeline.duration_frames - 1;
        self.cue(base.saturating_add_signed(frames).clamp(timeline.range_start_frame, last))
    }

    pub fn cue(&mut self, frame: u64) -> bool {
        self.touched = Some(std::time::Instant::now());
        // A player still being prepared takes the latest frame once it is ready.
        if self.player.is_some() && !self.player_view.has_confirmed_position() && self.player_view.error.is_none() {
            self.cue_waiting = Some(frame);
            return true;
        }
        if self.cue_in_flight.is_some() {
            self.cue_next = Some(frame);
            return true;
        }
        self.cue_in_flight = Some((frame, std::time::Instant::now()));
        self.cue_asked = Some(frame);
        self.send(Action::Cue(frame))
    }

    /// The cue in flight landed (or waited too long): the latest asked frame goes.
    fn release_cue(&mut self) {
        let Some((frame, sent)) = self.cue_in_flight else {
            return;
        };
        let landed = self.player_view.confirmed_source_frame() == Some(frame);
        if landed {
            self.cue_asked = None;
        }
        if landed || sent.elapsed() >= std::time::Duration::from_millis(300) {
            self.cue_in_flight = None;
            if let Some(next) = self.cue_next.take() {
                self.cue(next);
            }
        }
    }

    /// Handles the intent of a passive timeline. While a program plays, the
    /// source timeline brings the chosen clip back at the pointed frame (v5).
    pub fn timeline_intent(&mut self, intent: &TimelineIntent) -> bool {
        match intent {
            TimelineIntent::CueFrame(frame) if self.program => match self.view.clip_id.clone() {
                Some(clip_id) => self.open_at(&clip_id, *frame),
                None => false,
            },
            TimelineIntent::CueFrame(frame) => self.cue(*frame),
            TimelineIntent::GoHome => self.timeline_intent(&TimelineIntent::CueFrame(self.view.timeline.range_start_frame)),
            _ => false,
        }
    }

    /// Opens a story program in the same player and monitor, at a program frame.
    /// The source clip, its timeline and artifacts stay shown (v5 Wrap).
    fn open_program(
        &mut self,
        first_frame: u64,
        build: impl FnOnce(&InputReader, &str) -> Result<ProgramInput, String> + Send + 'static,
    ) -> bool {
        let Some(context) = self.context.clone() else {
            self.view.message = qnc_ui_text::text("player.work_settings_project_loaded").into();
            return true;
        };
        if self.player.is_none() {
            match Player::new() {
                Ok(player) => self.player = Some(player),
                Err(error) => {
                    self.view.message = error;
                    return true;
                }
            }
        }
        let Some(player) = &self.player else {
            return true;
        };
        self.program = true;
        self.cue_waiting = None;
        self.play_when_ready = false;
        (self.program_levels, self.levels_sent) = ([100, 100], None);
        (self.cue_in_flight, self.cue_next, self.cue_asked) = (None, None, None);
        player.prepare_at(first_frame, program_launch(context, build));
        let view = player.view();
        self.apply_player_view(view);
        true
    }

    /// A frame of the active project's story program (Wrap): a cue while the
    /// program already plays, else (or when `reopen`: the story changed) the
    /// program is built and opened at that frame.
    pub fn show_program_frame(&mut self, frame: u64, reopen: bool) -> bool {
        if !reopen && self.program && self.player_view.has_confirmed_position() {
            return self.cue(frame);
        }
        if !reopen && self.program && self.player.is_some() && self.player_view.error.is_none() {
            self.cue_waiting = Some(frame); // still being prepared: the latest frame waits
            return true;
        }
        let Some(context) = &self.context else {
            return true;
        };
        let project_id = context.settings.project_id.clone();
        self.open_story(frame, move |target| qnc_program_input::loader(target, project_id))
    }
    /// The sound of a segment or cover drafted on the Wrap row is heard at once (user
    /// 2026-10-16). Its level or off against the stored sound (`stored`, the same part)
    /// is the level of its output A1 or A2, taken by the player at once; another channel,
    /// or a part stored off, opens the program again with the draft at the frame it shows
    /// (`frame` when it shows none), playing on when it played. `None` hears the stored
    /// sound again. Nothing is written. False when nothing changed or no program plays.
    pub fn hear_program_draft(&mut self, draft: Option<SoundDraft>, stored: Option<SoundDraft>, frame: u64) -> bool {
        if self.program_draft == draft {
            return false;
        }
        self.program_draft = draft.clone();
        if !self.program {
            return false;
        }
        if !self.program_draft_reopened {
            let level = draft_output_level(draft.as_ref(), stored.as_ref());
            if level.is_some_and(|level| self.set_output_levels(level)) {
                return true;
            }
        }
        self.program_draft_reopened = draft.is_some();
        let Some(context) = &self.context else {
            return false;
        };
        let project_id = context.settings.project_id.clone();
        let pending = self.cue_next.or(self.cue_in_flight.map(|(frame, _)| frame)).or(self.cue_asked);
        let frame = pending.or_else(|| self.program_frame()).unwrap_or(frame);
        let playing = self.player_view.playing() || self.play_when_ready;
        match draft {
            Some(draft) => self.open_story(frame, move |target| qnc_program_input::draft_loader(target, project_id, draft)),
            None => self.open_story(frame, move |target| qnc_program_input::loader(target, project_id)),
        };
        self.play_when_ready = playing;
        true
    }

    /// One part of the playing program (frames `[from, to)` on output `output`) heard
    /// `gain_db` louder than stored, or silent, at once in the player (user 2026-10-22: the
    /// level of one cover, nothing is prepared again); `None` clears it. False without a
    /// program player.
    pub fn hear_part_level(&mut self, part: Option<(u16, u64, u64, i16, bool)>) -> bool {
        if !self.program {
            return false;
        }
        let (output, from, to, gain_db, off) = part.unwrap_or((0, 0, 0, 0, false));
        self.player.as_ref().is_some_and(|player| player.send(Action::SetPartLevel(output, from, to, gain_db, off)).is_ok())
    }

    /// The program plays a drafted story (opened again with a draft), not the stored one.
    pub fn program_drafted(&self) -> bool {
        self.program_draft.is_some()
    }

    /// The draft was written (Enter): the stored story now holds it, the program opens
    /// again from it as after every write.
    pub fn forget_program_draft(&mut self) {
        self.program_draft = None;
        self.program_draft_reopened = false;
    }

    /// The story program of the active project, its database (the one intermediary of
    /// the project database, `qnc-program-db`) found where the player is prepared: on
    /// the form thread reading the registry and `project.db` stood it up to 0.7 s while
    /// another connection wrote (live 2026-10-12).
    fn open_story<L>(
        &mut self,
        first_frame: u64,
        loader: impl FnOnce(qnc_db_broker::ProjectDbTarget) -> L + Send + 'static,
    ) -> bool
    where
        L: FnOnce(&InputReader, &str) -> Result<ProgramInput, String>,
    {
        let Some(context) = self.context.clone() else {
            return true;
        };
        self.open_program(first_frame, move |reader, workspace_db_uri| {
            let target = qnc_db_broker::ProjectDbTarget::for_project(&context.reader, &context.settings)?;
            loader(target)(reader, workspace_db_uri)
        })
    }

    /// The source timeline of a Sync play (v5 `set_source_playhead_frame`): the
    /// source playhead moves with the program, a closed slot shows its IN/OUT.
    pub fn with_sync_source(
        mut timeline: TimelineProjection,
        (frame, marks): (u64, Option<(u64, u64)>),
    ) -> TimelineProjection {
        timeline.playhead_frame = Some(frame.min(timeline.duration_frames));
        if let Some((in_frame, out_frame)) = marks {
            timeline.source_in_frame = Some(in_frame);
            timeline.source_out_frame = Some(out_frame);
        }
        timeline
    }

    /// The Sync window whose story A1 the clip session carries (user 2026-10-20: the story's
    /// A1 under a Sync, in the clip's own clock and device; IN opens no player). A clip shown
    /// and not playing opens again with it where it stands; one playing keeps its session.
    pub fn want_sync_bed(&mut self, window: Option<(u64, u64)>) {
        if self.bed_wanted == window {
            return;
        }
        self.bed_wanted = window;
        let open_clip = self.view.clip_id.clone().filter(|_| !self.program && self.player.is_some());
        let idle = !self.player_view.playing() && !self.play_when_ready && !self.player_view.preparing;
        if let Some(clip_id) = open_clip.filter(|_| idle && window.is_some() && self.bed_launched != window) {
            self.reopen_with_lead(&clip_id);
        }
    }

    /// The story's A1 of the Sync window plays from the sample of clip frame `source_frame`
    /// on, from the window start `window.0` (sample exact, in the clip's clock); false when the
    /// session does not carry that window.
    pub fn play_sync_bed(&mut self, window: (u64, u64), source_frame: u64) -> bool {
        let Some((start, _)) = self.bed_launched.filter(|(start, end)| *start <= window.0 && window.0 < *end) else {
            return false;
        };
        let sent = self.player.as_ref().is_some_and(|player| player.send(Action::StartSyncBed(source_frame, window.0 - start)).is_ok());
        self.bed_playing |= sent;
        sent
    }

    /// The story's A1 stops at once; the clip's own A1 is heard again.
    pub fn stop_sync_bed(&mut self) {
        if std::mem::take(&mut self.bed_playing) {
            if let Some(player) = &self.player {
                let _ = player.send(Action::StopSyncBed);
            }
        }
    }

    /// The clip plays from `frame`: at once when it stands there, else once the cue to it
    /// landed (a Play sent behind a cue in flight is refused as not ready).
    pub fn play_from(&mut self, frame: u64) -> bool {
        if self.player_view.confirmed_source_frame() == Some(frame) && self.cue_in_flight.is_none() {
            return self.toggle_play();
        }
        self.cue(frame);
        self.play_when_ready = true;
        true
    }

    /// The clip frame the player confirmed, while it plays the clip (not a program).
    pub fn clip_frame(&self) -> Option<u64> {
        (!self.program).then(|| self.player_view.confirmed_source_frame()).flatten()
    }

    /// Whether the player plays a program rather than the source clip.
    pub fn in_program(&self) -> bool {
        self.program
    }

    /// The program frame the player confirmed, while a program plays.
    pub fn program_frame(&self) -> Option<u64> {
        self.program
            .then(|| self.player_view.confirmed_source_frame())
            .flatten()
    }

    /// The source channel outputs 1 and 2 play, heard at once; false when the player cannot
    /// take it now (not prepared yet).
    fn set_output_routes(&mut self, channels: [u16; 2]) -> bool {
        self.player.as_ref().is_some_and(|player| player.send(Action::SetOutputRoutes(channels)).is_ok())
    }

    /// The level of outputs 1 and 2 in percent, heard at once; false when the player cannot
    /// take it now. A program's level is normalised on top while normalising is on.
    fn set_output_levels(&mut self, levels: [u16; 2]) -> bool {
        let levels = match self.program {
            true => {
                self.program_levels = levels;
                self.normalized(levels)
            }
            false => levels,
        };
        let sent = self.player.as_ref().is_some_and(|player| player.send(Action::SetOutputLevels(levels)).is_ok());
        self.levels_sent = sent.then_some(levels).or(self.levels_sent);
        sent
    }

    /// `levels` with the normalising gains on them, within what the output holds.
    fn normalized(&self, levels: [u16; 2]) -> [u16; 2] {
        let gains = self.normalize.unwrap_or_default();
        let level = |percent: u16, db: f64| (f64::from(percent) * 10f64.powf(db / 20.0)).round().clamp(0.0, 400.0) as u16;
        [level(levels[0], gains[0]), level(levels[1], gains[1])]
    }

    /// The program's A1 and A2 brought to the project's loudness targets (user 2026-10-24)
    /// from their measured loudness (`None`: as edited); a side not measured keeps its level.
    /// Heard at once while a program plays. False when nothing changed.
    pub fn normalize_program(&mut self, measured: Option<[Option<f64>; 2]>) -> bool {
        let targets = self.context.as_ref().and_then(|context| context.settings.audio_loudness().ok());
        let gains = measured.zip(targets).map(|(measured, targets)| {
            let gain = |target: f64, measured: Option<f64>| {
                measured.map_or(0.0, |lufs| (target - lufs).clamp(-NORMALIZE_MAX_DB, NORMALIZE_MAX_DB))
            };
            [gain(targets.a1_lufs, measured[0]), gain(targets.a2_lufs, measured[1])]
        });
        if self.normalize == gains {
            return false;
        }
        self.normalize = gains;
        self.send_program_levels()
    }

    /// The program's output levels to the player when it has not got them yet.
    fn send_program_levels(&mut self) -> bool {
        if !self.program || !self.player_view.has_confirmed_position() {
            return false;
        }
        let levels = self.normalized(self.program_levels);
        if self.levels_sent == Some(levels) {
            return false;
        }
        let sent = self.player.as_ref().is_some_and(|player| player.send(Action::SetOutputLevels(levels)).is_ok());
        if sent {
            self.levels_sent = Some(levels);
        }
        sent
    }

    fn send(&mut self, action: Action) -> bool {
        let result = self
            .player
            .as_ref()
            .ok_or_else(|| qnc_ui_text::text("player.broadcast_player_connected").to_string())
            .and_then(|player| player.send(action));
        match result {
            Ok(()) => self.play_when_ready = false,
            Err(error) => self.view.message = error,
        }
        true
    }

    /// A clip whose saved record is still completed in the background opens again
    /// once a second, for up to three minutes (v5 waits for the media probe).
    fn retry_incomplete_record(&mut self) -> bool {
        // A new attempt that is still preparing keeps the wait: the monitor stays on the
        // same message instead of flashing between attempts.
        let incomplete = self
            .player_view
            .error
            .as_deref()
            .is_some_and(qnc_player_input::is_incomplete_media)
            || (self.player_view.preparing && self.waits_for_record());
        let Some(clip_id) = self.view.clip_id.clone().filter(|_| incomplete) else {
            if self.record_wait.take().is_some() && self.view.message == record_wait_text() {
                self.view.message.clear(); // the record is complete: the wait is over
            }
            return false;
        };
        let now = std::time::Instant::now();
        let (since, last) = match &self.record_wait {
            Some((id, since, last)) if *id == clip_id => (*since, *last),
            _ => (now, now),
        };
        // The worker that completes the record is woken at once and every few seconds while
        // the clip waits (user 2026-10-18: nothing woke it, the clip never played).
        let new_wait = since == now;
        if new_wait || now.duration_since(last) >= WAKE_AGAIN {
            if let Some(RecordWaker(wake)) = self.record_waker.clone() {
                let _ = std::thread::Builder::new().name("qnc-record-wake".into()).spawn(move || wake());
            }
        }
        self.record_wait = Some((clip_id.clone(), since, last));
        self.view.message = record_wait_text().into();
        if now.duration_since(since) > std::time::Duration::from_secs(180)
            || now.duration_since(last) < RECORD_RETRY
        {
            return false;
        }
        self.record_wait = Some((clip_id.clone(), since, now));
        self.open_at(&clip_id, 0)
    }

    /// What wakes the background worker when a shown clip waits for its record.
    fn set_record_waker(&mut self, waker: RecordWaker) {
        self.record_waker = Some(waker);
    }

    /// The background worker of the active project of the QNC `root` (`qnc-worker-wake`)
    /// is woken when a shown clip waits for its record.
    pub fn wake_worker_from(&mut self, root: &std::path::Path) {
        let root = root.to_path_buf();
        self.set_record_waker(RecordWaker::new(move || _ = qnc_worker_wake::wake_active(&root)));
    }

    /// The shown clip is waiting for its record to be completed in the background.
    fn waits_for_record(&self) -> bool {
        matches!((&self.record_wait, &self.view.clip_id), (Some((waiting, ..)), Some(shown)) if waiting == shown)
    }

    /// Applies the player state. Returns whether the view changed.
    pub fn poll(&mut self) -> bool {
        self.poll_pointed(None)
    }

    /// The same, with the clip the media pool points at (mouse over its card, or the
    /// keyboard on it): its filmstrip and wave are read into memory now, so a click on it
    /// shows them at once (user 2026-10-20).
    pub fn poll_pointed(&mut self, pointed: Option<&str>) -> bool {
        self.pausing &= self.player_view.playing();
        if let Some(clip_id) = self.pointed.new_clip(pointed).filter(|clip| self.view.clip_id.as_deref() != Some(*clip)) {
            self.timeline_assets.request(clip_id, false);
        }
        let mut changed = self.retry_incomplete_record();
        changed |= self.take_assets();
        self.read_missing_assets();
        if let Some(player) = &self.player {
            let playback = player.view();
            if playback != self.player_view {
                if let Some(error) = &playback.error {
                    if !self.waits_for_record() && !qnc_player_input::is_incomplete_media(error) {
                        self.view.message = error.clone();
                        self.play_when_ready = false;
                    }
                }
                self.apply_player_view(playback);
                changed = true;
            }
        }
        // The asked frame is done once the player shows it or plays on from anywhere.
        if self.player_view.playing() || self.player_view.confirmed_source_frame() == self.cue_asked {
            self.cue_asked = None;
        }
        self.release_cue();
        // A program opened (again) gets its normalised levels once the player is ready.
        changed |= self.send_program_levels();
        if self.player_view.has_confirmed_position() {
            if let Some(frame) = self.cue_waiting.take() {
                self.cue(frame);
            }
        }
        if self.play_when_ready {
            let waiting = self.player_view.error.as_deref().is_some_and(qnc_player_input::is_incomplete_media);
            if self.player_view.error.is_some() && !waiting {
                self.play_when_ready = false;
            } else if self.player_view.can_start_playback() && !self.player_view.playing() && self.cue_in_flight.is_none() {
                // A cue still in flight first lands (live 2026-10-20: Play sent behind it was
                // refused as not ready).
                if let Some(player) = &self.player {
                    match player.send(Action::TogglePlayPause) {
                        Ok(()) => self.play_when_ready = false,
                        Err(error) if error == qnc_player_client::PREPARING => {}
                        Err(error) => {
                            self.play_when_ready = false;
                            self.view.message = error;
                        }
                    }
                    changed = true;
                }
            } else if self.player_view.playing() {
                self.play_when_ready = false;
            }
        }
        self.report_activity();
        changed
    }

    fn apply_player_view(&mut self, playback: PlayerView) {
        self.view.video_visible = playback.video_visible;
        // While the record of the shown clip is completed, one steady message, no flashing.
        let waiting = self.waits_for_record() || playback.error.as_deref().is_some_and(qnc_player_input::is_incomplete_media);
        self.view.monitor_message = if waiting { Some(record_wait_text().into()) } else { playback.error.clone() };
        self.view.monitor_frame = playback.picture.as_ref().map(|picture| MonitorFrame {
            session_id: picture.header.session_id.to_string(),
            generation: picture.header.output_generation,
            sequence: picture.header.sequence,
            width: picture.header.width as usize,
            height: picture.header.height as usize,
            rgba: picture.rgba.clone(),
            frame_map: Some(picture.frame_map.clone()),
        });
        // A program keeps the source timeline of the chosen clip (v5 Wrap).
        if !self.program {
            self.view.timeline =
                qnc_player_timeline::projection_from_player_reply(playback.reply.as_ref());
        }
        self.view.playing = playback.playing();
        self.view.output_levels = [playback.output_level_dbfs(1), playback.output_level_dbfs(2)];
        self.player_view = playback;
    }
}

impl Drop for SourcePreview {
    fn drop(&mut self) {
        self.close();
        self.player = None;
    }
}

#[cfg(test)]
mod tests {
    /// Normalising multiplies the program's output level by the gain to the target (user
    /// 2026-10-24), within what the output holds.
    #[test]
    fn a_normalising_gain_is_on_top_of_the_program_level_within_the_output() {
        let mut preview = super::SourcePreview::new();
        preview.normalize = Some([6.0, -6.0]);
        assert_eq!(preview.normalized([100, 100]), [200, 50]);
        preview.normalize = Some([12.0, 0.0]);
        assert_eq!(preview.normalized([200, 100]), [400, 100], "the output holds at most 400 %");
        preview.normalize = None;
        assert_eq!(preview.normalized([100, 80]), [100, 80]);
    }

    #[test]
    fn a_drafted_level_opens_the_program_with_the_draft_never_the_whole_output() {
        let sound = |level_db: i8| SoundDraft { lane: 0, id: "s".into(), channel: 0, level_db, off: false, overlaps: Vec::new(), picture_lead: None };
        assert_eq!(draft_output_level(Some(&sound(-6)), Some(&sound(-12))), None, "never the whole output: only this part's sound (live 2026-10-22)");
        assert_eq!(draft_output_level(Some(&sound(0)), Some(&sound(-20))), None, "1000 % is not: the draft itself is heard");
        assert_eq!(draft_output_level(None, Some(&sound(-20))), Some([100, 100]), "the stored sound again");
    }

    use super::*;

    #[test]
    fn arrow_presses_add_up_from_the_frame_last_asked_for() {
        let mut preview = SourcePreview::new();
        preview.view.timeline.duration_frames = 100;
        preview.cue_in_flight = Some((50, std::time::Instant::now()));
        preview.step(-1);
        preview.step(-1);
        assert_eq!(preview.cue_next, Some(48), "two presses, two frames, while 50 is prepared");
        preview.step(1);
        assert_eq!(preview.cue_next, Some(49));
        preview.cue_next = Some(0);
        preview.step(-1);
        assert_eq!(preview.cue_next, Some(0), "never before the clip");
        preview.cue_next = None;
        preview.cue_in_flight = None;
        preview.toggle_play();
        assert_eq!(preview.cue_asked, None, "after Play the arrows count from where it stops");
    }

    #[test]
    fn starts_passive_and_empty() {
        let preview = SourcePreview::new();
        assert!(preview.view().clip_id.is_none());
        assert!(!preview.has_player());
        assert!(preview.next_repaint_delay().is_none());
    }

    #[test]
    fn opening_without_a_project_keeps_the_choice_and_reports_it() {
        let mut preview = SourcePreview::new();
        assert!(preview.open("clip-a"));
        assert_eq!(preview.view().clip_id.as_deref(), Some("clip-a"));
        assert_eq!(
            preview.view().message,
            qnc_ui_text::text("player.work_settings_project_loaded")
        );
        assert!(!preview.has_player());
    }

    #[test]
    fn transport_without_a_player_says_so() {
        let mut preview = SourcePreview::new();
        assert!(preview.toggle_play());
        assert_eq!(preview.view().message, qnc_ui_text::text("player.broadcast_player_connected"));
        for change in [preview.step(1), preview.step(-1), preview.cue(10)] {
            assert!(change);
        }
        assert_eq!(preview.view().message, qnc_ui_text::text("player.broadcast_player_connected"));
    }

    #[test]
    fn only_a_cue_timeline_intent_is_acted_on() {
        let mut preview = SourcePreview::new();
        assert!(!preview.timeline_intent(&TimelineIntent::None));
        assert!(preview.timeline_intent(&TimelineIntent::CueFrame(5)));
    }

    #[test]
    fn close_clears_everything_shown() {
        let mut preview = SourcePreview::new();
        preview.open("clip-a");
        preview.close();
        assert!(preview.view().clip_id.is_none());
        assert!(preview.view().message.is_empty());
    }
}

impl std::fmt::Debug for SourcePreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourcePreview")
            .field("clip_id", &self.view.clip_id)
            .field("has_player", &self.player.is_some())
            .field("play_when_ready", &self.play_when_ready)
            .finish()
    }
}

/// Transport bindings for the player launch: the sources of the host, and the project
/// folder of this machine as one more source, where media the import copied is read.
/// The launch of a story program, prepared off the form thread: the program built by
/// `build`, launched with the transport of this machine.
fn program_launch(
    context: PreviewContext,
    build: impl FnOnce(&InputReader, &str) -> Result<ProgramInput, String> + Send + 'static,
) -> impl FnOnce() -> Result<qnc_player_client::Launch, String> + Send + 'static {
    move || {
        let sources = transport_bindings(&context)?;
        let executable = qnc_player_launcher::sibling_executable("qnc-broadcast-player")?;
        let reader = InputReader::with_content_reader(context.reader.clone(), context.player_content.clone());
        let program = build(&reader, &context.settings.workspace_db_uri)?;
        qnc_player_launcher::prepare_program_launch(program, &sources, executable)
    }
}

fn transport_bindings(context: &PreviewContext) -> Result<Vec<SourceTransportBinding>, String> {
    // Without it only copied media fails (no binding), never media on a card.
    let project = context
        .reader
        .local_workspace_dir(&context.settings)
        .ok()
        .flatten()
        .zip(context.settings.project_media_source_uri().ok())
        .map(|(dir, uri)| SourceTransportBinding::local(uri, dir));
    let mut bindings = context
        .sources
        .iter()
        .map(|binding| {
            if let Some(root) = &binding.file {
                return Ok(SourceTransportBinding::local(
                    binding.uri.clone(),
                    root.clone(),
                ));
            }
            SourceTransportBinding::network(
                binding.uri.clone(),
                binding.endpoint.clone().ok_or("Source endpoint missing.")?,
                binding.token()?.ok_or("Source credential missing.")?,
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    bindings.extend(project);
    Ok(bindings)
}

/// What the preview shows while the record of the clip is completed in the background.
/// While a clip's record is completed the preview looks like any player getting ready
/// (user 2026-10-18: a separate waiting message is not acceptable; Play is kept and goes
/// as soon as the clip is ready).
fn record_wait_text() -> &'static str {
    qnc_ui_text::text("player.player_getting_ready")
}
