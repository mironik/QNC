use qnc_player_contract::{
    AudioFormat, BroadcastPlaybackRequest, BroadcastPlayerProtocolCommand, ColorSpace, FieldMode,
    FrameRange, SourceRuntime, Timebase, VideoFormat,
};

fn source() -> SourceRuntime {
    SourceRuntime::new("clip-from-db", 100, Timebase::new(30000, 1001).unwrap())
        .unwrap()
        .with_video_format(
            VideoFormat::new(1920, 1080, FieldMode::Progressive, ColorSpace::Rec709).unwrap(),
        )
        .with_audio_format(AudioFormat::new(48000, 2).unwrap())
}

#[test]
fn shared_timebase_is_used_without_rounding_or_fallback() {
    let canonical = qnc_frame_timebase::FrameTimebase::new(30000, 1001).unwrap();
    let player: Timebase = canonical;
    assert_eq!(source().timebase, player);
}

#[test]
fn wire_deserialization_cannot_bypass_source_validation() {
    for (pointer, value) in [
        ("/timebase/fps_num", 0),
        ("/timebase/fps_den", -1),
        ("/duration_frames", 0),
        ("/video_format/width", 0),
        ("/video_format/pixel_aspect/den", 0),
        ("/audio_format/channel_count", 0),
        ("/audio_format/sample_rate_hz", 0),
    ] {
        let mut json = serde_json::to_value(source()).unwrap();
        *json.pointer_mut(pointer).unwrap() = value.into();
        let bad: SourceRuntime = serde_json::from_value(json).unwrap();
        assert!(bad.validate().is_err(), "{pointer}");
        assert!(
            BroadcastPlayerProtocolCommand::LoadSource { source: bad }
                .validate()
                .is_err()
        );
    }
}

#[test]
fn invalid_ranges_and_exclusive_end_cannot_become_playback_frames() {
    let request = BroadcastPlaybackRequest::new("r", source()).unwrap();
    for (start, end, frame) in [(10, 10, 10), (20, 10, 15), (0, 100, 100)] {
        let mut bad = request.clone();
        bad.execution_range = FrameRange {
            start_frame: start,
            end_frame: end,
        };
        bad.initial_frame = frame;
        assert!(bad.validate().is_err());
    }
    assert!(
        !FrameRange::new(0, u64::MAX)
            .unwrap()
            .contains_item(u64::MAX - 1, 2)
    );
}

#[test]
fn undeclared_command_fields_are_not_ignored() {
    let mut value =
        serde_json::to_value(BroadcastPlayerProtocolCommand::LoadSource { source: source() })
            .unwrap();
    value["LoadSource"]["probe_if_missing"] = true.into();
    assert!(serde_json::from_value::<BroadcastPlayerProtocolCommand>(value).is_err());
}
