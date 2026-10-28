//! Puts unit files together as an export does, with the rule of EBU R 128, into `OUT`.
//! `cargo run -p qnc-unit-join --example join_units -- FFMPEG OUT UNIT...` (reads the units only).

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let ffmpeg = std::path::PathBuf::from(args.next().ok_or("FFMPEG")?);
    let out = std::path::PathBuf::from(args.next().ok_or("OUT")?);
    let units: Vec<std::path::PathBuf> = args.map(Into::into).collect();
    let rule = qnc_unit_join::LoudnessRule { a1_lufs: -23.0, a2_lufs: -23.0, true_peak_dbtp: -1.0, tolerance_lu: 0.5 };
    let sound = qnc_unit_join::Sound { sample_rate: 48_000, channels: 2 };
    let joined = qnc_unit_join::join(&units, sound, rule, &ffmpeg, &out, &[], "10:00:00:00")?;
    println!("{:#?}", joined);
    Ok(())
}
