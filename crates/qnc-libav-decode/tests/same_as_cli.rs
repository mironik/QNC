//! The in-process decoder gives the same pictures as the FFmpeg CLI adapter: the same
//! count, timestamps, time base and bytes, for the requests the player makes (whole
//! stream, a start inside it, the conversion to 4:2:0, deinterlace, twice and half the
//! rate). Needs `ffmpeg` on PATH (makes the test media) and the FFmpeg DLLs of the build.
//! `QNC_LIBAV_REAL=<a camera original (H.264 4:2:2 10-bit MXF), read only>` adds it.

use qnc_ffmpeg_decode::FfmpegAdapter;
use qnc_libav_decode::LibavAdapter;
use qnc_media_decode::*;
use qnc_media_metadata::*;
use std::path::Path;

fn fact<T>(value: T) -> Fact<T> {
    Fact { value, evidence_id: "saved".into(), locator: "saved".into() }
}

struct Media {
    container: &'static str,
    codec: &'static str,
    pixel_format: &'static str,
    fps: i64,
    frames: u64,
    scan: ScanMode,
    time_base: (i64, i64),
}

fn request(media: &Media, name: &str) -> DecodeRequest {
    DecodeRequest {
        version: VERSION.into(),
        stream_index: 0,
        start: None,
        output_pixel_format: None,
        progressive: false,
        delivered_rate: None,
        media: MediaRepresentation {
            media_uri: format!("qnc://local/source/test/file/{name}"),
            container: Some(fact(media.container.into())),
            duration_seconds: Some(fact(Rational { numerator: media.frames as i64, denominator: media.fps })),
            streams_complete: Some(fact(true)),
            tags: Default::default(),
            streams: vec![MediaStream {
                index: Some(fact(0)),
                codec: Some(fact(Signal::Known(media.codec.into()))),
                profile: None,
                time_base: Some(fact(Rational { numerator: media.time_base.0, denominator: media.time_base.1 })),
                start_pts: Some(fact(0)),
                duration_ts: None,
                details: StreamDetails::Video(Box::new(VideoMetadata {
                    width: Some(fact(1920)),
                    height: Some(fact(1080)),
                    frame_rate: Some(fact(FrameTimebase { fps_num: media.fps, fps_den: 1 })),
                    frame_rate_mode: Some(fact(FrameRateMode::Constant)),
                    frame_count: Some(fact(FrameCount::Exact(media.frames))),
                    scan_mode: Some(fact(media.scan)),
                    pixel_format: Some(fact(media.pixel_format.into())),
                    sample_aspect_ratio: Some(fact(Rational { numerator: 1, denominator: 1 })),
                    rotation_degrees: Some(fact(Signal::Known(0))),
                    color: ColorMetadata { primaries: None, transfer: None, matrix: None, range: None },
                })),
            }],
        },
    }
}

fn decode(adapter: impl DecoderAdapter + 'static, request: &DecodeRequest, file: &Path, most: usize) -> Vec<(i64, Rational, u64, usize)> {
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(file, &request.media.media_uri).unwrap();
    let mut decoder = Decoder::open_endpoint(request.clone(), endpoint, "stamp".into(), DecoderConfig::new(adapter)).unwrap();
    let mut packets = Vec::new();
    while packets.len() < most {
        let Some(packet) = decoder.next_packet().unwrap() else { break };
        packets.push((packet.pts, packet.time_base, hash(&packet.bytes), packet.bytes.len()));
    }
    packets
}

fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn same(request: &DecodeRequest, file: &Path, most: usize, what: &str) {
    println!("{what}: CLI"); let cli = decode(FfmpegAdapter::new("ffmpeg"), request, file, most);
    println!("{what}: in-process");
    let started = std::time::Instant::now();
    let inside = decode(LibavAdapter::new("ffmpeg"), request, file, most);
    println!("{what}: {} pictures, in-process {} ms", inside.len(), started.elapsed().as_millis());
    assert!(!cli.is_empty(), "{what}: the CLI gave pictures");
    assert_eq!(cli.len(), inside.len(), "{what}: picture count");
    for (index, (a, b)) in cli.iter().zip(&inside).enumerate() {
        assert_eq!(a, b, "{what}: picture {index} (pts, time base, bytes, size)");
    }
}

fn make(args: &[&str], file: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-f", "lavfi", "-i"])
        .args(args)
        .arg(file)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn h264_422_10bit_progressive_like_xavc() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("xavc.mov");
    make(
        &["testsrc2=size=1920x1080:rate=50:duration=3", "-c:v", "libx264", "-profile:v", "high422", "-pix_fmt", "yuv422p10le", "-g", "24", "-bf", "2", "-f", "mov"],
        &file,
    );
    let media = Media { container: "mov", codec: "h264", pixel_format: "yuv422p10le", fps: 50, frames: 150, scan: ScanMode::Progressive, time_base: (1, 50) };
    let mut whole = request(&media, "xavc%2Emov");
    same(&whole, &file, usize::MAX, "whole stream, saved layout");
    whole.output_pixel_format = Some("yuv420p".into());
    same(&whole, &file, usize::MAX, "whole stream, 4:2:0");
    for start in [(37, 50), (73, 50), (1, 1)] {
        let mut from = whole.clone();
        from.start = Some(Rational { numerator: start.0, denominator: start.1 });
        same(&from, &file, 40, &format!("from {}/{} s", start.0, start.1));
    }
    let mut half = whole.clone();
    half.delivered_rate = Some(FrameTimebase { fps_num: 25, fps_den: 1 });
    same(&half, &file, usize::MAX, "half the rate");
    let mut twice = whole.clone();
    twice.delivered_rate = Some(FrameTimebase { fps_num: 100, fps_den: 1 });
    same(&twice, &file, usize::MAX, "twice the rate");
}

#[test]
fn interlaced_mpeg2_deinterlaced_and_each_field() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("xdcam.mxf");
    make(
        &["testsrc2=size=1920x1080:rate=50:duration=1", "-vf", "tinterlace=mode=interleave_top,setfield=tff", "-c:v", "mpeg2video", "-pix_fmt", "yuv422p", "-b:v", "50M", "-flags", "+ildct+ilme", "-top", "1", "-f", "mxf"],
        &file,
    );
    let media = Media { container: "mxf", codec: "mpeg2video", pixel_format: "yuv422p", fps: 25, frames: 25, scan: ScanMode::InterlacedTopFieldFirst, time_base: (1, 25) };
    let mut progressive = request(&media, "xdcam%2Emxf");
    progressive.output_pixel_format = Some("yuv420p".into());
    progressive.progressive = true;
    same(&progressive, &file, usize::MAX, "deinterlaced");
    let mut fields = progressive.clone();
    fields.delivered_rate = Some(FrameTimebase { fps_num: 50, fps_den: 1 });
    same(&fields, &file, usize::MAX, "each field");
}

#[test]
fn camera_original_if_given() {
    let Some(path) = std::env::var_os("QNC_LIBAV_REAL") else {
        return;
    };
    let file = Path::new(&path);
    let media = Media { container: "mxf", codec: "h264", pixel_format: "yuv422p10le", fps: 50, frames: 11178, scan: ScanMode::Progressive, time_base: (1, 50) };
    let mut from = request(&media, "real%2Emxf");
    from.output_pixel_format = Some("yuv420p".into());
    for start in [(30, 1), (1513, 50), (4021, 50), (777, 50)] {
        from.start = Some(Rational { numerator: start.0, denominator: start.1 });
        same(&from, file, 100, &format!("camera original from {}/{} s", start.0, start.1));
    }
}

#[test]
fn first_picture_time_on_camera_original() {
    let Some(path) = std::env::var_os("QNC_LIBAV_REAL") else {
        return;
    };
    let file = Path::new(&path);
    let media = Media { container: "mxf", codec: "h264", pixel_format: "yuv422p10le", fps: 50, frames: 11178, scan: ScanMode::Progressive, time_base: (1, 50) };
    let mut from = request(&media, "real%2Emxf");
    from.output_pixel_format = Some("yuv420p".into());
    for (name, frames) in [("in-process", true), ("cli", false)] {
        let mut times = Vec::new();
        for frame in [1505i64, 1475, 4021, 777, 6123, 1888] {
            from.start = Some(Rational { numerator: frame, denominator: 50 });
            let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(file, &from.media.media_uri).unwrap();
            let started = std::time::Instant::now();
            let config = if frames { DecoderConfig::new(LibavAdapter::new("ffmpeg")) } else { DecoderConfig::new(FfmpegAdapter::new("ffmpeg")) };
            let mut decoder = Decoder::open_endpoint(from.clone(), endpoint, "stamp".into(), config).unwrap();
            let opened = started.elapsed().as_millis();
            decoder.next_packet().unwrap().unwrap();
            times.push((opened, started.elapsed().as_millis()));
        }
        println!("{name}: (open ms, first picture ms) {times:?}");
    }
}

/// One open in-process decoder jumps through `starts` (reading `most` pictures after each,
/// the first time to the end); after every jump it gives the pictures the CLI gives when
/// opened at that start.
fn same_after_jumps(request: &DecodeRequest, file: &Path, starts: &[(i64, i64)], most: usize, what: &str) {
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(file, &request.media.media_uri).unwrap();
    let mut open = Decoder::open_endpoint(request.clone(), endpoint, "stamp".into(), DecoderConfig::new(LibavAdapter::new("ffmpeg"))).unwrap();
    while open.next_packet().unwrap().is_some() {} // to the end: a jump after the end works too
    // `next_packet` shut the decoder down at its end; a decoder read with `try_next_packet`
    // stays. Open again and read a few pictures first instead.
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(file, &request.media.media_uri).unwrap();
    open = Decoder::open_endpoint(request.clone(), endpoint, "stamp".into(), DecoderConfig::new(LibavAdapter::new("ffmpeg"))).unwrap();
    for _ in 0..5 {
        open.next_packet().unwrap();
    }
    for &(num, den) in starts {
        let start = Rational { numerator: num, denominator: den };
        let started = std::time::Instant::now();
        assert!(open.seek(start).unwrap(), "{what}: the in-process decoder jumps");
        let mut jumped = Vec::new();
        while jumped.len() < most {
            let Some(packet) = open.next_packet().unwrap() else { break };
            if jumped.is_empty() {
                println!("{what}: jump to {num}/{den} first picture {} ms", started.elapsed().as_millis());
            }
            jumped.push((packet.pts, packet.time_base, hash(&packet.bytes), packet.bytes.len()));
        }
        let mut fresh = request.clone();
        fresh.start = Some(start);
        let cli = decode(FfmpegAdapter::new("ffmpeg"), &fresh, file, most);
        if cli != jumped {
            let pts = |v: &Vec<(i64, Rational, u64, usize)>| v.iter().take(6).map(|p| p.0).collect::<Vec<_>>();
            println!("{what} {num}/{den}: cli {:?} in-process {:?}", pts(&cli), pts(&jumped));
        }
        assert_eq!(cli.len(), jumped.len(), "{what}: count after jump to {num}/{den}");
        for (index, (a, b)) in cli.iter().zip(&jumped).enumerate() {
            assert_eq!(a, b, "{what}: picture {index} after jump to {num}/{den}");
        }
    }
}

#[test]
fn jumps_give_the_pictures_of_a_fresh_open() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("xavc.mov");
    make(
        &["testsrc2=size=1920x1080:rate=50:duration=3", "-c:v", "libx264", "-profile:v", "high422", "-pix_fmt", "yuv422p10le", "-g", "24", "-bf", "2", "-f", "mov"],
        &file,
    );
    let media = Media { container: "mov", codec: "h264", pixel_format: "yuv422p10le", fps: 50, frames: 150, scan: ScanMode::Progressive, time_base: (1, 50) };
    let mut request = request(&media, "xavc%2Emov");
    request.output_pixel_format = Some("yuv420p".into());
    same_after_jumps(&request, &file, &[(73, 50), (12, 50), (140, 50), (0, 1), (99, 50)], 20, "progressive");
    let dir2 = tempfile::tempdir().unwrap();
    let file2 = dir2.path().join("xdcam.mxf");
    make(
        &["testsrc2=size=1920x1080:rate=50:duration=2", "-vf", "tinterlace=mode=interleave_top,setfield=tff", "-c:v", "mpeg2video", "-pix_fmt", "yuv422p", "-b:v", "50M", "-flags", "+ildct+ilme", "-top", "1", "-f", "mxf"],
        &file2,
    );
    let media = Media { container: "mxf", codec: "mpeg2video", pixel_format: "yuv422p", fps: 25, frames: 50, scan: ScanMode::InterlacedTopFieldFirst, time_base: (1, 25) };
    let mut fields = self::request(&media, "xdcam%2Emxf");
    fields.output_pixel_format = Some("yuv420p".into());
    fields.progressive = true;
    fields.delivered_rate = Some(FrameTimebase { fps_num: 50, fps_den: 1 });
    same_after_jumps(&fields, &file2, &[(31, 25), (3, 25), (45, 25)], 10, "each field");
    if let Some(path) = std::env::var_os("QNC_LIBAV_REAL") {
        let media = Media { container: "mxf", codec: "h264", pixel_format: "yuv422p10le", fps: 50, frames: 11178, scan: ScanMode::Progressive, time_base: (1, 50) };
        let mut real = self::request(&media, "real%2Emxf");
        real.output_pixel_format = Some("yuv420p".into());
        real.start = Some(Rational { numerator: 1500, denominator: 50 });
        same_after_jumps(&real, Path::new(&path), &[(1505, 50), (1475, 50), (4021, 50), (777, 50), (1888, 50), (1880, 50)], 30, "camera original");
    }
}

/// Many jumps back and forth on the camera original (`QNC_LIBAV_REAL`), the first pictures
/// after each compared with a fresh CLI open; prints every difference with its timestamps.
#[test]
fn many_jumps_on_camera_original() {
    let Some(path) = std::env::var_os("QNC_LIBAV_REAL") else {
        return;
    };
    let file = Path::new(&path);
    let media = Media { container: "mxf", codec: "h264", pixel_format: "yuv422p10le", fps: 50, frames: 5097, scan: ScanMode::Progressive, time_base: (1, 50) };
    let mut request = request(&media, "real%2Emxf");
    request.output_pixel_format = Some("yuv420p".into());
    request.start = Some(Rational { numerator: 1500, denominator: 50 });
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(file, &request.media.media_uri).unwrap();
    let mut open = Decoder::open_endpoint(request.clone(), endpoint, "stamp".into(), DecoderConfig::new(LibavAdapter::new("ffmpeg"))).unwrap();
    let mut frame: i64 = 1500;
    let steps = [-8, -1, -1, 3, 24, -25, -12, 50, -100, 7, -2, -30, 13, -13, 1, -1, 200, -190, 5, -48, 2, -3, 31, -31, 9, -9, 400, -401, 4, -4, 17, -6, -11, 60, -59, 8, -8, 1, -24, 22];
    let mut differences = 0;
    for step in steps {
        frame = (frame + step).clamp(0, 5000);
        let start = Rational { numerator: frame, denominator: 50 };
        assert!(open.seek(start).unwrap());
        let mut jumped = Vec::new();
        while jumped.len() < 3 {
            let Some(packet) = open.next_packet().unwrap() else { break };
            jumped.push((packet.pts, packet.time_base, hash(&packet.bytes), packet.bytes.len()));
        }
        let mut fresh = request.clone();
        fresh.start = Some(start);
        let cli = decode(FfmpegAdapter::new("ffmpeg"), &fresh, file, 3);
        if cli != jumped {
            differences += 1;
            let pts = |v: &Vec<(i64, Rational, u64, usize)>| v.iter().map(|p| (p.0, p.2 % 1000)).collect::<Vec<_>>();
            println!("DIFF jump {step:+} to {frame}: cli {:?} in-process {:?}", pts(&cli), pts(&jumped));
        }
    }
    println!("{differences} of {} jumps differ", steps.len());
    assert_eq!(differences, 0);
}
