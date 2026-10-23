//! Cuts frames FROM..TO of an MP4/QuickTime original (read only) at RATE (50/1) into DEST:
//! `cargo run -p qnc-mp4-cut --example mp4_cut -- FFMPEG SOURCE FROM TO NUM DEN DEST`.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (ffmpeg, source) = (std::path::Path::new(&a[0]), std::path::Path::new(&a[1]));
    let (from, to, rate) = (a[2].parse().unwrap(), a[3].parse().unwrap(), (a[4].parse().unwrap(), a[5].parse().unwrap()));
    let first = qnc_mp4_cut::intra_at_or_before(ffmpeg, source, from, rate).unwrap();
    qnc_mp4_cut::write(ffmpeg, source, std::path::Path::new(&a[6]), (first, to), rate, Some("01:00:00:00"), &Default::default()).unwrap();
    println!("first {first}");
}
