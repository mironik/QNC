use super::*;

fn tone(rate: u32, hz: f64, seconds: f64) -> Vec<f32> {
    let count = (f64::from(rate) * seconds) as usize;
    (0..count)
        .map(|n| (0.5 * (2.0 * std::f64::consts::PI * hz * n as f64 / f64::from(rate)).sin()) as f32)
        .collect()
}

fn level(samples: &[f32]) -> f64 {
    let middle = &samples[samples.len() / 4..samples.len() * 3 / 4];
    (middle.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / middle.len() as f64).sqrt()
}

#[test]
fn speech_band_survives_the_new_rate_and_what_it_cannot_hold_is_gone() {
    let speech = to_whisper_rate(&tone(48_000, 1_000.0, 1.0), 48_000).unwrap();
    assert_eq!(speech.len(), 16_000);
    assert!((level(&speech) - 0.5 / 2f64.sqrt()).abs() < 0.01, "a 1 kHz tone keeps its level");
    // 12 kHz cannot be held at 16 kHz: it must not fold back as a false tone.
    let high = to_whisper_rate(&tone(48_000, 12_000.0, 1.0), 48_000).unwrap();
    assert!(level(&high) < 0.01, "level {}", level(&high));
    assert_eq!(to_whisper_rate(&tone(44_100, 1_000.0, 1.0), 44_100).unwrap().len(), 16_000);
}

#[test]
fn a_wav_written_reads_back_and_another_format_is_refused() {
    let folder = std::env::temp_dir().join(format!("qnc-transcribe-test-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let wav = folder.join("sound.wav");
    let samples = tone(WHISPER_RATE, 440.0, 0.5);
    write_wav(&wav, &samples).unwrap();
    let back = read_wav(&wav).unwrap();
    assert_eq!(back.len(), samples.len());
    assert!(back.iter().zip(&samples).all(|(a, b)| (a - b).abs() < 1.0 / 16_000.0));
    let mut stereo = std::fs::read(&wav).unwrap();
    stereo[22] = 2; // two channels
    std::fs::write(&wav, stereo).unwrap();
    assert!(read_wav(&wav).is_err());
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn a_job_and_a_segment_keep_the_contract_shape() {
    let job = Transcribe {
        model: "m.bin".into(),
        vad_model: None,
        backend: Backend::Vulkan,
        wav: "a.wav".into(),
        language: "hr".into(),
        threads: 4,
    };
    let value = serde_json::to_value(&job).unwrap();
    assert_eq!(value["backend"], "vulkan");
    assert!(value.get("vad_model").is_none());
    assert_eq!(serde_json::from_value::<Transcribe>(value).unwrap(), job);
    let segment: Segment = serde_json::from_str(
        r#"{"start":0.5,"end":1.25,"text":"Dobar dan","words":[{"start":0.5,"end":0.8,"text":"Dobar","p":0.9}]}"#,
    )
    .unwrap();
    assert_eq!(segment.words[0].text, "Dobar");
}
