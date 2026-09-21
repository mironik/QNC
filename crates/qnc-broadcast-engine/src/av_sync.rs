#![cfg(test)]
//! Picture vs sound on the saved source timebase. Not a second clock.

use super::input::{frame_from_samples, picture_audio_offset_frames, sample_boundary};
use super::*;
use std::path::PathBuf;

/// One source frame of present jitter is allowed. A later picture is not lag.
fn picture_lags_sound(offset_frames: i64) -> bool {
    offset_frames < -1
}

#[test]
fn audio_clock_frame_uses_saved_clip_timebase() {
    let p50 = Timebase::new(50, 1).unwrap();
    assert_eq!(frame_from_samples(0, p50, 48_000).unwrap(), 0);
    assert_eq!(frame_from_samples(960, p50, 48_000).unwrap(), 1);
    assert_eq!(frame_from_samples(48_000, p50, 48_000).unwrap(), 50);
}

#[test]
fn sample_boundary_and_frame_from_samples_invert_on_source_timebase() {
    for (num, den, rate) in [(50, 1, 48_000u32)] {
        let timebase = Timebase::new(num, den).unwrap();
        for frame in [0, 1, 7, 50, 100] {
            let sample = sample_boundary(frame, timebase, rate).unwrap();
            assert_eq!(
                frame_from_samples(sample, timebase, rate).unwrap(),
                frame,
                "timebase {num}/{den} frame {frame}"
            );
        }
    }
}

#[test]
fn picture_must_not_lag_the_audio_clock() {
    assert!(!picture_lags_sound(picture_audio_offset_frames(100, 100)));
    assert!(!picture_lags_sound(picture_audio_offset_frames(100, 101)));
    assert!(!picture_lags_sound(picture_audio_offset_frames(102, 100)));
    assert!(picture_lags_sound(picture_audio_offset_frames(98, 100)));
    assert!(picture_lags_sound(picture_audio_offset_frames(90, 100)));
}

#[derive(Debug, Clone)]
struct AvPoint {
    picture: u64,
    audio_frame: u64,
    offset: i64,
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let token = line.split_whitespace().find(|part| part.starts_with(key))?;
    token.strip_prefix(key)
}

fn parse_timebase(line: &str) -> Option<Timebase> {
    let num = line.split("fps_num: ").nth(1)?.split([',', ' ']).next()?;
    let den = line
        .split("fps_den: ")
        .nth(1)?
        .split([',', ' ', '}'])
        .next()?;
    Timebase::new(num.parse().ok()?, den.parse().ok()?).ok()
}

fn line_stamp_ns(line: &str) -> Option<u128> {
    if let Some(unix) = field(line, "unix_ns=").and_then(|v| v.parse().ok()) {
        return Some(unix);
    }
    let stamp: u128 = line.split_whitespace().next()?.parse().ok()?;
    if stamp > 1_000_000_000_000_000 {
        Some(stamp)
    } else {
        Some(stamp.saturating_mul(1_000_000))
    }
}

fn interpolate_audio_samples(sample: u64, rate: u32, audio_ns: u128, picture_ns: u128) -> u64 {
    let dt = picture_ns.abs_diff(audio_ns);
    let delta = (dt.saturating_mul(u128::from(rate)) / 1_000_000_000) as u64;
    if picture_ns >= audio_ns {
        sample.saturating_add(delta)
    } else {
        sample.saturating_sub(delta)
    }
}

fn parse_player_log_av_points(text: &str) -> Vec<AvPoint> {
    let mut timebase = None;
    let mut last_audio: Option<(u128, u64, u32)> = None;
    let mut points = Vec::new();
    for line in text.lines() {
        if let Some(tb) = parse_timebase(line) {
            timebase = Some(tb);
        }
        let stamp_ns = line_stamp_ns(line).unwrap_or(0);
        if line.contains(" AV_A ") {
            if let (Some(sample), Some(rate)) = (
                field(line, "sample=").and_then(|v| v.parse().ok()),
                field(line, "rate=").and_then(|v| v.parse().ok()),
            ) {
                last_audio = Some((stamp_ns, sample, rate));
            }
            continue;
        }
        let Some(tb) = timebase else {
            continue;
        };
        if line.contains("player-output ") && line.contains("submitted=Some(") {
            let picture = line
                .split("submitted=Some(")
                .nth(1)
                .and_then(|rest| rest.split(')').next())
                .and_then(|v| v.parse().ok());
            let samples = line
                .split("submitted_frames: ")
                .nth(1)
                .and_then(|rest| rest.split(',').next())
                .and_then(|v| v.parse().ok());
            if let (Some(picture), Some(samples), Some((_, _, rate))) =
                (picture, samples, last_audio)
            {
                if let Ok(audio_frame) = frame_from_samples(samples, tb, rate) {
                    points.push(AvPoint {
                        picture,
                        audio_frame,
                        offset: picture_audio_offset_frames(picture, audio_frame),
                    });
                    continue;
                }
            }
            if let Some(picture) = picture {
                if let Some((audio_at, sample, rate)) = last_audio {
                    if stamp_ns.abs_diff(audio_at) <= 80_000_000 {
                        let sample = interpolate_audio_samples(sample, rate, audio_at, stamp_ns);
                        if let Ok(audio_frame) = frame_from_samples(sample, tb, rate) {
                            points.push(AvPoint {
                                picture,
                                audio_frame,
                                offset: picture_audio_offset_frames(picture, audio_frame),
                            });
                        }
                    }
                }
            }
            continue;
        }
        let picture = if line.contains(" AV_F ") && line.contains(" frame=") {
            field(line, "frame=").and_then(|v| v.parse().ok())
        } else if line.contains(" AV_V ") && line.contains(" sequence=") {
            field(line, "sequence=").and_then(|v| v.parse().ok())
        } else {
            None
        };
        let Some(picture) = picture else {
            continue;
        };
        let Some((audio_at, sample, rate)) = last_audio else {
            continue;
        };
        if stamp_ns.abs_diff(audio_at) > 80_000_000 {
            continue;
        }
        let sample = interpolate_audio_samples(sample, rate, audio_at, stamp_ns);
        let Ok(audio_frame) = frame_from_samples(sample, tb, rate) else {
            continue;
        };
        points.push(AvPoint {
            picture,
            audio_frame,
            offset: picture_audio_offset_frames(picture, audio_frame),
        });
    }
    points
}

#[test]
fn recorded_fixture_picture_stays_with_sound() {
    let log = "\
1 CarrierPositionChanged { timebase: Some(FrameTimebase { fps_num: 50, fps_den: 1 }) }
10 AV_A session=s sample=48000 rate=48000 unix_ns=1
12 AV_F session=s generation=1 sequence=50 frame=50 frame_map_us=1
20 AV_A session=s sample=48960 rate=48000 unix_ns=2
21 player-output status=Playing ready=false carrier=51 submitted=Some(51) presented=None
";
    let points = parse_player_log_av_points(log);
    assert_eq!(points.len(), 2);
    assert!(
        points.iter().all(|p| !picture_lags_sound(p.offset)),
        "{points:?}"
    );
}

#[test]
fn recorded_fixture_detects_picture_behind_sound() {
    let log = "\
1 CarrierPositionChanged { timebase: Some(FrameTimebase { fps_num: 50, fps_den: 1 }) }
10 AV_A session=s sample=96000 rate=48000 unix_ns=1
12 AV_F session=s generation=1 sequence=90 frame=90 frame_map_us=1
";
    let points = parse_player_log_av_points(log);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].audio_frame, 100);
    assert_eq!(points[0].picture, 90);
    assert!(picture_lags_sound(points[0].offset));
}

#[test]
#[ignore = "reads the local, untracked data/diagnostics/player.log, which accumulates old sessions; run with --ignored"]
fn recorded_player_log_picture_must_not_lag_sound() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/diagnostics/player.log");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let tail = if text.len() > 512 * 1024 {
        &text[text.len() - 512 * 1024..]
    } else {
        &text
    };
    let mut timebase = None;
    let mut last_audio: Option<(u128, u64, u32)> = None;
    let mut points = Vec::new();
    for line in tail.lines() {
        if let Some(tb) = parse_timebase(line) {
            timebase = Some(tb);
        }
        let stamp_ns = line_stamp_ns(line).unwrap_or(0);
        if line.contains(" AV_A ") {
            if let (Some(sample), Some(rate)) = (
                field(line, "sample=").and_then(|v| v.parse().ok()),
                field(line, "rate=").and_then(|v| v.parse().ok()),
            ) {
                last_audio = Some((stamp_ns, sample, rate));
            }
            continue;
        }
        if !line.contains("player-output ") || !line.contains("submitted=Some(") {
            continue;
        }
        let Some(tb) = timebase else {
            continue;
        };
        let Some(picture) = line
            .split("submitted=Some(")
            .nth(1)
            .and_then(|rest| rest.split(')').next())
            .and_then(|v| v.parse().ok())
        else {
            continue;
        };
        let Some((audio_at, sample, rate)) = last_audio else {
            continue;
        };
        if stamp_ns.abs_diff(audio_at) > 80_000_000 {
            continue;
        }
        let sample = interpolate_audio_samples(sample, rate, audio_at, stamp_ns);
        let Ok(audio_frame) = frame_from_samples(sample, tb, rate) else {
            continue;
        };
        points.push(AvPoint {
            picture,
            audio_frame,
            offset: picture_audio_offset_frames(picture, audio_frame),
        });
    }
    if points.is_empty() {
        return;
    }
    let worst = points.iter().map(|p| p.offset).min().unwrap();
    let late = points.iter().filter(|p| p.offset < -2).count();
    assert!(
        !points.iter().any(|p| p.offset < -2),
        "picture lagged sound by up to {worst} source frames on {late} of {} samples",
        points.len()
    );
}
