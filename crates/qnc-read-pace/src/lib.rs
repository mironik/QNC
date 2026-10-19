//! How many readers of a medium run at once, found by one short test of the medium (user
//! 2026-10-18: "your own short test of the reading speed, and from it the number of cores and
//! threads; after that no more testing"). The test reads small blocks spread through one
//! media file, as a pass does (it reads the sound of each frame and skips the picture), with
//! 1, 2, 4 and 8 readers for a moment each, and takes the fewest readers that read within 5 %
//! of the best. A USB 2.0 card comes out low, a USB 3 stick or an SSD high. It only
//! reads, never writes, and runs only as long as the test. On Windows the test reads past
//! the system's file cache (live 2026-10-18: a stick read earlier came out at memory speed
//! with one reader); elsewhere it reads as the passes do.

use std::{
    fs::File,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// One read of the test: about what a pass reads for a frame's sound.
const BLOCK: usize = 64 * 1024;
/// The step between two reads of one reader: a pass skips the picture between them.
const STRIDE: u64 = 1024 * 1024;
/// How long each level reads.
const LEVEL_TIME: Duration = Duration::from_millis(300);
/// The levels tried.
const LEVELS: [usize; 4] = [1, 2, 4, 8];
/// A file smaller than this cannot hold the test.
pub const LEAST_FILE: u64 = 64 * 1024 * 1024;
/// More readers stay only when the fewer do not read at least this share of the best (the first level reads a little cold).
const NEAR_BEST: f64 = 0.95;

/// What the test found.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// How many readers to run.
    pub readers: usize,
    /// Bytes per second read at each level tried.
    pub rates: Vec<(usize, f64)>,
}

/// Tests the medium of `file` with at most `most` readers. `None` when the file is too
/// small to test (another file can be tried) or the test was cancelled.
pub fn measure(file: &Path, most: usize, cancel: &AtomicBool) -> Result<Option<Measured>, String> {
    let length = std::fs::metadata(file).map_err(|error| format!("read pace: {error}"))?.len();
    if length < LEAST_FILE {
        return Ok(None);
    }
    let levels: Vec<usize> = LEVELS.iter().copied().filter(|level| *level <= most.max(1)).collect();
    // Each reader of each level reads its own part of the file, so no level reads what an
    // earlier one already brought into memory.
    let parts = levels.iter().sum::<usize>() as u64;
    let part = length / parts;
    // A device asleep takes seconds for its first read (live 2026-10-19: a CFexpress reader
    // gave 0 MB/s for one reader): one read wakes it before anything is measured.
    wake(file)?;
    let mut first_part = 0u64;
    let mut rates = Vec::new();
    for &level in &levels {
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        let starts: Vec<u64> = (0..level as u64).map(|index| (first_part + index) * part).collect();
        first_part += level as u64;
        let started = Instant::now();
        let bytes: Result<Vec<u64>, String> = std::thread::scope(|scope| {
            let readers: Vec<_> = starts
                .iter()
                .map(|&start| scope.spawn(move || read_part(file, start, part, started)))
                .collect();
            readers
                .into_iter()
                .map(|reader| reader.join().map_err(|_| "read pace: a reader stopped".to_string())?)
                .collect()
        });
        let seconds = started.elapsed().as_secs_f64();
        rates.push((level, bytes?.iter().sum::<u64>() as f64 / seconds));
    }
    Ok(Some(Measured { readers: choose(&rates), rates }))
}

/// The fewest readers that read within 5 % of the best.
fn choose(rates: &[(usize, f64)]) -> usize {
    let best = rates.iter().map(|(_, rate)| *rate).fold(0.0, f64::max);
    rates
        .iter()
        .find(|(_, rate)| *rate >= best * NEAR_BEST)
        .map_or(1, |(level, _)| *level)
}

/// One read past the cache at the start of the file.
fn wake(file: &Path) -> Result<(), String> {
    let handle = open_uncached(file).map_err(|error| format!("read pace: {error}"))?;
    let mut buffer = vec![0u8; BLOCK + ALIGN];
    let skip = buffer.as_ptr().align_offset(ALIGN);
    read_at(&handle, &mut buffer[skip..skip + BLOCK], 0).map_err(|error| format!("read pace: {error}"))?;
    Ok(())
}

fn read_part(file: &Path, start: u64, length: u64, started: Instant) -> Result<u64, String> {
    let file = open_uncached(file).map_err(|error| format!("read pace: {error}"))?;
    // Reads past the cache must start on a sector and land in memory aligned to one.
    let mut buffer = vec![0u8; BLOCK + ALIGN];
    let skip = buffer.as_ptr().align_offset(ALIGN);
    let block = &mut buffer[skip..skip + BLOCK];
    let mut offset = start / ALIGN as u64 * ALIGN as u64;
    let mut read = 0u64;
    while started.elapsed() < LEVEL_TIME && offset + BLOCK as u64 <= start + length {
        read += read_at(&file, block, offset).map_err(|error| format!("read pace: {error}"))? as u64;
        offset += STRIDE;
    }
    Ok(read)
}

/// A sector size every medium in use divides.
const ALIGN: usize = 4096;

#[cfg(windows)]
fn open_uncached(file: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    std::fs::OpenOptions::new().read(true).custom_flags(FILE_FLAG_NO_BUFFERING).open(file)
}

#[cfg(not(windows))]
fn open_uncached(file: &Path) -> std::io::Result<File> {
    File::open(file)
}

#[cfg(unix)]
fn read_at(file: &File, block: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, block, offset)
}

#[cfg(windows)]
fn read_at(file: &File, block: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, block, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fewest_readers_near_the_best_are_chosen() {
        // A USB 3 stick: 1, 2 and 4 readers grow, 8 adds almost nothing.
        assert_eq!(choose(&[(1, 62.0), (2, 94.0), (4, 158.0), (8, 165.0)]), 4);
        // A USB 2.0 card: one reader already reads all it can.
        assert_eq!(choose(&[(1, 28.0), (2, 28.9), (4, 28.5), (8, 27.0)]), 1);
        // An SSD: every level adds.
        assert_eq!(choose(&[(1, 300.0), (2, 600.0), (4, 1100.0), (8, 1900.0)]), 8);
    }

    #[test]
    fn a_small_file_is_not_tested_and_nothing_is_written() {
        let dir = std::env::temp_dir().join(format!("qnc-read-pace-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("small.bin");
        std::fs::write(&file, vec![0u8; 1024]).unwrap();
        assert_eq!(measure(&file, 8, &AtomicBool::new(false)).unwrap(), None);
        assert_eq!(std::fs::metadata(&file).unwrap().len(), 1024);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_large_file_is_measured_at_every_level_up_to_the_most() {
        let dir = std::env::temp_dir().join(format!("qnc-read-pace-large-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("large.bin");
        let handle = File::create(&file).unwrap();
        handle.set_len(LEAST_FILE).unwrap();
        drop(handle);
        let measured = measure(&file, 4, &AtomicBool::new(false)).unwrap().expect("measured");
        assert_eq!(measured.rates.iter().map(|(level, _)| *level).collect::<Vec<_>>(), vec![1, 2, 4]);
        assert!((1..=4).contains(&measured.readers));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
