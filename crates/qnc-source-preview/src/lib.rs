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
pub use still_media::{LocatedPicture, StillLocator};

use std::{sync::Arc, time::Duration};

use qnc_content_read::ContentReader;
use qnc_player_client::{Action, Player, View as PlayerView};
use qnc_player_input::{InputReader, PlayerContentRead, ProgramInput};
use qnc_player_launcher::SourceTransportBinding;
pub use qnc_program_input::{SoundDraft, TransientCover};
use qnc_source_bindings::{SourceBinding, TransportBindings};
use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_timeline_assets::{
    SourceTimelineAssets, TimelineArtifactRead, TimelineAssetContext, TimelineAssetReader,
};
use qnc_work_settings::{ProductArea, SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.source-preview";
pub const VERSION: &str = "0.1.0";

/// Confirmed monitor picture (RGBA), as delivered by the player.
#[derive(Debug, Clone)]
pub struct MonitorFrame {
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub width: usize,
    pub height: usize,
    pub rgba: Arc<[u8]>,
    /// Where the player writes the pictures of this session (a program output on an
    /// external screen reads the same map at its own refresh).
    pub frame_map: Option<Arc<std::path::Path>>,
}

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
    /// Monitor Auto (user 2026-10-08, the default): while the program is on an external
    /// screen the monitor leaves the picture to it; Monitor On keeps showing it.
    pub monitor_auto: bool,
    /// The level (dBFS) of A1 and A2 at the player's output, for a meter (user 2026-10-16).
    pub output_levels: [Option<f32>; 2],
}

impl PreviewView {
    /// The switch Auto | On under the monitor: which one is on.
    pub fn monitor_mode_on(&self, action_id: &str) -> bool {
        match action_id {
            MONITOR_AUTO => self.monitor_auto,
            MONITOR_ON => !self.monitor_auto,
            _ => false,
        }
    }
}

pub const MONITOR_AUTO: &str = "monitor_auto";
pub const MONITOR_ON: &str = "monitor_on";

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
            monitor_auto: true,
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
        Self {
            reader,
            settings,
            sources,
            player_content,
            artifacts,
            notice: None,
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
        let player_content = Arc::new(content::PlayerContent {
            target: qnc_content_store::ContentTarget::for_project(&reader, &settings),
        });
        Self {
            reader,
            settings,
            sources: bindings.sources,
            player_content,
            artifacts,
            notice,
        }
    }

    pub fn project_id(&self) -> &str {
        &self.settings.project_id
    }
}

/// How long after a frame step, cue or Play the user still counts as working.
const HANDS_ON: std::time::Duration = std::time::Duration::from_secs(3);

pub struct SourcePreview {
    view: PreviewView,
    context: Option<PreviewContext>,
    player: Option<Player>,
    player_view: PlayerView,
    timeline_assets: TimelineAssetReader,
    play_when_ready: bool,
    /// The last frame step, cue or Play of the user: the machine is theirs a while after.
    touched: Option<std::time::Instant>,
    /// The clip whose record is still completed, since when and the last try.
    record_wait: Option<(String, std::time::Instant, std::time::Instant)>,
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
    /// The program was opened again with the draft (another channel): Escape opens it
    /// again from the story, a level no longer goes to the output.
    program_draft_reopened: bool,
}

impl Default for SourcePreview {
    fn default() -> Self {
        Self {
            view: PreviewView::default(),
            context: None,
            player: None,
            player_view: PlayerView::default(),
            timeline_assets: TimelineAssetReader::default(),
            play_when_ready: false,
            touched: None,
            record_wait: None,
            cue_in_flight: None,
            cue_next: None,
            cue_asked: None,
            opening_at: None,
            activity: qnc_playback_activity::PlaybackReporter::new(),
            activity_target: None,
            program: false,
            lead: None,
            program_draft: None,
            program_draft_reopened: false,
        }
    }
}

impl SourcePreview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Monitor Auto or On (the switch under the monitor); false for any other action.
    pub fn set_monitor_mode(&mut self, action_id: &str) -> bool {
        let auto = match action_id {
            MONITOR_AUTO => true,
            MONITOR_ON => false,
            _ => return false,
        };
        self.view.monitor_auto = auto;
        true
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
        self.activity_target =
            qnc_content_store::ContentTarget::for_project(&context.reader, &context.settings).ok();
        self.context = Some(context);
    }

    fn report_activity(&mut self) {
        // Editing frame by frame counts as working too: background work (the prerender)
        // reading the card meanwhile made the trim pictures 4 s slow (2026-10-12).
        let editing = self.touched.is_some_and(|at| at.elapsed() < HANDS_ON);
        // A clip waiting for its record is not playing: the background completion of that
        // record waits while a player works, so counting the wait as work kept both waiting
        // for ever (2026-10-12: a new project on the originals never played).
        let works = !self.waits_for_record()
            && (self.play_when_ready || self.player_view.preparing || self.player_view.playing() || editing);
        self.activity.update(self.activity_target.as_ref(), works);
    }

    /// Cuts the current session and clears everything shown.
    pub fn close(&mut self) {
        (self.cue_in_flight, self.cue_next, self.cue_asked) = (None, None, None);
        self.play_when_ready = false;
        self.program = false;
        if let Some(player) = &self.player {
            player.close();
        }
        self.player_view = PlayerView::default();
        self.view = PreviewView {
            assets: SourceTimelineAssets::empty(),
            monitor_auto: self.view.monitor_auto, // a choice of this laptop, not of a clip
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
        // The frame shown or asked for, not the start: a channel changed again before the
        // player confirmed a position reopens where it was (live 2026-10-16).
        let pending = self.cue_next.or(self.cue_in_flight.map(|(frame, _)| frame)).or(self.cue_asked);
        let opening = self.opening_at.as_ref().filter(|(id, _)| *id == clip_id).map(|(_, frame)| *frame);
        let frame = pending.or_else(|| self.player_view.confirmed_source_frame()).or(opening).unwrap_or(0);
        // A channel picked while playing is heard playing on (user 2026-10-14: the draft
        // channel is played before Enter keeps it).
        let playing = self.player_view.playing() || self.play_when_ready;
        let prepared = self.prepare_clip(&clip_id, frame);
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
            qnc_player_launcher::prepare_launch(input, &sources, executable)
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

    pub fn toggle_play(&mut self) -> bool {
        self.touched = Some(std::time::Instant::now());
        self.cue_asked = None; // Play moves the playhead: arrows count from where it stops
        let Some(player) = &self.player else {
            self.view.message = qnc_ui_text::text("player.broadcast_player_connected").into();
            return true;
        };
        let playback = player.view();
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
    pub fn open_program(
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
        self.play_when_ready = false;
        (self.cue_in_flight, self.cue_next, self.cue_asked) = (None, None, None);
        player.prepare_at(first_frame, move || {
            let sources = transport_bindings(&context)?;
            let executable = qnc_player_launcher::sibling_executable("qnc-broadcast-player")?;
            let reader = InputReader::with_content_reader(
                context.reader.clone(),
                context.player_content.clone(),
            );
            let program = build(&reader, &context.settings.workspace_db_uri)?;
            qnc_player_launcher::prepare_program_launch(program, &sources, executable)
        });
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
            let level = match (&draft, &stored) {
                (None, _) => Some([100, 100]),
                (Some(draft), Some(stored)) if draft.channel == stored.channel && !stored.off => {
                    let gain = if draft.off { 0.0 } else { 10f32.powf(f32::from(draft.level_db - stored.level_db) / 20.0) };
                    let percent = (gain * 100.0).round().clamp(0.0, 400.0) as u16;
                    Some(if draft.lane == 0 { [percent, 100] } else { [100, percent] })
                }
                _ => None,
            };
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

    /// The draft was written (Enter): the stored story now holds it, the program opens
    /// again from it as after every write.
    pub fn forget_program_draft(&mut self) {
        self.program_draft = None;
        self.program_draft_reopened = false;
    }

    /// Sync/B-roll (v5 `PlayProgram` of the Sync preview): the program window
    /// `[in, out)` with the source from its IN over it, opened at its first frame
    /// and played as soon as it is ready. Its frames start at 0.
    pub fn open_program_window(&mut self, window: (u64, u64), cover: TransientCover) -> bool {
        let Some(context) = &self.context else {
            return true;
        };
        let project_id = context.settings.project_id.clone();
        self.open_story(0, move |target| qnc_program_input::window_loader(target, project_id, window, cover));
        self.play_when_ready = true;
        true
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

    /// The level of outputs 1 and 2 in percent, heard at once; false when the player
    /// cannot take it now (not prepared yet).
    fn set_output_levels(&mut self, levels: [u16; 2]) -> bool {
        self.player.as_ref().is_some_and(|player| player.send(Action::SetOutputLevels(levels)).is_ok())
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
            if self.record_wait.take().is_some() && self.view.message == RECORD_WAIT {
                self.view.message.clear(); // the record is complete: the wait is over
            }
            return false;
        };
        let now = std::time::Instant::now();
        let (since, last) = match &self.record_wait {
            Some((id, since, last)) if *id == clip_id => (*since, *last),
            _ => (now, now),
        };
        self.record_wait = Some((clip_id.clone(), since, last));
        self.view.message = RECORD_WAIT.into();
        if now.duration_since(since) > std::time::Duration::from_secs(180)
            || now.duration_since(last) < std::time::Duration::from_secs(1)
        {
            return false;
        }
        self.record_wait = Some((clip_id.clone(), since, now));
        self.open_at(&clip_id, 0)
    }

    /// The shown clip is waiting for its record to be completed in the background.
    fn waits_for_record(&self) -> bool {
        matches!((&self.record_wait, &self.view.clip_id), (Some((waiting, ..)), Some(shown)) if waiting == shown)
    }

    /// Applies the player state. Returns whether the view changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.retry_incomplete_record();
        changed |= self.take_assets();
        if let Some(player) = &self.player {
            let playback = player.view();
            if playback != self.player_view {
                if let Some(error) = &playback.error {
                    if !self.waits_for_record() && !qnc_player_input::is_incomplete_media(error) {
                        self.view.message = error.clone();
                    }
                    self.play_when_ready = false;
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
        if self.play_when_ready {
            if self.player_view.error.is_some() {
                self.play_when_ready = false;
            } else if self.player_view.can_start_playback() && !self.player_view.playing() {
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
        self.view.monitor_message = if waiting { Some(RECORD_WAIT.into()) } else { playback.error.clone() };
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
    use super::*;

    /// Diagnostics on an isolated copy only (never the project itself): prints the
    /// sources of every item of a Sync window. QNC_DIAG_ROOT=<copy root>
    /// QNC_DIAG_CLIP=<cover clip> QNC_DIAG_IN=<source in> QNC_DIAG_END=<window end>.
    #[test]
    #[ignore = "reads an isolated copy of a real project"]
    fn prints_the_items_of_a_sync_window() {
        let var = |name| std::env::var(name).unwrap();
        let reader = SettingsReader::from_root(std::path::Path::new(&var("QNC_DIAG_ROOT"))).unwrap();
        let settings = reader.read().unwrap();
        let target = qnc_db_broker::ProjectDbTarget::for_project(&reader, &settings).unwrap();
        let content = Arc::new(content::PlayerContent {
            target: qnc_content_store::ContentTarget::for_project(&reader, &settings),
        });
        let inputs = InputReader::with_content_reader(reader, content);
        let cover = TransientCover {
            clip_id: var("QNC_DIAG_CLIP"),
            source_in: var("QNC_DIAG_IN").parse().unwrap(),
            timebase: (50, 1),
            a2_source_channel: 1,
            a2_level_db: 0,
            a2_off: false,
        };
        let end: u64 = var("QNC_DIAG_END").parse().unwrap();
        let load = qnc_program_input::window_loader(target, settings.project_id.clone(), (0, end), cover);
        let program = load(&inputs, &settings.workspace_db_uri).unwrap();
        for item in &program.playlist.items {
            println!("item {:?}", item.record_range);
            for source in &item.sources {
                println!(
                    "  {} clip={} layer={:?} in={} routes={:?} media={:?}",
                    source.source_id, source.clip_id, source.video_layer,
                    source.source_range.source_in, source.audio_routes, source.media
                );
            }
        }
    }

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
const RECORD_WAIT: &str = "Podaci klipa se pripremaju...";
