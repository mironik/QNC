//! Writes the first and the last intra picture of an MXF original to OUT_DIR (read only
//! source): `cargo run --release -p qnc-mxf-index --example first_last -- FILE OUT_DIR`.
use std::{fs::File, path::Path, time::Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("an MXF file");
    let out = args.next().expect("an output folder");
    let started = Instant::now();
    let index = qnc_mxf_index::MxfIndex::read(Path::new(&path)).unwrap().expect("an index");
    let last = index.entries.len() as u64 - 1;
    let mut file = File::open(&path).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    for (name, frame) in [("first", 0), ("last", last)] {
        let entry = index.intra_nearest(frame).unwrap();
        let package = qnc_mxf_index::read_package(&mut file, &index, entry, true, false).unwrap();
        std::fs::write(Path::new(&out).join(format!("{name}.h264")), package.picture.unwrap()).unwrap();
    }
    println!("read {:.0} ms", started.elapsed().as_secs_f64() * 1000.0);
}
