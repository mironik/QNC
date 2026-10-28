//! Times reading one sound element over a range of frames (the file is only read).
//! Usage: element_read MXF ELEMENT SAMPLE_BYTES FROM TO

use std::{sync::atomic::AtomicBool, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path, element, bytes, from, to] = args.as_slice() else { return Err("usage: element_read MXF ELEMENT SAMPLE_BYTES FROM TO".into()) };
    let number = |text: &str| text.parse::<u64>().map_err(|_| format!("{text} is not a number"));
    let started = Instant::now();
    let samples = qnc_mxf_sound::read_element_frames(std::path::Path::new(path), number(element)? as u8, number(bytes)? as usize, number(from)?, number(to)?, &AtomicBool::new(false))?;
    let peak = samples.iter().fold(0f32, |peak, sample| peak.max(sample.abs()));
    println!("{} samples ({:.1} s at 48 kHz), peak {peak:.3}, read in {:.2} s", samples.len(), samples.len() as f64 / 48_000.0, started.elapsed().as_secs_f64());
    Ok(())
}
