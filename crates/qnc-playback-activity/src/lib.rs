//! The database is the only link between processes and forms.
//!
//! When a form plays a clip it writes that into the project database; the background
//! application reads it there and waits. The background application writes that it runs
//! (a lease that it renews) and what it did; a form or another start reads it there.
//! Ages are counted by the clock of the database, so machines that disagree about the
//! time still agree here. Local, LAN or intranet: only the content target differs.

use qnc_content_store::{Access, ContentTarget, ContentWriteTransport, LeaseAct};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub const MODULE_ID: &str = "qnc.module.playback-activity";
pub const VERSION: &str = "0.1.0";

/// A player prepares or plays.
pub const PLAYBACK: &str = "playback_active";
/// The clip a preview opens last: whoever completes records takes it first (v5: the
/// selected clip first).
pub const PLAYBACK_CLIP: &str = "playback_clip";
/// The background application runs.
pub const WORKER: &str = "worker_lease";
/// What the background application did last.
pub const WORKER_RESULT: &str = "worker_result";

const ON: &str = "on";
const OFF: &str = "off";
/// The user edits (a step, a mark, a trim) without the player playing: still "on" for
/// whoever waits for any player work, a lighter level for the background (user 2026-10-20).
const EDITING: &str = "on:editing";

/// How much a player works now (user 2026-10-20, measured on a 4 core laptop: beside a
/// playing player one core of background work costs nothing, two cost stutters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackLevel {
    #[default]
    Idle,
    /// The user edits: a step, a mark, a trim (the player does not play).
    Editing,
    /// The player prepares or plays.
    Playing,
}

impl PlaybackLevel {
    /// The workers background work may run at this level (`None`: as many as it likes).
    pub fn background_cap(self) -> Option<usize> {
        match self {
            Self::Idle => None,
            Self::Editing => Some(2),
            Self::Playing => Some(1),
        }
    }
}

/// The level of player work, refreshed recently enough; asks the database at most twice
/// a second.
pub fn playback_level(target: ContentTarget) -> Arc<dyn Fn() -> PlaybackLevel + Send + Sync> {
    let last: Mutex<Option<(Instant, PlaybackLevel)>> = Mutex::new(None);
    Arc::new(move || {
        let mut last = last.lock().expect("level state");
        if let Some((at, value)) = *last {
            if at.elapsed() < Duration::from_millis(500) {
                return value;
            }
        }
        let value = target
            .open(Access::ReadOnly)
            .and_then(|mut client| client.get_runtime(PLAYBACK))
            .ok()
            .flatten()
            .filter(|entry| entry.age_seconds <= PLAYBACK_FRESH_SECONDS)
            .map_or(PlaybackLevel::Idle, |entry| match entry.value.as_str() {
                ON => PlaybackLevel::Playing,
                EDITING => PlaybackLevel::Editing,
                _ => PlaybackLevel::Idle,
            });
        *last = Some((Instant::now(), value));
        value
    })
}

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
        .is_some_and(|entry| held(&entry.value) && entry.age_seconds <= max_age_seconds)
}

/// "on", or a lease held by an owner ("on:<owner>").
fn held(value: &str) -> bool {
    value == ON || value.starts_with("on:")
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
        self.wait(&key).map(|_| ())
    }

    /// One lease act of `owner` on `name`; true when it succeeded for this owner.
    pub fn lease(&mut self, name: &str, owner: &str, act: LeaseAct, max_age_seconds: i64) -> Result<bool, String> {
        self.sequence += 1;
        let key = format!("lease-{}", self.sequence);
        self.transport
            .lease(key.clone(), name.into(), owner.into(), act, max_age_seconds)
            .map_err(|e| e.to_string())?;
        match self.wait(&key)? {
            qnc_content_store::ContentWriteData::Leased(held) => Ok(held),
            _ => Err(qnc_ui_text::text("db.content_writer_returned_result").into()),
        }
    }

    fn wait(&mut self, key: &str) -> Result<qnc_content_store::ContentWriteData, String> {
        loop {
            for completion in self.transport.poll() {
                if completion.key == key {
                    return completion.result.map(|result| result.data).map_err(|e| e.to_string());
                }
            }
            if !self.transport.has_pending() {
                return Err(qnc_ui_text::text("db.content_writer_returned_result").into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// The one holder of `name` in a project (external audit 2026-10-17): taken in one
/// conditional statement with its owner, renewed and let go only while it is still this
/// owner's. Two processes never both take it; one ending never lets go of another's.
pub struct Lease {
    stop: Arc<AtomicBool>,
    held: Arc<AtomicBool>,
    /// Set once the lease is lost: the cancel of the work done under it.
    lost: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    target: ContentTarget,
    name: &'static str,
    owner: String,
    fresh_seconds: i64,
}

impl Lease {
    /// Takes `name` when nobody holds it, its holder let it go, or it was not renewed for
    /// `fresh_seconds`; `None` when another holds it. It is renewed once a second.
    pub fn claim(target: ContentTarget, name: &'static str, fresh_seconds: i64) -> Result<Option<Self>, String> {
        let owner = owner_id();
        let mut writer = Writer::start(target.clone())?;
        if !writer.lease(name, &owner, LeaseAct::Claim, fresh_seconds)? {
            return Ok(None);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let held = Arc::new(AtomicBool::new(true));
        let lost = Arc::new(AtomicBool::new(false));
        let (flag, holding, losing, renewer) = (stop.clone(), held.clone(), lost.clone(), owner.clone());
        let thread = std::thread::Builder::new()
            .name("playback-activity-lease".into())
            .spawn(move || {
                let mut renewed = Instant::now();
                let lose = || {
                    holding.store(false, Ordering::Relaxed);
                    losing.store(true, Ordering::Relaxed);
                };
                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(RENEW_EVERY);
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    match writer.lease(name, &renewer, LeaseAct::Renew, fresh_seconds) {
                        Ok(true) => renewed = Instant::now(),
                        // Another owner took it.
                        Ok(false) => return lose(),
                        // Not renewed for as long as it is fresh (external audit 2026-10-17: a
                        // failing renewal was ignored and the work went on): another may take it.
                        Err(_) if renewed.elapsed() >= Duration::from_secs(fresh_seconds.max(1) as u64) => return lose(),
                        Err(_) => {}
                    }
                }
                let _ = writer.lease(name, &renewer, LeaseAct::Release, fresh_seconds);
            })
            .map_err(|e| e.to_string())?;
        Ok(Some(Self { stop, held, lost, thread: Some(thread), target, name, owner, fresh_seconds }))
    }

    /// The cancel of the work done under this lease: set once it is lost.
    pub fn lost(&self) -> &AtomicBool {
        &self.lost
    }

    /// Still this owner's in the database, now: asked before work is published (external
    /// audit 2026-10-17), so a process whose lease another took writes nothing more.
    pub fn confirm(&self) -> bool {
        self.held()
            && self
                .target
                .open(Access::ReadOnly)
                .and_then(|mut client| client.get_runtime(self.name))
                .ok()
                .flatten()
                .is_some_and(|entry| {
                    entry.value == qnc_content_store::lease_value(&self.owner) && entry.age_seconds <= self.fresh_seconds
                })
    }

    /// What a write made under this lease carries: the database owner checks it in the
    /// same transaction as the write (external audit 2026-10-17).
    pub fn write_lease(&self) -> qnc_content_store::WriteLease {
        qnc_content_store::WriteLease {
            key: self.name.into(),
            owner: self.owner.clone(),
            max_age_seconds: self.fresh_seconds,
        }
    }

    /// Still this owner's: false once another owner took it (it was not renewed in time).
    pub fn held(&self) -> bool {
        self.held.load(Ordering::Relaxed)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// This process and the moment it asked: unique among the holders of a project.
fn owner_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{}-{nanos}", std::process::id())
}

/// For a form: says whether a player works, without ever blocking the form.
#[derive(Default, Debug)]
pub struct PlaybackReporter {
    target_uri: Option<String>,
    sender: Option<Sender<Report>>,
    thread: Option<JoinHandle<()>>,
    last: PlaybackLevel,
    /// The last clip told, told again by a reporter started anew.
    clip: Option<String>,
    /// When a reporter that stopped (its writer did not open) is started again.
    retry_at: Option<Instant>,
}

/// How long a reporter that stopped waits before it is started again for the same project.
const REPORTER_RETRY: Duration = Duration::from_secs(2);

impl PlaybackReporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call it as often as you like; the database is written when the state changes and
    /// renewed once a second while a player works.
    pub fn update(&mut self, target: Option<&ContentTarget>, active: bool) {
        self.update_level(target, if active { PlaybackLevel::Playing } else { PlaybackLevel::Idle });
    }

    /// The same with the level of the work (playing, editing or idle).
    pub fn update_level(&mut self, target: Option<&ContentTarget>, level: PlaybackLevel) {
        let Some(target) = target else {
            return;
        };
        self.bind(target);
        if self.last != level {
            self.last = level;
            if let Some(sender) = &self.sender {
                let _ = sender.send(Report::Active(level));
            }
        }
    }

    /// The clip a preview opens: written once, never blocking the form.
    pub fn clip(&mut self, target: Option<&ContentTarget>, clip_id: &str) {
        let Some(target) = target else {
            return;
        };
        self.bind(target);
        self.clip = Some(clip_id.to_string());
        if let Some(sender) = &self.sender {
            let _ = sender.send(Report::Clip(clip_id.into()));
        }
    }

    /// The reporter runs (its writer opened and it has not stopped): false while one that
    /// stopped waits to be started again (external audit 2026-10-17).
    pub fn reporting(&self) -> bool {
        self.thread.as_ref().is_some_and(|thread| !thread.is_finished())
    }

    fn bind(&mut self, target: &ContentTarget) {
        if self.target_uri.as_deref() != Some(target.uri()) {
            self.stop();
            self.target_uri = Some(target.uri().to_string());
            (self.last, self.clip, self.retry_at) = (PlaybackLevel::Idle, None, None);
            self.start(target);
            return;
        }
        // The reporter of the same project stopped (external audit 2026-10-17: its writer
        // did not open and nothing more was ever told): started again after a pause, with
        // what it was last told.
        if self.reporting() {
            return;
        }
        let now = Instant::now();
        match self.retry_at {
            None => self.retry_at = Some(now + REPORTER_RETRY),
            Some(at) if now >= at => {
                self.retry_at = None;
                self.start(target);
                if let Some(sender) = &self.sender {
                    if self.last != PlaybackLevel::Idle {
                        let _ = sender.send(Report::Active(self.last));
                    }
                    if let Some(clip) = &self.clip {
                        let _ = sender.send(Report::Clip(clip.clone()));
                    }
                }
            }
            Some(_) => {}
        }
    }

    fn start(&mut self, target: &ContentTarget) {
        self.sender = start_reporter(target.clone(), &mut self.thread).ok();
    }

    /// Lets the reporter go without waiting for it (external audit 2026-10-17: the form
    /// waited for its database work): its channel closes, it writes what it was told and
    /// ends on its own.
    fn stop(&mut self) {
        self.sender = None;
        self.thread = None;
    }
}

impl Drop for PlaybackReporter {
    fn drop(&mut self) {
        if self.last != PlaybackLevel::Idle {
            if let Some(sender) = &self.sender {
                let _ = sender.send(Report::Active(PlaybackLevel::Idle));
            }
        }
        self.stop();
    }
}

#[derive(Debug)]
enum Report {
    Active(PlaybackLevel),
    Clip(String),
}

fn start_reporter(
    target: ContentTarget,
    thread: &mut Option<JoinHandle<()>>,
) -> Result<Sender<Report>, String> {
    let (send, receive) = mpsc::channel::<Report>();
    *thread = Some(
        std::thread::Builder::new()
            .name("playback-activity-report".into())
            .spawn(move || {
                // The writer opens on its own thread, never on the form's. A write that fails
                // (the database was away) drops the writer: the next one opens it again and
                // what is still to be told is told then (external audit 2026-10-17: a writer
                // that failed once left the reporter silent for good).
                let mut writer: Option<Writer> = None;
                let (mut active, mut clip_to_tell): (Option<PlaybackLevel>, Option<String>) = (None, None);
                loop {
                    match receive.recv_timeout(RENEW_EVERY) {
                        Ok(Report::Active(state)) => active = Some(state),
                        Ok(Report::Clip(clip_id)) => clip_to_tell = Some(clip_id),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => {
                            // What it was told last is still written before it ends.
                            if active.is_some() || clip_to_tell.is_some() {
                                tell(&target, &mut writer, &mut active, &mut clip_to_tell);
                            }
                            break;
                        }
                    }
                    tell(&target, &mut writer, &mut active, &mut clip_to_tell);
                }
            })
            .map_err(|e| e.to_string())?,
    );
    Ok(send)
}

/// Writes what the reporter is to tell: the playback state (renewed while on, written
/// once when off) and the last clip. Kept to tell again when the write fails.
fn tell(target: &ContentTarget, writer: &mut Option<Writer>, active: &mut Option<PlaybackLevel>, clip: &mut Option<String>) {
    if writer.is_none() {
        *writer = Writer::start(target.clone()).ok();
    }
    let Some(open) = writer.as_mut() else {
        return;
    };
    let mut failed = None;
    if let Some(state) = *active {
        let value = match state {
            PlaybackLevel::Playing => ON,
            PlaybackLevel::Editing => EDITING,
            PlaybackLevel::Idle => OFF,
        };
        match open.set(PLAYBACK, value) {
            // On is renewed every second; off is written once.
            Ok(()) if state == PlaybackLevel::Idle => *active = None,
            Ok(()) => {}
            Err(error) => failed = Some(error),
        }
    }
    if let Some(clip_id) = clip.clone().filter(|_| failed.is_none()) {
        match open.set(PLAYBACK_CLIP, &clip_id) {
            Ok(()) => *clip = None,
            Err(error) => failed = Some(error),
        }
    }
    if let Some(error) = failed {
        *writer = None;
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!("playback reporter write failed, it opens its writer again: {error}"),
            );
        }
    }
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
        qnc_content_store::ContentClient::from_owner_binding(&path, URI, Access::ReadWrite)
            .unwrap();
        let target = ContentTarget::from_owner_binding(&path, URI).unwrap();
        (dir, target)
    }

    #[test]
    fn a_reporter_whose_writes_failed_tells_its_state_once_the_database_is_back() {
        let (dir, target) = target();
        // The database says it is another project's: every open of the writer is refused.
        // No file is moved (third audit 2026-10-17: a rename of an open file failed on
        // Windows), so the test does not depend on who holds the file when.
        let belong = |project: &str| {
            let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
            conn.busy_timeout(Duration::from_secs(5)).unwrap();
            conn.execute("UPDATE project_settings SET project_id=?1", [project]).unwrap();
        };
        belong("another");
        let mut reporter = PlaybackReporter::new();
        reporter.update(Some(&target), true);
        std::thread::sleep(Duration::from_millis(1500)); // its writes fail meanwhile
        assert!(reporter.reporting(), "it keeps running");
        assert!(!is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS), "nothing was written meanwhile");
        belong("p1");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS) && Instant::now() < deadline {
            reporter.update(Some(&target), true);
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(is_fresh(&target, PLAYBACK, PLAYBACK_FRESH_SECONDS), "the database back, it told that a player works");
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
        assert_eq!(
            read(&target, WORKER_RESULT).as_deref(),
            Some("uvezeno 2, neuspjelo 0")
        );
    }

    #[test]
    fn a_lease_another_owner_took_is_not_confirmed_and_its_work_is_cancelled() {
        let (_dir, target) = target();
        let lease = Lease::claim(target.clone(), WORKER, WORKER_FRESH_SECONDS).unwrap().expect("taken");
        assert!(lease.confirm(), "its own in the database");
        // Another owner holds it now (as after this one was not renewed in time).
        Writer::start(target.clone()).unwrap().set(WORKER, &qnc_content_store::lease_value("other-owner")).unwrap();
        assert!(!lease.confirm(), "nothing may be published under it any more");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !lease.lost().load(Ordering::Relaxed) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(lease.lost().load(Ordering::Relaxed), "the renewal saw it and cancelled the work");
        assert!(!lease.held());
    }

    #[test]
    fn a_lease_is_one_holders_and_its_end_lets_go_only_of_its_own() {
        let (_dir, target) = target();
        let first = Lease::claim(target.clone(), WORKER, WORKER_FRESH_SECONDS).unwrap().expect("the first takes it");
        assert!(is_fresh(&target, WORKER, WORKER_FRESH_SECONDS));
        assert!(Lease::claim(target.clone(), WORKER, WORKER_FRESH_SECONDS).unwrap().is_none(), "a second does not");
        assert!(first.held());
        drop(first);
        assert!(!is_fresh(&target, WORKER, WORKER_FRESH_SECONDS));
        let second = Lease::claim(target.clone(), WORKER, WORKER_FRESH_SECONDS).unwrap().expect("free again");
        // An owner that lost it (here: a stale one) cannot let go of the new holder's.
        let mut writer = Writer::start(target.clone()).unwrap();
        assert!(!writer.lease(WORKER, "someone-else", LeaseAct::Release, WORKER_FRESH_SECONDS).unwrap());
        assert!(is_fresh(&target, WORKER, WORKER_FRESH_SECONDS));
        drop(second);
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
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "never turned off"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
