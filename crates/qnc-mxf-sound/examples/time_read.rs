//! Times the sound of one MXF file (read only): `cargo run --release --example time_read -- FILE`.
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};

fn main() {
    let path = std::env::args().nth(1).expect("an MXF file");
    let size = std::fs::metadata(&path).expect("file").len();
    let started = Instant::now();
    let mut bytes = 0usize;
    qnc_mxf_sound::read_sound_elements(Path::new(&path), &AtomicBool::new(false), &mut |_, value| {
        bytes += value.len();
        Ok(())
    })
    .expect("sound");
    let seconds = started.elapsed().as_secs_f64();
    println!("{:.0} MB in {seconds:.2} s ({:.0} MB/s of file), sound {:.1} MB", size as f64 / 1e6, size as f64 / 1e6 / seconds, bytes as f64 / 1e6);
}
