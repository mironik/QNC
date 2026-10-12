use super::*;

fn file_of(len: usize) -> (tempfile::TempDir, PathBuf, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.bin");
    let bytes: Vec<u8> = (0..len).map(|i| (i.wrapping_mul(31) ^ (i >> 11)) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    (dir, path, bytes)
}

fn read_exact_at(stream: &mut Stream, at: u64, len: usize) -> Vec<u8> {
    stream.seek(SeekFrom::Start(at)).unwrap();
    let mut out = vec![0u8; len];
    let mut done = 0;
    while done < len {
        let n = stream.read(&mut out[done..]).unwrap();
        if n == 0 {
            out.truncate(done);
            break;
        }
        done += n;
    }
    out
}

#[test]
fn streams_read_the_bytes_of_the_file_at_any_position() {
    let (_dir, path, bytes) = file_of(5 * BLOCK as usize + 12345);
    let reader = MediaReader::new(Config::default());
    let mut a = reader.open(&path).unwrap();
    let mut b = reader.open(&path).unwrap();
    assert_eq!(a.len(), bytes.len() as u64);
    for (at, len) in [(0u64, 100usize), (BLOCK - 7, 20), (3 * BLOCK + 5, 2 * BLOCK as usize), (5 * BLOCK, 20000)] {
        let want = &bytes[at as usize..(at as usize + len).min(bytes.len())];
        assert_eq!(read_exact_at(&mut a, at, len), want, "stream a at {at}");
        assert_eq!(read_exact_at(&mut b, at, len), want, "stream b at {at}");
    }
    assert_eq!(read_exact_at(&mut a, bytes.len() as u64 + 5, 10), Vec::<u8>::new(), "past the end");
    let end = a.seek(SeekFrom::End(-3)).unwrap();
    assert_eq!(end, bytes.len() as u64 - 3);
}

#[test]
fn the_reader_reads_ahead_so_the_next_read_waits_for_nothing() {
    let (_dir, path, bytes) = file_of(12 * BLOCK as usize);
    let reader = MediaReader::new(Config { ahead: 8 * BLOCK, ..Config::default() });
    let mut stream = reader.open(&path).unwrap();
    assert_eq!(read_exact_at(&mut stream, 0, 10), &bytes[..10]);
    let started = Instant::now();
    while reader.stats().bytes_read < 8 * BLOCK && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let waits = reader.stats().waits;
    let want = &bytes[6 * BLOCK as usize..7 * BLOCK as usize];
    assert_eq!(read_exact_at(&mut stream, 6 * BLOCK, BLOCK as usize), want);
    assert_eq!(reader.stats().waits, waits, "a block read ahead is served from memory");
    assert!(reader.stats().runs >= 2, "read in runs: {:?}", reader.stats());
}

#[test]
fn memory_stays_within_the_budget_and_goes_with_the_last_stream() {
    let (_dir, path, bytes) = file_of(20 * BLOCK as usize);
    let config = Config { ahead: 2 * BLOCK, behind: 0, budget: 4 * BLOCK, run_blocks: 2, demand_blocks: 1 };
    let reader = MediaReader::new(config);
    let mut stream = reader.open(&path).unwrap();
    for at in (0..20).map(|block| block * BLOCK) {
        assert_eq!(read_exact_at(&mut stream, at, 64), &bytes[at as usize..at as usize + 64]);
    }
    std::thread::sleep(Duration::from_millis(100));
    let cached = lock(&reader.shared).cached;
    assert!(cached <= 4 * BLOCK + config.run_blocks * BLOCK, "cached {cached}");
    drop(stream);
    let state = lock(&reader.shared);
    assert_eq!(state.cached, 0, "the file's memory goes with its last stream");
    assert!(state.files.is_empty() && state.by_path.is_empty());
}

#[test]
fn many_streams_on_many_files_read_their_own_bytes() {
    let files: Vec<_> = (0..3).map(|i| file_of(3 * BLOCK as usize + i * 777)).collect();
    let reader = MediaReader::new(Config::default());
    let threads: Vec<_> = files
        .iter()
        .flat_map(|(_, path, bytes)| (0..2).map(move |n| (path.clone(), bytes.clone(), n)))
        .map(|(path, bytes, n)| {
            let reader = reader.clone();
            std::thread::spawn(move || {
                let mut stream = reader.open(&path).unwrap();
                for step in 0..40u64 {
                    let at = (step * 104_729 + n * 50_000) % bytes.len() as u64;
                    let len = 70_000usize;
                    let end = (at as usize + len).min(bytes.len());
                    assert_eq!(read_exact_at(&mut stream, at, len), &bytes[at as usize..end]);
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn a_missing_file_is_an_error_of_open() {
    let reader = MediaReader::new(Config::default());
    assert!(reader.open(Path::new("no/such/media.mxf")).is_err());
}
