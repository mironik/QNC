//! Runs the medium test on one file (read only): `cargo run --release -p qnc-read-pace --example measure -- FILE`.
fn main() {
    let file = std::env::args().nth(1).expect("a media file");
    let started = std::time::Instant::now();
    let measured = qnc_read_pace::measure(std::path::Path::new(&file), 8, &std::sync::atomic::AtomicBool::new(false));
    println!("{measured:?} in {:.2} s", started.elapsed().as_secs_f64());
}
