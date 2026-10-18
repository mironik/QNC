//! Writes the intra pictures nearest 13 places of an MXF original to OUT_DIR (read only
//! source): `cargo run --release -p qnc-mxf-index --example intra_pictures -- FILE OUT_DIR`.
use std::{fs::File, path::Path, time::Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("an MXF file");
    let out = args.next().expect("an output folder");
    let started = Instant::now();
    let index = qnc_mxf_index::MxfIndex::read(Path::new(&path)).unwrap().expect("an index");
    let frames = index.entries.len() as u64;
    let mut file = File::open(&path).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    for target in 0..13u64 {
        let entry = index.intra_nearest(target * frames / 13).unwrap();
        let package = qnc_mxf_index::read_package(&mut file, &index, entry, true, false).unwrap();
        std::fs::write(Path::new(&out).join(format!("{target:03}.h264")), package.picture.unwrap()).unwrap();
    }
    println!("{frames} frames, 13 pictures in {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
}
