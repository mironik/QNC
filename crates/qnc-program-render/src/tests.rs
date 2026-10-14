use super::*;
use qnc_media_metadata::FrameTimebase;

fn export(codec: &str, field_order: &str, fps: f64) -> Value {
    serde_json::json!({
        "video_codec": codec, "width": 1920, "height": 1080, "fps": fps, "field_order": field_order,
        "audio_sample_rate": 48000, "audio_channels": 2, "container": "mp4",
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
