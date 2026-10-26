//! The sound the contract takes: 16 kHz mono 16-bit PCM in a WAV file.

use std::{fs::File, io::Read, io::Write, path::Path};

/// The rate speech models listen at.
pub const WHISPER_RATE: u32 = 16_000;

/// One channel of sound at `rate` brought to [`WHISPER_RATE`]: a windowed-sinc low-pass at the
/// new rate's band and the samples taken at their exact positions.
pub fn to_whisper_rate(samples: &[f32], rate: u32) -> Result<Vec<f32>, String> {
    if rate == 0 {
        return Err("sound without a sample rate".into());
    }
    if rate == WHISPER_RATE {
        return Ok(samples.to_vec());
    }
    let ratio = f64::from(rate) / f64::from(WHISPER_RATE);
    // Up to just under the new Nyquist frequency with a steep edge: speech keeps its consonants
    // up to 7.8 kHz (a short filter that rolled off from about 6 kHz made whisper hear
    // "Hvala." instead of speech, measured 2026-10-26).
    let cutoff = (1.0 / ratio).min(1.0) * 0.975;
    let half = (128.0 * ratio.max(1.0)).ceil() as i64;
    let count = ((samples.len() as f64) / ratio).floor() as usize;
    let weight = |x: f64| cutoff * sinc(cutoff * x) * blackman(x / half as f64);
    let mut out = Vec::with_capacity(count);
    if rate % WHISPER_RATE == 0 {
        // A whole ratio (48 kHz, 32 kHz): every output sample sits on an input sample, so one
        // set of weights serves them all.
        let step = (rate / WHISPER_RATE) as usize;
        let taps: Vec<f64> = (-half + 1..=half).map(|k| weight(-(k as f64))).collect();
        for n in 0..count {
            let centre = (n * step) as i64;
            let (mut sum, mut weights) = (0.0f64, 0.0f64);
            for (i, k) in (centre - half + 1..=centre + half).enumerate() {
                if k >= 0 && (k as usize) < samples.len() {
                    sum += f64::from(samples[k as usize]) * taps[i];
                    weights += taps[i];
                }
            }
            out.push(if weights.abs() > 1e-12 { (sum / weights) as f32 } else { 0.0 });
        }
        return Ok(out);
    }
    for n in 0..count {
        let t = n as f64 * ratio;
        let centre = t.floor() as i64;
        let (mut sum, mut weights) = (0.0f64, 0.0f64);
        for k in (centre - half + 1)..=(centre + half) {
            if k < 0 || k as usize >= samples.len() {
                continue;
            }
            let w = weight(t - k as f64);
            sum += f64::from(samples[k as usize]) * w;
            weights += w;
        }
        out.push(if weights.abs() > 1e-12 { (sum / weights) as f32 } else { 0.0 });
    }
    Ok(out)
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        px.sin() / px
    }
}

/// The Blackman window over -1..1.
fn blackman(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    let a = std::f64::consts::PI * (x + 1.0);
    0.42 - 0.5 * a.cos() + 0.08 * (2.0 * a).cos()
}

/// Writes 16 kHz mono samples as a 16-bit PCM WAV file.
pub fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let data_bytes = u32::try_from(samples.len() * 2).map_err(|_| "sound too long for one WAV file")?;
    let mut bytes = Vec::with_capacity(44 + data_bytes as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&WHISPER_RATE.to_le_bytes());
    bytes.extend_from_slice(&(WHISPER_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let mut file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads a WAV file the contract takes; any other format is an error.
pub fn read_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut at = 12;
    let mut format_ok = false;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]) as usize;
        let body = at + 8;
        let end = body.checked_add(size).filter(|end| *end <= bytes.len()).ok_or("a WAV chunk past the end of the file")?;
        match id {
            b"fmt " => {
                let field = |o: usize| u16::from_le_bytes([bytes[body + o], bytes[body + o + 1]]);
                if size < 16 {
                    return Err("a short WAV format chunk".into());
                }
                let rate = u32::from_le_bytes([bytes[body + 4], bytes[body + 5], bytes[body + 6], bytes[body + 7]]);
                format_ok = field(0) == 1 && field(2) == 1 && rate == WHISPER_RATE && field(14) == 16;
                if !format_ok {
                    return Err("the WAV file is not 16 kHz mono 16-bit PCM".into());
                }
            }
            b"data" => {
                if !format_ok {
                    return Err("WAV data before its format".into());
                }
                return Ok(bytes[body..end]
                    .chunks_exact(2)
                    .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
                    .collect());
            }
            _ => {}
        }
        at = end + (size & 1);
    }
    Err("a WAV file without data".into())
}
