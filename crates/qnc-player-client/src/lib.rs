//! Public process client. No decoder, playback clock, database owner or application identity.
mod connection;
use qnc_player_contract::{
    BroadcastPlayerProtocolEvent as Event, Timebase, TransportStatus, envelope::EventEnvelope,
    session::MonitorHeader,
};
use qnc_player_input::PreparedInput;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::Duration,
};

pub type Result<T> = std::result::Result<T, String>;

/// Private storage-owner binding. Never part of a player command or state reply.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaBinding {
    Local {
        source_uri: String,
        root: PathBuf,
    },
    Network {
        environment: String,
        authority: String,
        base_url: String,
        token: String,
    },
}
pub struct Launch {
    pub executable: PathBuf,
    pub input: PreparedInput,
    pub media_binding: MediaBinding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorFrame {
    pub header: MonitorHeader,
    pub rgba: Arc<[u8]>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub preparing: bool,
    pub video_visible: bool,
    pub reply: Option<EventEnvelope>,
    pub picture: Option<Arc<MonitorFrame>>,
    pub error: Option<String>,
}
impl View {
    pub fn playing(&self) -> bool {
        self.reply.as_ref().is_some_and(|r| {
            r.events.iter().any(|e| {
                matches!(
                    e,
                    Event::TransportStatusChanged {
                        status: TransportStatus::Playing
                    }
                )
            })
        })
    }
    pub fn ready(&self) -> bool {
        self.reply.as_ref().is_some_and(|r| {
            r.events
                .iter()
                .any(|e| matches!(e, Event::PlaybackReadinessChanged { ready: true, .. }))
        })
    }
    pub fn has_confirmed_position(&self) -> bool {
        self.reply.as_ref().is_some_and(|r| {
            r.events.iter().rev().any(|e| {
                matches!(
                    e,
                    Event::CarrierPositionChanged {
                        range: Some(_),
                        timebase: Some(_),
                        ..
                    }
                )
            })
        })
    }
    pub fn can_start_playback(&self) -> bool {
        self.ready() && self.has_confirmed_position()
    }

    pub fn source_timebase(&self) -> Option<Timebase> {
        if let Some(picture) = &self.picture {
            return Some(picture.header.timebase);
        }
        self.reply.as_ref().and_then(|reply| {
            reply.events.iter().rev().find_map(|event| match event {
                Event::CarrierPositionChanged {
                    timebase: Some(timebase),
                    ..
                } => Some(*timebase),
                _ => None,
            })
        })
    }

    pub fn source_frame_interval(&self) -> Option<Duration> {
        let timebase = self.source_timebase()?;
        frame_interval(timebase)
    }
}

fn frame_interval(timebase: Timebase) -> Option<Duration> {
    if timebase.fps_num <= 0 || timebase.fps_den <= 0 {
        return None;
    }
    let nanos = 1_000_000_000u128
        .checked_mul(u128::try_from(timebase.fps_den).ok()?)?
        .div_ceil(u128::try_from(timebase.fps_num).ok()?);
    Some(Duration::from_nanos(u64::try_from(nanos).ok()?))
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    TogglePlayPause,
    Step(i64),
    Cue(u64),
}
type Loader = Box<dyn FnOnce() -> Result<Launch> + Send>;
struct Shared {
    generation: AtomicU64,
    stop: AtomicBool,
    load: Mutex<Option<(u64, Loader)>>,
    view: Mutex<View>,
    notify: OnceLock<Box<dyn Fn() + Send + Sync>>,
}
pub struct Player {
    shared: Arc<Shared>,
    commands: SyncSender<(u64, Action)>,
    worker: Option<thread::JoinHandle<()>>,
}
impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PlayerClient")
    }
}
impl Player {
    pub fn new() -> Result<Self> {
        let shared = Arc::new(Shared {
            generation: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            load: Mutex::new(None),
            view: Mutex::new(View::default()),
            notify: OnceLock::new(),
        });
        let (commands, receiver) = mpsc::sync_channel(16);
        let state = shared.clone();
        let worker = thread::Builder::new()
            .name("player-client".into())
            .spawn(move || {
                let mut active: Option<(u64, connection::Connection)> = None;
                let mut initial_preview: Option<(u64, u64)> = None;
                loop {
                    if state.stop.load(Ordering::Acquire) {
                        break;
                    }
                    let current = state.generation.load(Ordering::Acquire);
                    if active
                        .as_ref()
                        .is_some_and(|(generation, _)| *generation != current)
                    {
                        active = None;
                        initial_preview = None;
                    }
                    let pending = state.load.lock().unwrap().take();
                    if let Some((generation, load)) = pending {
                        // Selection may arrive after the generation check above.
                        // Reap the preceding process before reading/preparing its replacement.
                        active = None;
                        initial_preview = None;
                        let result = load().and_then(|launch| {
                            if state.generation.load(Ordering::Acquire) != generation
                                || state.stop.load(Ordering::Acquire)
                            {
                                return Err("superseded player selection".into());
                            }
                            connection::Connection::launch(
                                launch,
                                generation,
                                {
                                    let state = state.clone();
                                    Arc::new(move |picture| {
                                        publish_picture(&state, generation, picture)
                                    })
                                },
                            )
                        });
                        if state.generation.load(Ordering::Acquire) == generation {
                            match result {
                                Ok(connection) => {
                                    active = Some((generation, connection));
                                    initial_preview = Some((generation, 0));
                                }
                                Err(error) => publish(
                                    &state,
                                    generation,
                                    View {
                                        error: Some(error),
                                        ..View::default()
                                    },
                                ),
                            }
                        }
                    }
                    if let Some((generation, connection)) = &mut active {
                        let action = receiver
                            .try_recv()
                            .ok()
                            .and_then(|(g, action)| (g == *generation).then_some(action));
                        let action = action.or_else(|| {
                            let view = state.view.lock().unwrap().clone();
                            initial_preview_action(&mut initial_preview, *generation, &view)
                        });
                        match connection.poll(action) {
                            Ok(view) => publish(&state, *generation, view),
                            Err(error) => {
                                publish(
                                    &state,
                                    *generation,
                                    View {
                                        error: Some(error),
                                        ..View::default()
                                    },
                                );
                                active = None;
                            }
                        }
                    } else {
                        while receiver.try_recv().is_ok() {}
                    }
                    thread::sleep(Duration::from_millis(if active.is_some() { 1 } else { 8 }));
                }
                drop(active);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            shared,
            commands,
            worker: Some(worker),
        })
    }
    /// Latest selection wins; loading and process startup never run on the caller/UI thread.
    pub fn prepare(&self, load: impl FnOnce() -> Result<Launch> + Send + 'static) {
        let mut view = self.shared.view.lock().unwrap();
        let generation = self.shared.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *view = View {
            preparing: true,
            ..View::default()
        };
        *self.shared.load.lock().unwrap() = Some((generation, Box::new(load)));
    }
    pub fn close(&self) {
        let mut view = self.shared.view.lock().unwrap();
        self.shared.generation.fetch_add(1, Ordering::AcqRel);
        self.shared.load.lock().unwrap().take();
        *view = View::default();
    }
    pub fn send(&self, action: Action) -> Result<()> {
        let view = self.view();
        if view.reply.is_none() {
            return Err(view.error.unwrap_or_else(|| "Player se priprema.".into()));
        }
        if !view.has_confirmed_position() {
            return Err("Player se priprema.".into());
        }
        self.commands
            .try_send((self.shared.generation.load(Ordering::Acquire), action))
            .map_err(|_| "Player command queue is full or closed.".into())
    }
    pub fn view(&self) -> View {
        self.shared.view.lock().unwrap().clone()
    }
    /// Register a nonblocking display wake-up. It carries no playback state or clock.
    pub fn notify_on_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        let _ = self.shared.notify.set(Box::new(notify));
    }
}
fn publish_picture(state: &Shared, generation: u64, picture: Option<Arc<MonitorFrame>>) {
    let mut current = state.view.lock().unwrap();
    if state.generation.load(Ordering::Acquire) != generation {
        return;
    }
    let same_picture = match (&current.picture, &picture) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    current.picture = picture;
    drop(current);
    if !same_picture && let Some(notify) = state.notify.get() {
        notify();
    }
}

fn publish(state: &Shared, generation: u64, view: View) {
    let mut current = state.view.lock().unwrap();
    if state.generation.load(Ordering::Acquire) == generation {
        let same_picture = match (&current.picture, &view.picture) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        let changed = !same_picture
            || current.preparing != view.preparing
            || current.video_visible != view.video_visible
            || current.error != view.error
            || current.reply != view.reply;
        *current = view;
        drop(current);
        if changed && let Some(notify) = state.notify.get() {
            notify();
        }
    }
}

fn initial_preview_action(
    pending: &mut Option<(u64, u64)>,
    generation: u64,
    view: &View,
) -> Option<Action> {
    let (pending_generation, frame) = *pending.as_ref()?;
    if pending_generation != generation || view.error.is_some() || view.playing() {
        *pending = None;
        return None;
    }
    if !view.has_confirmed_position() {
        return None;
    }
    *pending = None;
    Some(Action::Cue(frame))
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_player_contract::{FrameRange, Timebase};

    fn reply(events: Vec<Event>) -> EventEnvelope {
        EventEnvelope {
            contract_version: qnc_player_contract::VERSION.into(),
            session_id: "s".into(),
            source_generation: 1,
            sequence: 1,
            events,
        }
    }

    #[test]
    fn view_start_playback_requires_ready_position_range_and_timebase() {
        let mut view = View {
            reply: Some(reply(vec![Event::PlaybackReadinessChanged {
                source_id: Some("clip".into()),
                frame: 0,
                ready: true,
            }])),
            ..View::default()
        };
        assert!(view.ready());
        assert!(!view.can_start_playback());

        view.reply = Some(reply(vec![Event::CarrierPositionChanged {
            source_id: Some("clip".into()),
            frame: 0,
            range: Some(FrameRange::new(0, 100).unwrap()),
            timebase: Some(Timebase::new(50, 1).unwrap()),
            status: TransportStatus::Ready,
        }]));
        assert!(view.has_confirmed_position());
        assert!(!view.can_start_playback());

        view.reply = Some(reply(vec![
            Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 0,
                range: Some(FrameRange::new(0, 100).unwrap()),
                timebase: Some(Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Ready,
            },
            Event::PlaybackReadinessChanged {
                source_id: Some("clip".into()),
                frame: 0,
                ready: true,
            },
        ]));
        assert!(view.can_start_playback());
    }

    #[test]
    fn repaint_interval_comes_from_confirmed_source_timebase() {
        let view = View {
            reply: Some(reply(vec![Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 12,
                range: Some(FrameRange::new(0, 100).unwrap()),
                timebase: Some(Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Playing,
            }])),
            ..View::default()
        };

        assert_eq!(
            view.source_frame_interval(),
            Some(Duration::from_millis(20))
        );
    }

    #[test]
    fn unconfirmed_player_view_has_no_repaint_clock() {
        let view = View {
            reply: Some(reply(vec![Event::PlaybackReadinessChanged {
                source_id: Some("clip".into()),
                frame: 0,
                ready: true,
            }])),
            ..View::default()
        };

        assert_eq!(view.source_timebase(), None);
        assert_eq!(view.source_frame_interval(), None);
    }

    #[test]
    fn send_rejects_unconfirmed_player_position_before_queueing() {
        let player = Player::new().unwrap();
        publish(
            &player.shared,
            0,
            View {
                reply: Some(reply(vec![Event::PlaybackReadinessChanged {
                    source_id: Some("clip".into()),
                    frame: 0,
                    ready: true,
                }])),
                ..View::default()
            },
        );
        assert_eq!(
            player.send(Action::TogglePlayPause).unwrap_err(),
            "Player se priprema."
        );
    }

    #[test]
    fn initial_preview_cues_first_frame_once_after_confirmed_position() {
        let mut pending = Some((7, 0));
        let empty = View::default();
        assert!(initial_preview_action(&mut pending, 7, &empty).is_none());
        assert_eq!(pending, Some((7, 0)));

        let confirmed = View {
            reply: Some(reply(vec![Event::CarrierPositionChanged {
                source_id: Some("clip".into()),
                frame: 0,
                range: Some(FrameRange::new(0, 100).unwrap()),
                timebase: Some(Timebase::new(50, 1).unwrap()),
                status: TransportStatus::Preparing,
            }])),
            ..View::default()
        };
        assert!(matches!(
            initial_preview_action(&mut pending, 7, &confirmed),
            Some(Action::Cue(0))
        ));
        assert!(pending.is_none());
        assert!(initial_preview_action(&mut pending, 7, &confirmed).is_none());
    }

    #[test]
    fn initial_preview_is_discarded_for_stale_or_playing_session() {
        let confirmed_playing = View {
            reply: Some(reply(vec![
                Event::TransportStatusChanged {
                    status: TransportStatus::Playing,
                },
                Event::CarrierPositionChanged {
                    source_id: Some("clip".into()),
                    frame: 0,
                    range: Some(FrameRange::new(0, 100).unwrap()),
                    timebase: Some(Timebase::new(50, 1).unwrap()),
                    status: TransportStatus::Playing,
                },
            ])),
            ..View::default()
        };
        let mut stale = Some((6, 0));
        assert!(initial_preview_action(&mut stale, 7, &confirmed_playing).is_none());
        assert!(stale.is_none());

        let mut playing = Some((7, 0));
        assert!(initial_preview_action(&mut playing, 7, &confirmed_playing).is_none());
        assert!(playing.is_none());
    }

    #[test]
    fn notification_is_only_for_current_changed_view_and_runs_without_view_lock() {
        let player = Player::new().unwrap();
        let calls = Arc::new(AtomicU64::new(0));
        let count = calls.clone();
        let weak = Arc::downgrade(&player.shared);
        player.notify_on_change(move || {
            assert!(weak.upgrade().unwrap().view.try_lock().is_ok());
            count.fetch_add(1, Ordering::Relaxed);
        });
        let view = View {
            video_visible: true,
            ..View::default()
        };
        publish(&player.shared, 1, view.clone());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        publish(&player.shared, 0, view.clone());
        publish(&player.shared, 0, view);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn mailbox_picture_publishes_without_a_new_state_reply() {
        let shared = Shared {
            generation: AtomicU64::new(1),
            stop: AtomicBool::new(false),
            load: Mutex::new(None),
            view: Mutex::new(View {
                reply: Some(reply(Vec::new())),
                ..View::default()
            }),
            notify: OnceLock::new(),
        };
        let calls = Arc::new(AtomicU64::new(0));
        let count = calls.clone();
        let _ = shared.notify.set(Box::new(move || {
            count.fetch_add(1, Ordering::Relaxed);
        }));
        let picture = Arc::new(MonitorFrame {
            header: MonitorHeader {
                contract_version: qnc_player_contract::VERSION.into(),
                session_id: "s".into(),
                source_generation: 1,
                output_generation: 4,
                sequence: 9,
                source_id: "clip".into(),
                frame: 9,
                timebase: Timebase::new(50, 1).unwrap(),
                width: 2,
                height: 2,
            },
            rgba: vec![0; 16].into(),
        });
        publish_picture(&shared, 1, Some(picture.clone()));
        let view = shared.view.lock().unwrap();
        assert!(view.reply.is_some());
        assert!(Arc::ptr_eq(view.picture.as_ref().unwrap(), &picture));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn stale_load_result_cannot_replace_new_selection_or_closed_view() {
        let player = Player::new().unwrap();
        let (started, wait_started) = mpsc::sync_channel(1);
        let (resume, wait_resume) = mpsc::sync_channel(1);
        player.prepare(move || {
            started.send(()).unwrap();
            wait_resume.recv().unwrap();
            Err("old".into())
        });
        wait_started.recv_timeout(Duration::from_secs(1)).unwrap();
        player.prepare(|| Err("current".into()));
        resume.send(()).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(1);
        while player.view().preparing && std::time::Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(player.view().error.as_deref(), Some("current"));
        player.close();
        assert_eq!(player.view(), View::default());
        assert!(player.send(Action::TogglePlayPause).is_err());
    }
}
