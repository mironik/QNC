//! One reader of the media files of a process for every decoder in it (user 2026-10-12:
//! breaking news is edited straight from the camera card, copying is not an option). The
//! card sits in a USB 2.0 reader of about 29 MB/s; one camera original needs about
//! 15 MB/s, so on average a story fits, but every decoder reading its own file at its own
//! moment made bursts of four to six random reads at each cut and at the start of play,
//! and the card fell far below its rate. Here one thread reads the media, one read at a
//! time, in large sequential runs: first the block a decoder waits for, then ahead of
//! every open stream, the one with the least read ahead first. Decoders read the blocks
//! from memory.
//!
//! Only reading: nothing is written, created, renamed or removed (a card is read only).
//! No decoding, no probe, no database; it does not know what the bytes are.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

/// One block of a file in memory. Small, so a block a decoder waits for after a jump is
/// read quickly (256 KiB about 9 ms on the USB 2.0 card reader); reading ahead reads many
/// in one run.
pub const BLOCK: u64 = 256 << 10;

/// How the reader reads and how much it keeps.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Blocks read in one run ahead at most (a long read is what the card does fastest).
    pub run_blocks: u64,
    /// Blocks read in one run for a stream that waits (short: it waits for all of it).
    pub demand_blocks: u64,
    /// Bytes read ahead of each open stream (about 2 s of a camera original).
    pub ahead: u64,
    /// Bytes kept behind each stream's position (a decoder seeks back a little).
    pub behind: u64,
    /// Bytes in memory at most; blocks outside every stream's window go first.
    pub budget: u64,
    /// Below this many bytes ahead a stream is read at the full rate of the media; above
    /// it reading ahead is paced to `paced_rate`, so the media is shared with what reads it
    /// beside this reader (the sound decoders: a cover's read ahead at the full rate of the
    /// card starved them and the sound ran dry before cuts, live 2026-10-12).
    pub low_water: u64,
    /// Bytes per second read ahead at most while every stream has `low_water` ahead.
    pub paced_rate: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            run_blocks: 4,
            demand_blocks: 2,
            ahead: 32 << 20,
            behind: 8 << 20,
            budget: 768 << 20,
            low_water: 8 << 20,
            paced_rate: 20 << 20,
        }
    }
}

/// What the reader has done (diagnostics).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Bytes read from the media.
    pub bytes_read: u64,
    /// Runs read (one read call each).
    pub runs: u64,
    /// Times a stream had to wait for a block that was not in memory.
    pub waits: u64,
    /// The time those streams waited, in total (microseconds).
    pub waited_us: u64,
}

struct Block {
    data: Arc<[u8]>,
    used: u64,
}

struct FileState {
    file: Arc<File>,
    len: u64,
    blocks: HashMap<u64, Block>,
    streams: usize,
}

struct StreamState {
    file: u64,
    pos: u64,
    ahead: u64,
}

#[derive(Default)]
struct State {
    files: HashMap<u64, FileState>,
    by_path: HashMap<PathBuf, u64>,
    streams: HashMap<u64, StreamState>,
    demand: VecDeque<(u64, u64)>,
    inflight: HashSet<(u64, u64)>,
    failed: HashMap<(u64, u64), String>,
    next_id: u64,
    cached: u64,
    tick: u64,
    stats: Stats,
}

struct Shared {
    config: Config,
    state: Mutex<State>,
    work: Condvar,
    ready: Condvar,
}

/// The reader of a process: one thread reads, every stream reads from memory.
#[derive(Clone)]
pub struct MediaReader {
    shared: Arc<Shared>,
    /// While a copy of the reader lives its threads run (streams alone do not keep them).
    _owner: Arc<()>,
}

impl MediaReader {
    /// The reader of this process, started on first use.
    pub fn shared() -> &'static MediaReader {
        static READER: OnceLock<MediaReader> = OnceLock::new();
        READER.get_or_init(|| MediaReader::new(Config::default()))
    }

    /// A reader of its own (tests, or a host that wants other limits).
    pub fn new(config: Config) -> Self {
        let shared = Arc::new(Shared {
            config,
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            ready: Condvar::new(),
        });
        // Two threads: one reads only the blocks a stream waits for, so a decoder that
        // opens or jumps never waits behind a long read ahead (opening a file waited
        // ~140 ms a read behind 4 MB runs and stalled the player, 2026-10-12); the other
        // reads ahead, and takes waiting blocks first too when it is free.
        let owner = Arc::new(());
        for (name, ahead) in [("qnc-media-reader-wait", false), ("qnc-media-reader-ahead", true)] {
            let (worker, alive) = (Arc::downgrade(&shared), Arc::downgrade(&owner));
            std::thread::Builder::new()
                .name(name.into())
                .spawn(move || run(worker, alive, ahead))
                .expect("media reader thread");
        }
        Self { shared, _owner: owner }
    }

    /// A stream over a file, read through this reader; the file is opened for reading
    /// only. Streams of the same file share its blocks.
    pub fn open(&self, path: &Path) -> io::Result<Stream> {
        // A file not open yet is opened outside the lock: a card that wakes takes seconds,
        // and every decoder of the process waits on the lock (audit 2026-10-25).
        let known = lock(&self.shared).by_path.contains_key(path);
        let opened = match known {
            true => None,
            false => {
                let handle = File::open(path)?;
                let len = handle.metadata()?.len();
                Some((handle, len))
            }
        };
        let mut state = lock(&self.shared);
        let file = match (state.by_path.get(path).copied(), opened) {
            (Some(file), _) => file,
            // Gone from the map while it was being opened (a failed read): open it now.
            (None, None) => {
                drop(state);
                return self.open(path);
            }
            (None, Some((handle, len))) => {
                state.next_id += 1;
                let id = state.next_id;
                state.files.insert(
                    id,
                    FileState {
                        file: Arc::new(handle),
                        len,
                        blocks: HashMap::new(),
                        streams: 0,
                    },
                );
                state.by_path.insert(path.to_path_buf(), id);
                id
            }
        };
        let len = {
            let entry = state.files.get_mut(&file).expect("file of the stream");
            entry.streams += 1;
            entry.len
        };
        state.next_id += 1;
        let id = state.next_id;
        let ahead = self.shared.config.ahead;
        state.streams.insert(id, StreamState { file, pos: 0, ahead });
        drop(state);
        self.shared.work.notify_all();
        Ok(Stream {
            shared: self.shared.clone(),
            id,
            file,
            len,
            pos: 0,
        })
    }

    pub fn stats(&self) -> Stats {
        lock(&self.shared).stats
    }
}

/// A position in one file, read through the reader. Each decoder has its own.
pub struct Stream {
    shared: Arc<Shared>,
    id: u64,
    file: u64,
    len: u64,
    pos: u64,
}

impl Stream {
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes read ahead of this stream (0 keeps nothing ahead).
    pub fn set_ahead(&self, bytes: u64) {
        if let Some(stream) = lock(&self.shared).streams.get_mut(&self.id) {
            stream.ahead = bytes;
        }
        self.shared.work.notify_all();
    }

    /// Reads at the stream's position from the blocks in memory, waiting for the reader
    /// when the block is not there yet.
    fn read_here(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let block = self.pos / BLOCK;
        let mut state = lock(&self.shared);
        if let Some(stream) = state.streams.get_mut(&self.id) {
            stream.pos = self.pos;
        }
        let mut waited: Option<Instant> = None;
        let data = loop {
            state.tick += 1;
            let tick = state.tick;
            let found = state
                .files
                .get_mut(&self.file)
                .and_then(|file| file.blocks.get_mut(&block))
                .map(|entry| {
                    entry.used = tick;
                    entry.data.clone()
                });
            if let Some(data) = found {
                break data;
            }
            if let Some(message) = state.failed.remove(&(self.file, block)) {
                return Err(io::Error::other(message));
            }
            if waited.is_none() {
                waited = Some(Instant::now());
                state.stats.waits += 1;
            }
            if !state.inflight.contains(&(self.file, block)) && !state.demand.contains(&(self.file, block)) {
                state.demand.push_back((self.file, block));
            }
            self.shared.work.notify_all();
            state = self
                .shared
                .ready
                .wait_timeout(state, Duration::from_secs(1))
                .map(|(state, _)| state)
                .unwrap_or_else(|poisoned| poisoned.into_inner().0);
        };
        if let Some(started) = waited {
            state.stats.waited_us += u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        }
        drop(state);
        let offset = usize::try_from(self.pos - block * BLOCK).unwrap_or(0);
        let n = buf.len().min(data.len().saturating_sub(offset));
        buf[..n].copy_from_slice(&data[offset..offset + n]);
        self.pos += n as u64;
        // The window moved: the reader reads further ahead.
        self.shared.work.notify_all();
        Ok(n)
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_here(buf)
    }
}

impl Seek for Stream {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(at) => i128::from(at),
            SeekFrom::Current(by) => i128::from(self.pos) + i128::from(by),
            SeekFrom::End(by) => i128::from(self.len) + i128::from(by),
        };
        let target = u64::try_from(target).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        self.pos = target;
        if let Some(stream) = lock(&self.shared).streams.get_mut(&self.id) {
            stream.pos = target;
        }
        self.shared.work.notify_all();
        Ok(target)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        let mut state = lock(&self.shared);
        state.streams.remove(&self.id);
        let last = state.files.get_mut(&self.file).is_some_and(|file| {
            file.streams = file.streams.saturating_sub(1);
            file.streams == 0
        });
        if last {
            // Nobody reads the file any more: its memory and handle go.
            if let Some(file) = state.files.remove(&self.file) {
                let freed: u64 = file.blocks.values().map(|block| block.data.len() as u64).sum();
                state.cached = state.cached.saturating_sub(freed);
            }
            state.by_path.retain(|_, id| *id != self.file);
            let file = self.file;
            state.demand.retain(|(id, _)| *id != file);
            state.failed.retain(|(id, _), _| *id != file);
        }
    }
}

fn lock(shared: &Shared) -> MutexGuard<'_, State> {
    shared.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One read the reader thread does: `count` blocks of a file from `first`.
struct Job {
    /// A run ahead of a stream that already has `low_water` ahead (paced).
    paced: bool,
    file_id: u64,
    file: Arc<File>,
    len: u64,
    first: u64,
    count: u64,
}

fn run(shared: std::sync::Weak<Shared>, alive: std::sync::Weak<()>, reads_ahead: bool) {
    loop {
        let Some(strong) = shared.upgrade() else {
            return;
        };
        let job = {
            let mut state = lock(&strong);
            loop {
                if let Some(job) = pick(&strong.config, &mut state, reads_ahead) {
                    break job;
                }
                // Waking now and then lets the thread end once its reader is gone.
                state = strong
                    .work
                    .wait_timeout(state, Duration::from_millis(500))
                    .map(|(state, _)| state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner().0);
                if alive.strong_count() == 0 {
                    return;
                }
            }
        };
        let started = Instant::now();
        let read = read_run(&job);
        let mut state = lock(&strong);
        for block in job.first..job.first + job.count {
            state.inflight.remove(&(job.file_id, block));
        }
        match read {
            Ok(bytes) => {
                state.stats.bytes_read += bytes.len() as u64;
                state.stats.runs += 1;
                state.tick += 1;
                let tick = state.tick;
                let mut added = 0u64;
                if let Some(file) = state.files.get_mut(&job.file_id) {
                    for (index, chunk) in bytes.chunks(BLOCK as usize).enumerate() {
                        let block = job.first + index as u64;
                        if !file.blocks.contains_key(&block) {
                            added += chunk.len() as u64;
                            file.blocks.insert(block, Block { data: chunk.into(), used: tick });
                        }
                    }
                }
                state.cached += added;
                evict(&strong.config, &mut state);
            }
            Err(error) => {
                let message = format!("media read failed: {error}");
                for block in job.first..job.first + job.count {
                    if state.files.contains_key(&job.file_id) {
                        state.failed.insert((job.file_id, block), message.clone());
                    }
                }
                // The handle may be dead (the card gone for a moment): a stream opened after
                // this opens the file again instead of sharing it (audit 2026-10-25).
                state.by_path.retain(|_, id| *id != job.file_id);
            }
        }
        drop(state);
        strong.ready.notify_all();
        // Paced: the run takes at least as long as it would at the paced rate (after its
        // blocks are in memory, so nobody waits for them meanwhile).
        if job.paced && strong.config.paced_rate > 0 {
            let due = Duration::from_secs_f64((job.count * BLOCK) as f64 / strong.config.paced_rate as f64);
            if let Some(rest) = due.checked_sub(started.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    }
}

/// The next read: a block a stream waits for first, then ahead of the stream with the
/// least read ahead of it.
fn pick(config: &Config, state: &mut State, reads_ahead: bool) -> Option<Job> {
    while let Some((file_id, block)) = state.demand.pop_front() {
        let Some(file) = state.files.get(&file_id) else {
            continue;
        };
        if file.blocks.contains_key(&block) || state.inflight.contains(&(file_id, block)) {
            continue;
        }
        return Some(job(config.demand_blocks, state, file_id, block, u64::MAX));
    }
    if !reads_ahead {
        return None;
    }
    let mut best: Option<(u64, u64, u64, u64)> = None; // (read ahead, file, first missing, window end)
    for stream in state.streams.values() {
        let Some(file) = state.files.get(&stream.file) else {
            continue;
        };
        if stream.ahead == 0 || stream.pos >= file.len {
            continue;
        }
        let end = stream.pos.saturating_add(stream.ahead).min(file.len);
        let mut block = stream.pos / BLOCK;
        while block * BLOCK < end
            && (file.blocks.contains_key(&block) || state.inflight.contains(&(stream.file, block)))
        {
            block += 1;
        }
        if block * BLOCK >= end {
            continue;
        }
        let ahead = (block * BLOCK).saturating_sub(stream.pos);
        if best.is_none_or(|(least, ..)| ahead < least) {
            best = Some((ahead, stream.file, block, end));
        }
    }
    let (ahead, file_id, block, end) = best?;
    let last = end.div_ceil(BLOCK);
    let mut job = job(config.run_blocks, state, file_id, block, last);
    job.paced = ahead >= config.low_water;
    Some(job)
}

/// A run from `first`: the following missing blocks too, up to the run length, the end
/// of the file and `last` (exclusive).
fn job(run_blocks: u64, state: &mut State, file_id: u64, first: u64, last: u64) -> Job {
    let file = &state.files[&file_id];
    let blocks_in_file = file.len.div_ceil(BLOCK);
    let mut count = 1;
    while count < run_blocks.max(1)
        && first + count < blocks_in_file.min(last)
        && !file.blocks.contains_key(&(first + count))
        && !state.inflight.contains(&(file_id, first + count))
    {
        count += 1;
    }
    let job = Job {
        paced: false,
        file_id,
        file: file.file.clone(),
        len: file.len,
        first,
        count,
    };
    for block in first..first + count {
        state.inflight.insert((file_id, block));
    }
    job
}

fn read_run(job: &Job) -> io::Result<Vec<u8>> {
    let start = job.first * BLOCK;
    let end = ((job.first + job.count) * BLOCK).min(job.len);
    let mut bytes = vec![0u8; usize::try_from(end.saturating_sub(start)).map_err(io::Error::other)?];
    let mut done = 0usize;
    while done < bytes.len() {
        let n = read_at(&job.file, &mut bytes[done..], start + done as u64)?;
        if n == 0 {
            bytes.truncate(done);
            break;
        }
        done += n;
    }
    Ok(bytes)
}

#[cfg(windows)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::windows::fs::FileExt;
    file.seek_read(buf, offset)
}

#[cfg(unix)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::unix::fs::FileExt;
    file.read_at(buf, offset)
}

/// Over the budget: the least recently used blocks outside every stream's window go.
fn evict(config: &Config, state: &mut State) {
    if state.cached <= config.budget {
        return;
    }
    let windows: Vec<(u64, u64, u64)> = state
        .streams
        .values()
        .map(|stream| {
            (
                stream.file,
                stream.pos.saturating_sub(config.behind) / BLOCK,
                stream.pos.saturating_add(stream.ahead).div_ceil(BLOCK),
            )
        })
        .collect();
    let mut outside: Vec<(u64, u64, u64)> = Vec::new(); // (used, file, block)
    for (file_id, file) in &state.files {
        for (block, entry) in &file.blocks {
            let wanted = windows
                .iter()
                .any(|(file, from, to)| file == file_id && block >= from && block < to);
            if !wanted {
                outside.push((entry.used, *file_id, *block));
            }
        }
    }
    outside.sort_unstable();
    for (_, file_id, block) in outside {
        if state.cached <= config.budget {
            break;
        }
        if let Some(entry) = state.files.get_mut(&file_id).and_then(|file| file.blocks.remove(&block)) {
            state.cached = state.cached.saturating_sub(entry.data.len() as u64);
        }
    }
}

#[cfg(test)]
mod tests;
