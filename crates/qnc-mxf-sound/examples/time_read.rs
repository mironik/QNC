//! Times the sound of one MXF file (read only): `cargo run --release --example time_read -- FILE`.
//! With `CODEC FRAMES OUT_DIR` it also takes the 13 filmstrip key pictures in the same pass
//! and writes them to `OUT_DIR` (`000.CODEC`...), never next to the media.
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("an MXF file");
    let pictures = args.next().map(|codec| {
        let frames: u64 = args.next().expect("the picture count").parse().expect("a number");
        (codec, frames, args.next().expect("an output folder"))
    });
    let size = std::fs::metadata(&path).expect("file").len();
    let started = Instant::now();
    let mut bytes = 0usize;
    let targets: Vec<u64> = pictures.as_ref().map_or(Vec::new(), |(_, frames, _)| (0..13).map(|index| index * frames / 13).collect());
    let is_key = pictures
        .as_ref()
        .map(|(codec, _, _)| qnc_mxf_sound::key_picture_test(codec).expect("a known codec"))
        .unwrap_or(|_| false);
    let mut found = Vec::new();
    qnc_mxf_sound::read_sound_and_pictures(
        Path::new(&path),
        &AtomicBool::new(false),
        &targets,
        &is_key,
        &mut |_, value| {
            bytes += value.len();
            Ok(())
        },
        &mut |target, number, value| {
            found.push((target, number, value.to_vec()));
            Ok(())
        },
    )
    .expect("sound");
    let seconds = started.elapsed().as_secs_f64();
    println!("{:.0} MB in {seconds:.2} s ({:.0} MB/s of file), sound {:.1} MB, pictures {}", size as f64 / 1e6, size as f64 / 1e6 / seconds, bytes as f64 / 1e6, found.len());
    if let Some((codec, _, out)) = pictures {
        std::fs::create_dir_all(&out).expect("output folder");
        for (target, number, value) in found {
            println!("  frame target {} -> key picture {number} ({} KB)", targets[target], value.len() / 1024);
            std::fs::write(Path::new(&out).join(format!("{target:03}.{codec}")), value).expect("write");
        }
    }
}
