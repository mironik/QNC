//! Cuts frames FROM..TO of an MXF original (read only) into DEST:
//! `cargo run --release -p qnc-mxf-cut --example mxf_cut -- SOURCE FROM TO DEST`.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (source, from, to, dest) = (std::path::Path::new(&args[0]), args[1].parse().unwrap(), args[2].parse().unwrap(), std::path::Path::new(&args[3]));
    let started = std::time::Instant::now();
    let index = qnc_mxf_index::MxfIndex::read(source).unwrap().expect("an index");
    let plan = qnc_mxf_cut::plan(&index, from, to).unwrap();
    qnc_mxf_cut::write(source, &index, &plan, dest, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    println!("first {} frames {} in {:.2} s", plan.first_frame, plan.frames, started.elapsed().as_secs_f64());
}
