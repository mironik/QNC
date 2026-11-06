//! One value of the clip a form shows (its sound, channels, timecode, shots...), followed in the
//! database (user rule: "the only truth is the database, no other link between forms"): read in
//! a background thread, read again in a rhythm while the clip stays shown, so a change written
//! by another form, process or computer shows; a read that fails keeps the last answer. The
//! form's thread never waits on the database. The piece reads nothing itself: the caller gives
//! the read; it knows no table, form or application.

use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const MODULE_ID: &str = "qnc.module.clip-follow";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How often the shown clip's value is read again.
pub const EVERY: Duration = Duration::from_secs(1);

type Key = (String, String);

#[derive(Debug)]
pub struct ClipFollow<T> {
    every: Duration,
    /// The last answer of the database for a clip: (project, clip), value.
    current: Option<(Key, T)>,
    /// A read on its own thread: (project, clip), the generation it was started in.
    reading: Option<(Key, u64, mpsc::Receiver<Result<T, String>>)>,
    /// When the clip is read again.
    due: Option<(Key, Instant)>,
    /// A value the caller wrote itself bumps it: a read started before it is not the truth.
    generation: u64,
    /// Why the last read of a clip failed; gone with its next answer.
    error: Option<(Key, String)>,
    name: &'static str,
}

impl<T: Clone + Send + 'static> Default for ClipFollow<T> {
    fn default() -> Self {
        Self::new("clip-follow", EVERY)
    }
}

impl<T: Clone + Send + 'static> ClipFollow<T> {
    /// `name` goes into the log with a failed read and names the thread.
    pub fn new(name: &'static str, every: Duration) -> Self {
        Self { every, current: None, reading: None, due: None, generation: 0, error: None, name }
    }

    /// The value of `clip` in project `project`; `None` until its first read came. `may_read`
    /// false holds the next read back (a write of the caller still on its way); `read` makes
    /// the read when one starts.
    pub fn get<F>(&mut self, project: &str, clip: &str, may_read: bool, read: impl FnOnce() -> F) -> Option<T>
    where
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        self.take_read();
        let key = (project.to_string(), clip.to_string());
        let wanted = |of: &Key| *of == key;
        let reading = self.reading.as_ref().is_some_and(|(of, _, _)| wanted(of));
        let due = !self.due.as_ref().is_some_and(|(of, at)| wanted(of) && Instant::now() < *at);
        if may_read && due && !reading {
            if self.reading.is_some() {
                // The read of another clip is not waited for.
                self.reading = None;
            }
            let (send, done) = mpsc::channel();
            let read = read();
            let spawned = std::thread::Builder::new().name(format!("qnc-{}-read", self.name)).spawn(move || _ = send.send(read()));
            if spawned.is_ok() {
                self.reading = Some((key.clone(), self.generation, done));
            }
            self.due = Some((key.clone(), Instant::now() + self.every));
        }
        self.current.as_ref().filter(|(of, _)| wanted(of)).map(|(_, value)| value.clone())
    }

    /// The caller wrote `value` of `clip` itself: it is the answer now, and a read started
    /// before it does not overwrite it.
    pub fn set(&mut self, project: &str, clip: &str, value: T) {
        self.generation = self.generation.wrapping_add(1);
        let key = (project.to_string(), clip.to_string());
        self.due = Some((key.clone(), Instant::now() + self.every));
        self.current = Some((key, value));
    }

    /// Why the last read of `clip` failed, while no later read answered.
    pub fn error(&self, project: &str, clip: &str) -> Option<&str> {
        self.error.as_ref().filter(|((p, c), _)| p == project && c == clip).map(|(_, error)| error.as_str())
    }

    /// The value already known for `clip`, without reading.
    pub fn known(&self, project: &str, clip: &str) -> Option<&T> {
        self.current.as_ref().filter(|((p, c), _)| p == project && c == clip).map(|(_, value)| value)
    }

    fn take_read(&mut self) {
        let Some((key, generation, done)) = &self.reading else { return };
        match done.try_recv() {
            Ok(answer) => {
                match answer {
                    Ok(value) if *generation == self.generation => {
                        self.current = Some((key.clone(), value));
                        self.error = None;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("{} not read: {error}", self.name));
                        self.error = Some((key.clone(), error));
                    }
                }
                self.reading = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => self.reading = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn wait<T: Clone + Send + 'static>(follow: &mut ClipFollow<T>, read: impl Fn() -> Result<T, String> + Send + Sync + Clone + 'static) -> Option<T> {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            let read = read.clone();
            let value = follow.get("p", "c", true, move || move || read());
            if value.is_some() || Instant::now() > until {
                return value;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_shown_clip_is_read_again_and_a_failed_read_keeps_the_last_answer() {
        let mut follow = ClipFollow::new("test", Duration::from_millis(20));
        let db = Arc::new(AtomicU32::new(1));
        let reader = |db: Arc<AtomicU32>| move || match db.load(Ordering::SeqCst) {
            0 => Err("gone".to_string()),
            value => Ok(value),
        };
        assert_eq!(wait(&mut follow, reader(db.clone())), Some(1));
        db.store(2, Ordering::SeqCst);
        let until = Instant::now() + Duration::from_secs(2);
        while wait(&mut follow, reader(db.clone())) != Some(2) && Instant::now() < until {}
        assert_eq!(follow.known("p", "c"), Some(&2), "another writer's change shows");
        db.store(0, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(wait(&mut follow, reader(db.clone())), Some(2), "a failed read keeps the last answer");
        assert_eq!(follow.known("p", "other"), None);
    }

    #[test]
    fn a_value_the_caller_wrote_is_not_overwritten_by_an_older_read() {
        let mut follow: ClipFollow<u32> = ClipFollow::new("test", Duration::from_secs(60));
        let (go, gate) = mpsc::channel::<()>();
        let gate = std::sync::Mutex::new(Some(gate));
        assert_eq!(follow.get("p", "c", true, || {
            let gate = gate.lock().unwrap().take().unwrap();
            move || {
                let _ = gate.recv();
                Ok(1)
            }
        }), None);
        follow.set("p", "c", 7);
        go.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(follow.get("p", "c", true, || || Ok(9)), Some(7));
    }
}
