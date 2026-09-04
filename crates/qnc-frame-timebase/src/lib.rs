pub const DEFAULT_FPS: f64 = 0.0;
pub const DEFAULT_DURATION_MARKS_SEC: [f64; 3] = [3.0, 5.0, 7.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DualFpsSnapshot {
    pub source_fps: f64,
    pub timeline_fps: f64,
    pub source_duration_frames: i64,
    pub timeline_duration_frames: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTimebase {
    pub fps_num: i64,
    pub fps_den: i64,
}

impl FrameTimebase {
    pub fn new(fps_num: i64, fps_den: i64) -> Result<Self, String> {
        if fps_num <= 0 || fps_den <= 0 {
            return Err("frame timebase must come from probe metadata and cannot be zero".into());
        }
        Ok(Self { fps_num, fps_den })
    }

    pub fn fps(self) -> f64 {
        self.fps_num as f64 / self.fps_den as f64
    }
}

pub fn is_valid_fps(raw: f64) -> bool {
    raw.is_finite() && raw > 0.0
}

pub fn require_fps(raw: f64, context: &str) -> Result<f64, String> {
    if is_valid_fps(raw) {
        Ok(raw)
    } else {
        Err(format!("{context}: missing valid FPS"))
    }
}

pub fn normalize_fps(raw: f64) -> f64 {
    if is_valid_fps(raw) {
        raw
    } else {
        DEFAULT_FPS
    }
}

pub fn rational_fps(fps: f64) -> FrameTimebase {
    let fps = normalize_fps(fps);
    for (num, den) in [(24000, 1001), (30000, 1001), (48000, 1001), (60000, 1001)] {
        if (fps - (num as f64 / den as f64)).abs() < 0.01 {
            return FrameTimebase {
                fps_num: num,
                fps_den: den,
            };
        }
    }

    let rounded = fps.round();
    if (fps - rounded).abs() < 0.001 && rounded >= 1.0 {
        return FrameTimebase {
            fps_num: rounded as i64,
            fps_den: 1,
        };
    }

    FrameTimebase {
        fps_num: (fps * 1000.0).round() as i64,
        fps_den: 1000,
    }
}

pub fn seconds_to_frame(seconds: f64, fps: f64) -> i64 {
    let fps = normalize_fps(fps);
    if fps <= 0.0 {
        return 0;
    }
    (seconds.max(0.0) * fps).round() as i64
}

pub fn frame_to_seconds(frame: i64, fps: f64) -> f64 {
    let fps = normalize_fps(fps);
    if fps <= 0.0 {
        return 0.0;
    }
    (frame.max(0) as f64) / fps
}

pub fn snap_seconds_to_frame(seconds: f64, fps: f64) -> f64 {
    frame_to_seconds(seconds_to_frame(seconds, fps), fps)
}

pub fn duration_frames(in_seconds: f64, out_seconds: f64, fps: f64) -> i64 {
    let in_frame = seconds_to_frame(in_seconds, fps);
    let out_frame = seconds_to_frame(out_seconds, fps);
    (out_frame - in_frame).max(0)
}

pub fn seconds_frames_label_from_frames(frames: i64, fps: f64) -> String {
    let fps = normalize_fps(fps);
    if fps <= 0.0 {
        return "0:00".into();
    }
    let fps = fps.round().max(1.0) as i64;
    let frames = frames.max(0);
    let seconds = frames / fps;
    let rem = frames % fps;
    format!("{seconds}:{rem:02}")
}

pub fn duration_color_key_from_frames(frames: i64, fps: f64) -> &'static str {
    let fps = normalize_fps(fps);
    if fps <= 0.0 {
        return "over_7";
    }
    let seconds = (frames.max(0) as f64) / fps;
    if seconds < DEFAULT_DURATION_MARKS_SEC[0] {
        "under_3"
    } else if seconds < DEFAULT_DURATION_MARKS_SEC[1] {
        "under_5"
    } else if seconds < DEFAULT_DURATION_MARKS_SEC[2] {
        "under_7"
    } else {
        "over_7"
    }
}

pub fn seconds_to_timecode(seconds: f64, fps: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "00:00:00:00".into();
    }
    let fps = normalize_fps(fps);
    if fps <= 0.0 {
        return "00:00:00:00".into();
    }
    frame_to_timecode(seconds_to_frame(seconds, fps), fps)
}

pub fn frame_to_timecode(frame: i64, fps: f64) -> String {
    let fps = normalize_fps(fps);
    if fps <= 0.0 {
        return "00:00:00:00".into();
    }
    let fps_int = fps.round().max(1.0) as i64;
    let total = frame.max(0);
    let frames = total % fps_int;
    let total_sec = total / fps_int;
    let ss = total_sec % 60;
    let mm = (total_sec / 60) % 60;
    let hh = total_sec / 3600;
    format!("{hh:02}:{mm:02}:{ss:02}:{frames:02}")
}

pub fn dual_fps_snapshot(
    in_frame: i64,
    out_frame: i64,
    source_fps: f64,
    timeline_fps: f64,
) -> DualFpsSnapshot {
    let source_fps = normalize_fps(source_fps);
    let timeline_fps = normalize_fps(timeline_fps);
    let source_duration_frames = (out_frame - in_frame).max(0);
    let timeline_duration_frames = if source_fps > 0.0 && timeline_fps > 0.0 {
        let duration_sec = source_duration_frames as f64 / source_fps;
        seconds_to_frame(duration_sec, timeline_fps)
    } else {
        0
    };
    DualFpsSnapshot {
        source_fps,
        timeline_fps,
        source_duration_frames,
        timeline_duration_frames,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_seconds_and_frames_like_qnc_v4() {
        assert_eq!(
            seconds_frames_label_from_frames(seconds_to_frame(3.84, 25.0), 25.0),
            "3:21"
        );
        assert_eq!(seconds_to_timecode(3.84, 25.0), "00:00:03:21");
        assert_eq!(frame_to_timecode(96, 25.0), "00:00:03:21");
    }

    #[test]
    fn snaps_to_frame_boundaries_like_qnc_v4() {
        assert_eq!(seconds_to_frame(1.021, 25.0), 26);
        assert_eq!(snap_seconds_to_frame(1.021, 25.0), 1.04);
    }

    #[test]
    fn assigns_default_duration_color_keys_like_qnc_v4() {
        assert_eq!(duration_color_key_from_frames(50, 25.0), "under_3");
        assert_eq!(duration_color_key_from_frames(100, 25.0), "under_5");
        assert_eq!(duration_color_key_from_frames(150, 25.0), "under_7");
        assert_eq!(duration_color_key_from_frames(200, 25.0), "over_7");
    }

    #[test]
    fn rational_fps_is_exact_for_common_broadcast_rates() {
        assert_eq!(
            rational_fps(50.0),
            FrameTimebase {
                fps_num: 50,
                fps_den: 1
            }
        );
        assert_eq!(
            rational_fps(25.0),
            FrameTimebase {
                fps_num: 25,
                fps_den: 1
            }
        );
        assert_eq!(
            rational_fps(29.97),
            FrameTimebase {
                fps_num: 30000,
                fps_den: 1001
            }
        );
        assert_eq!(
            rational_fps(23.976),
            FrameTimebase {
                fps_num: 24000,
                fps_den: 1001
            }
        );
        assert_eq!(
            rational_fps(59.94),
            FrameTimebase {
                fps_num: 60000,
                fps_den: 1001
            }
        );
        assert_eq!(
            rational_fps(0.0),
            FrameTimebase {
                fps_num: 0,
                fps_den: 1000
            }
        );
    }

    #[test]
    fn dual_fps_keeps_real_time_on_timeline() {
        let snap = dual_fps_snapshot(0, 100, 50.0, 25.0);
        assert_eq!(snap.source_duration_frames, 100);
        assert_eq!(snap.timeline_duration_frames, 50);
    }

    #[test]
    fn rejects_missing_probe_fps_when_required() {
        assert!(require_fps(0.0, "test").is_err());
        assert!(FrameTimebase::new(0, 1).is_err());
        assert!(FrameTimebase::new(25, 1).is_ok());
    }
}
