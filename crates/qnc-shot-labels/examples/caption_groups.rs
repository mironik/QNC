//! The group each caption of a file gives by the installed list's caption rules (one caption a
//! line, optionally after "| "); reads only the two files.
fn main() {
    let mut args = std::env::args().skip(1);
    let file = args.next().expect("captions file");
    let list = qnc_shot_labels::Labels::read(&qnc_shot_labels::installed_path().or_else(|_| Ok::<_, String>(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(qnc_shot_labels::LIST))).unwrap()).unwrap();
    for line in std::fs::read_to_string(file).unwrap().lines() {
        let caption = line.split_once("| ").map_or(line, |(_, caption)| caption);
        let caption = caption.trim_start_matches("In this image we can see ");
        println!("{:<12} {}", list.group_of_caption(caption).unwrap_or("-"), &caption[..caption.len().min(90)]);
    }
}
