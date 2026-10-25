//! Rendered units of a story put together into one file (user 2026-10-24, Express export):
//! the pictures and the sound are copied as they are, in the order given, by the ffmpeg of the
//! installed decoder catalog (the prerender's toolchain). Before they are put together their
//! sound is measured (EBU R 128 / BS.1770-4, `qnc-loudness`): A1 (output 1) and A2 (output 2)
//! keep the user's levels when they are within the target's tolerance and under the true peak
//! limit, otherwise each gets the gain that brings it to its target (lowered so its true peak
//! stays at the limit). The file is MXF OP1a (user 2026-10-24: TV houses prefer it), the
//! pictures copied, every sound output its own mono PCM 24-bit track (as the cameras write
//! them; an output without gain is copied sample for sample). Every other output is kept
//! as it is. It reads the unit files only and writes one file, `out`, under its final name (a
//! project folder allows no rename); the list of the units waits in the system's temporary
//! folder. It reads no database and decodes no picture.

use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const MODULE_ID: &str = "qnc.module.unit-join";

type Result<T> = std::result::Result<T, String>;

/// The loudness rule of the project (`audio.loudness`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessRule {
    pub a1_lufs: f64,
    pub a2_lufs: f64,
    pub true_peak_dbtp: f64,
    pub tolerance_lu: f64,
}

/// One lane as it was measured and what the export gave it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneOutcome {
    pub measured_lufs: Option<f64>,
    pub true_peak_dbtp: f64,
    pub gain_db: f64,
}

/// What was written.
#[derive(Debug, Clone, PartialEq)]
pub struct Joined {
    pub path: PathBuf,
    pub a1: LaneOutcome,
    pub a2: LaneOutcome,
}

/// The sound of the units: rate and the number of outputs (A1, A2, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sound {
    pub sample_rate: u32,
    pub channels: u16,
}

/// Puts `units` (MOV files, one picture stream and one PCM stream each) together into `out`
/// (MXF). `picture` empty copies the pictures; else it is how they are encoded once (an
/// interlaced delivery from its lossless progressive units, user 2026-10-24). `start_timecode`
/// (HH:MM:SS:FF, the project's setting) is the timecode of the first frame of the file.
pub fn join(units: &[PathBuf], sound: Sound, rule: LoudnessRule, ffmpeg: &Path, out: &Path, picture: &[String], start_timecode: &str) -> Result<Joined> {
    if units.is_empty() {
        return Err("No unit to put together.".into());
    }
    if sound.channels < 2 {
        return Err(format!("The story has {} sound outputs; A1 and A2 need two.", sound.channels));
    }
    let list = std::env::temp_dir().join(format!("qnc-unit-join-{}-{}.txt", std::process::id(), nanos()));
    std::fs::write(&list, list_text(units)).map_err(|e| format!("{}: {e}", list.display()))?;
    let result = measure(&list, sound, ffmpeg).and_then(|measured| {
        let lane = |index: usize, target: f64| {
            let (lufs, peak) = measured[index];
            LaneOutcome { measured_lufs: lufs, true_peak_dbtp: peak, gain_db: qnc_loudness::export_gain(lufs, peak, target, rule.tolerance_lu, rule.true_peak_dbtp) }
        };
        let (a1, a2) = (lane(0, rule.a1_lufs), lane(1, rule.a2_lufs));
        write(&list, sound, [a1.gain_db, a2.gain_db], ffmpeg, out, picture, start_timecode)?;
        Ok(Joined { path: out.to_path_buf(), a1, a2 })
    });
    let _ = std::fs::remove_file(&list);
    result
}

/// The concat list: every unit once, in order (single quotes are literal in it).
fn list_text(units: &[PathBuf]) -> String {
    units
        .iter()
        .map(|unit| format!("file '{}'\n", unit.to_string_lossy().replace('\\', "/").replace('\'', "'\\''")))
        .collect()
}

/// The integrated loudness and the true peak of each output of the joined sound.
fn measure(list: &Path, sound: Sound, ffmpeg: &Path) -> Result<Vec<(Option<f64>, f64)>> {
    let mut child = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-f", "concat", "-safe", "0", "-i"])
        .arg(list)
        .args(["-map", "0:a:0", "-vn", "-c:a", "pcm_f32le", "-f", "f32le", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("The sound of the units cannot be read: {e}"))?;
    let mut meters = (0..sound.channels).map(|_| qnc_loudness::Meter::new(sound.sample_rate)).collect::<Result<Vec<_>>>()?;
    let mut stdout = child.stdout.take().ok_or("no sound output")?;
    let (mut buffer, mut carry, mut channel) = (vec![0u8; 1 << 16], Vec::<u8>::new(), 0usize);
    loop {
        let read = stdout.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        carry.extend_from_slice(&buffer[..read]);
        let whole = carry.len() / 4 * 4;
        for bytes in carry[..whole].chunks_exact(4) {
            meters[channel].push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
            channel = (channel + 1) % meters.len();
        }
        carry.drain(..whole);
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("The sound of the units: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(meters
        .into_iter()
        .map(|meter| {
            let kept = meter.finish();
            let energies: Vec<f64> = kept.energies.iter().map(|e| f64::from(*e)).collect();
            let peak = kept.true_peaks.iter().copied().fold(0.0f32, f32::max);
            (qnc_loudness::integrated(&energies), qnc_loudness::peak_db(f64::from(peak)))
        })
        .collect())
}

/// The joined file (MXF OP1a): the pictures copied, every output a mono track, A1/A2 at
/// their gains.
fn write(list: &Path, sound: Sound, gains_db: [f64; 2], ffmpeg: &Path, out: &Path, picture: &[String], start_timecode: &str) -> Result<()> {
    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(list)
        .args(["-filter_complex", &mono_tracks(sound.channels, gains_db), "-map", "0:v:0"]);
    for channel in 0..sound.channels {
        command.args(["-map", &format!("[a{channel}]")]);
    }
    match picture.is_empty() {
        true => command.args(["-c:v", "copy"]),
        false => command.args(picture),
    };
    let output = command
        .args(["-c:a", "pcm_s24le", "-timecode", start_timecode, "-f", "mxf"])
        .arg(out)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("The units cannot be put together: {e}"))?;
    if !output.status.success() {
        return Err(format!("Putting the units together: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(())
}

/// Every output its own mono track `[aN]`; A1 and A2 at their gains, an output without gain
/// only picked out (sample for sample).
fn mono_tracks(channels: u16, gains_db: [f64; 2]) -> String {
    let split: String = (0..channels).map(|channel| format!("[s{channel}]")).collect();
    let mut text = format!("[0:a:0]asplit={channels}{split}");
    for channel in 0..usize::from(channels) {
        let gain = match gains_db.get(channel) {
            Some(db) if *db != 0.0 => format!("{:.6}*", qnc_gain::db_to_gain(*db)),
            _ => String::new(),
        };
        text.push_str(&format!(";[s{channel}]pan=mono|c0={gain}c{channel}[a{channel}]"));
    }
    text
}

fn nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_names_every_unit_in_order_and_every_output_is_a_mono_track() {
        let text = list_text(&[PathBuf::from(r"C:\a\b.mov"), PathBuf::from("/x/it's.mov")]);
        assert_eq!(text, "file 'C:/a/b.mov'\nfile '/x/it'\\''s.mov'\n");
        assert_eq!(
            mono_tracks(3, [6.0, 0.0]),
            "[0:a:0]asplit=3[s0][s1][s2];[s0]pan=mono|c0=1.995262*c0[a0];[s1]pan=mono|c0=c1[a1];[s2]pan=mono|c0=c2[a2]"
        );
    }
}
