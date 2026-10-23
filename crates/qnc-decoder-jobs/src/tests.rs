use super::*;

#[test]
fn a_sound_part_carries_the_samples_exactly() {
    let sent = vec![0.0_f32, -1.0, 0.25, f32::MIN_POSITIVE, 1.0];
    let mut parts = Vec::new();
    {
        let mut send = |part: Value| {
            parts.push(part);
            Ok(())
        };
        let mut sound = SoundParts { send: &mut send, gathered: Vec::new() };
        sound.add(&sent).unwrap();
        sound.flush().unwrap();
        sound.flush().unwrap(); // nothing gathered sends nothing
    }
    assert_eq!(parts.len(), 1);
    let mut got = Vec::new();
    samples(parts.remove(0), &mut |s| {
        got.extend_from_slice(s);
        Ok(())
    })
    .unwrap();
    assert_eq!(got, sent);
}

#[test]
fn a_part_that_is_not_whole_samples_or_not_base64_is_refused() {
    let mut ignore = |_: &[f32]| Ok(());
    assert!(samples(serde_json::json!({ "samples": STANDARD.encode([1u8, 2, 3]) }), &mut ignore).is_err());
    assert!(samples(serde_json::json!({ "samples": "%%%" }), &mut ignore).is_err());
    assert!(samples(serde_json::json!({ "other": "" }), &mut ignore).is_err());
}

#[test]
fn a_job_outside_the_contract_is_refused() {
    let job = serde_json::json!({ "decoder": "ffmpeg", "media": "a.mxf", "start_sec": 1.0, "duration_sec": 2.0, "extra": 1 });
    assert!(parse::<KeyPictures>(job).is_err());
    let job = serde_json::json!({ "decoder": "ffmpeg", "media": "a.mxf", "start_sec": 1.0, "duration_sec": 2.0 });
    assert_eq!(parse::<KeyPictures>(job).unwrap().duration_sec, 2.0);
}
