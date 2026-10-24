use super::*;
use qnc_media_metadata::FrameTimebase;

fn export(codec: &str, field_order: &str, fps: f64) -> Value {
    serde_json::json!({
        "video_codec": codec, "width": 1920, "height": 1080, "fps": fps, "field_order": field_order,
        "audio_sample_rate": 48000, "audio_channels": 2, "container": "mp4", "video_bitrate_mbps": 35,
    })
}

fn unit() -> RenderUnit {
    RenderUnit {
        record_in: 0,
        record_out: 50,
        program_rate: FrameTimebase { fps_num: 50, fps_den: 1 },
        sample_rate: 48000,
        channels: 2,
        samples: 48000,
        video: None,
        audio: Vec::new(),
    }
}

#[test]
fn the_project_h264_preset_is_a_delivery() {
    let delivery = Delivery::from_export(&export("h264", "progressive", 50.0)).unwrap();
    assert!(delivery.check(&unit()).is_ok());
    assert!(delivery.file_name(&unit()).ends_with(".mov"));
}

#[test]
fn what_cannot_be_rendered_is_refused_with_a_reason() {
    assert!(Delivery::from_export(&export("prores", "progressive", 50.0)).unwrap_err().contains("prores"));
    assert!(Delivery::from_export(&serde_json::json!({"video_codec": "h264"})).is_err());
    let interlaced = Delivery::from_export(&export("mpeg2_422_50mbit", "upper_first", 25.0)).unwrap();
    assert!(interlaced.check(&unit()).unwrap_err().contains("interlaced"));
    let other_rate = Delivery::from_export(&export("h264", "progressive", 25.0)).unwrap();
    assert!(other_rate.check(&unit()).is_err());
}

#[test]
fn a_unit_rendered_in_another_delivery_is_another_file() {
    let h264 = Delivery::from_export(&export("h264", "progressive", 50.0)).unwrap();
    let mpeg2 = Delivery::from_export(&export("mpeg2_422_50mbit", "progressive", 50.0)).unwrap();
    assert_ne!(h264.file_name(&unit()), mpeg2.file_name(&unit()));
}

/// An H.264 delivery is encoded at the project's constant bitrate (user 2026-10-24: CBR
/// 35 Mbit/s): without it there is no delivery, another bitrate is another unit, and every
/// encoder is given the same rate as its target, floor and ceiling.
#[test]
fn an_h264_delivery_has_a_constant_bitrate() {
    let mut without = export("h264", "progressive", 50.0);
    without.as_object_mut().unwrap().remove("video_bitrate_mbps");
    assert!(Delivery::from_export(&without).unwrap_err().contains("video_bitrate_mbps"));
    let mut fifty = export("h264", "progressive", 50.0);
    fifty["video_bitrate_mbps"] = serde_json::json!(50);
    let (at_35, at_50) = (Delivery::from_export(&export("h264", "progressive", 50.0)).unwrap(), Delivery::from_export(&fifty).unwrap());
    assert_ne!(at_35.file_name(&unit()), at_50.file_name(&unit()));
    for encoder in [encoder::H264::IntelQsv, encoder::H264::NvidiaNvenc, encoder::H264::AmdAmf, encoder::H264::Software] {
        let (_, args) = encoder.args(35);
        let after = |flag: &str| args.iter().position(|arg| arg == flag).map(|at| args[at + 1].clone());
        assert_eq!((after("-b:v"), after("-maxrate")), (Some("35M".to_string()), Some("35M".to_string())), "{encoder:?}");
    }
}
