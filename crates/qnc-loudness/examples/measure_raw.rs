//! Measures mono f32le samples from stdin (e.g. `ffmpeg ... -ac 1 -f f32le -`): integrated
//! loudness and true peak, to compare with another meter. Reads stdin only.
use std::io::Read;

fn main() {
    let rate: u32 = std::env::args().nth(1).and_then(|arg| arg.parse().ok()).unwrap_or(48_000);
    let mut meter = qnc_loudness::Meter::new(rate).expect("rate");
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes).expect("stdin");
    for sample in bytes.chunks_exact(4) {
        meter.push(f32::from_le_bytes(sample.try_into().unwrap()));
    }
    let kept = meter.finish();
    let energies: Vec<f64> = kept.energies.iter().map(|e| f64::from(*e)).collect();
    let peak = kept.true_peaks.iter().fold(0f32, |a, b| a.max(*b));
    println!(
        "integrated {:.2} LUFS, true peak {:.2} dBTP, {} segments",
        qnc_loudness::integrated(&energies).unwrap_or(f64::NEG_INFINITY),
        qnc_loudness::peak_db(f64::from(peak)),
        kept.energies.len()
    );
}
