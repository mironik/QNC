//! Shot analysis of one MXF original, printed (the file is only read; nothing is written but
//! the decoder's small pictures in the system temp folder, removed after).
//! `cargo run --release -p qnc-shot-analysis --example analyse_file -- <file.mxf> [h264|mpeg2video]`

use std::sync::atomic::AtomicBool;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().expect("an MXF file"));
    let codec = args.next().unwrap_or_else(|| "h264".into());
    let cancel = AtomicBool::new(false);
    let pace = qnc_shot_analysis::Pace { player_plays: &|| false, wanted: &|| None, cancel: &cancel };
    let started = std::time::Instant::now();
    let found = qnc_shot_analysis::analyse_file("example", &codec, &path, &pace).expect("analysis");
    println!("pictures={} shots={} parts={} ms={}", found.pictures, found.shots.len(), found.parts.len(), started.elapsed().as_millis());
    for shot in found.shots {
        println!("shot {:>6} - {:>6}  key={:>6}  bright={:>5.1} sharp={:>8.1} motion={:.3}  {:?}", shot.from, shot.to, shot.key_frame, shot.brightness, shot.sharpness, shot.motion, shot.looks);
    }
    for part in found.parts {
        println!("part {:>6} - {:>6}  key={:>6}  {}", part.from, part.to, part.key_frame, part.camera);
    }
}
