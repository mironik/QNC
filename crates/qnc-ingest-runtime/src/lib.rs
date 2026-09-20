//! The database is the only link between processes and forms.
//!
//! When a form plays a clip it writes that into the project database; the background
//! application reads it there and waits. The background application writes that it runs
//! (a lease that it renews) and what it did; a form or another start reads it there.
//! Ages are counted by the clock of the database, so machines that disagree about the
//! time still agree here. Local, LAN or intranet: only the content target differs.

use qnc_ingest_store::content::{Access, ContentTarget, ContentWriteTransport};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub const MODULE_ID: &str = "qnc.module.ingest-runtime";
pub const VERSION: &str = "0.1.0";

/// A player prepares or plays.
pub const PLAYBACK: &str = "playback_active";
/// The background application runs.
pub const WORKER: &str = "worker_lease";
/// What the background application did last.
pub const WORKER_RESULT: &str = "worker_result";

const ON: &str = "on";
const OFF: &str = "off";

/// Playback counts while it was refreshed this recently.
pub const PLAYBACK_FRESH_SECONDS: i64 = 5;
/// The worker lease counts while it was renewed this recently.
pub const WORKER_FRESH_SECONDS: i64 = 10;

const RENEW_EVERY: Duration = Duration::from_secs(1);

/// Whether `name` is on and was written no longer ago than `max_age_seconds`.
pub fn is_fresh(target: &ContentTarget, name: &str, max_age_seconds: i64) -> bool {
    target
        .open(Access::ReadOnly)
        .and_then(|mut client| client.get_runtime(name))
        .ok()
        .flatten()
        .is_some_and(|entry| entry.value == ON && entry.age_seconds <= max_age_seconds)
}

/// The last value written under `name`, whatever its age.
pub fn read(target: &ContentTarget, name: &str) -> Option<String> {
    target
        .open(Access::ReadOnly)
        .and_then(|mut client| client.get_runtime(name))
        .ok()
        .flatten()
        .map(|entry| entry.value)
}

/// A pause decision that asks the database at most twice a second.
pub fn playback_pause(target: ContentTarget) -> Arc<dyn Fn() -> bool + Send + Sync> {
    let last: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    Arc::new(move || {
        let mut last = last.lock().expect("pause state");
        if let Some((at, value)) = *last {
            if at.elapsed() < Duration::from_millis(500) {
                return value;
            }
        }
        let value = is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS);
        *last = Some((Instant::now(), value));
        value
    })
}

/// One writer over the serialized content write transport.
pub struct Writer {
    transport: ContentWriteTransport,
    sequence: u64,
}

impl Writer {
    pub fn start(target: ContentTarget) -> Result<Self, String> {
        Ok(Self {
            transport: ContentWriteTransport::start(target)?,
            sequence: 0,
        })
    }

    /// Writes and waits until the database has it.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), String> {
        self.sequence += 1;
        let key = format!("runtime-{}", self.sequence);
        self.transport
            .set_runtime(key.clone(), name.into(), value.into())
            .map_err(|e| e.to_string())?;
        loop {
            for completion in self.transport.poll() {
                if completion.key == key {
                    return completion.result.map(|_| ()).map_err(|e| e.to_string());
                }
            }
            if !self.transport.has_pending() {
                return Err("Content write transport nije vratio rezultat.".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// Keeps `name` on in the database while it lives and turns it off when it ends.
pub struct Beat {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Beat {
    pub fn start(target: ContentTarget, name: &'static str) -> Result<Self, String> {
        let mut writer = Writer::start(target)?;
        writer.set(name, ON)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("ingest-runtime-beat".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(RENEW_EVERY);
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let _ = writer.set(name, ON);
                }
                let _ = writer.set(name, OFF);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Beat {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// For a form: says whether a player works, without ever blocking the form.
#[derive(Default, Debug)]
pub struct PlaybackReporter {
    target_uri: Option<String>,
    sender: Option<Sender<bool>>,
    thread: Option<JoinHandle<()>>,
    last: bool,
}

impl PlaybackReporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call it as often as you like; the database is written when the state changes and
    /// renewed once a second while a player works.
    pub fn update(&mut self, target: Option<&ContentTarget>, active: bool) {
        let Some(target) = target else {
            return;
        };
        if self.target_uri.as_deref() != Some(target.uri()) {
            self.stop();
            self.target_uri = Some(target.uri().to_string());
            if let Ok(sender) = start_reporter(target.clone(), &mut self.thread) {
                self.sender = Some(sender);
                self.last = false;
            }
        }
        if self.last != active {
            self.last = active;
            if let Some(sender) = &self.sender {
                let _ = sender.send(active);
            }
        }
    }

    fn stop(&mut self) {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for PlaybackReporter {
    fn drop(&mut self) {
        if self.last {
            if let Some(sender) = &self.sender {
                let _ = sender.send(false);
            }
        }
        self.stop();
    }
}

fn start_reporter(
    target: ContentTarget,
    thread: &mut Option<JoinHandle<()>>,
) -> Result<Sender<bool>, String> {
    let (send, receive) = mpsc::channel::<bool>();
    let mut writer = Writer::start(target)?;
    *thread = Some(
        std::thread::Builder::new()
            .name("ingest-runtime-playback".into())
            .spawn(move || {
                let mut active = false;
                loop {
                    match receive.recv_timeout(RENEW_EVERY) {
                        Ok(state) => {
                            active = state;
                            let _ = writer.set(PLAYBACK, if active { ON } else { OFF });
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            if active {
                                let _ = writer.set(PLAYBACK, ON);
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .map_err(|e| e.to_string())?,
    );
    Ok(send)
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "qnc://local/db/ingest_content/p1";

    fn target() -> (tempfile::TempDir, ContentTarget) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE project_settings(project_id TEXT,settings_json TEXT);
             INSERT INTO project_settings VALUES('p1','{}');
             CREATE VIEW public_project_settings AS SELECT * FROM project_settings;",
        )
        .unwrap();
        drop(conn);
        // The owner opens it read-write once so the tables exist.
        qnc_ingest_store::content::ContentClient::from_owner_binding(&path, URI, Access::ReadWrite)
            .unwrap();
        let target = ContentTarget::from_owner_binding(&path, URI).unwrap();
        (dir, target)
    }

    #[test]
    fn nothing_is_told_until_something_is_written() {
        let (_dir, target) = target();
        assert!(!is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS));
        assert!(read(&target, WORKER).is_none());
    }

    #[test]
    fn a_written_state_is_read_back_from_the_database() {
        let (_dir, target) = target();
        let mut writer = Writer::start(target.clone()).unwrap();
        writer.set(PLAYBACK, "on").unwrap();
        assert!(is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS));
        writer.set(PLAYBACK, "off").unwrap();
        assert!(!is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS));
        writer.set(WORKER_RESULT, "uvezeno 2, neuspjelo 0").unwrap();
        assert_eq!(read(&target, WORKER_RESULT).as_deref(), Some("uvezeno 2, neuspjelo 0"));
    }

    #[test]
    fn a_beat_keeps_the_lease_on_while_it_lives_and_off_when_it_ends() {
        let (_dir, target) = target();
        let beat = Beat::start(target.clone(), WORKER).unwrap();
        assert!(is_fresh(&target, WORKER, WORKER_FRESH_SECONDS));
        drop(beat);
        assert!(!is_fresh(&target, WORKER, WORKER_FRESH_SECONDS));
    }

    #[test]
    fn the_form_reports_playback_and_the_pause_decision_follows_it() {
        let (_dir, target) = target();
        let pause = playback_pause(target.clone());
        assert!(!pause());
        let mut reporter = PlaybackReporter::new();
        reporter.update(Some(&target), true);
        let started = Instant::now();
        while !is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS) {
            assert!(started.elapsed() < Duration::from_secs(5), "never reported");
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(600));
        assert!(pause(), "the copy waits while the player works");
        reporter.update(Some(&target), false);
        let started = Instant::now();
        while is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS) {
            assert!(started.elapsed() < Duration::from_secs(5), "never turned off");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
