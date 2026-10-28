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

#[test]
fn a_long_sound_is_cut_at_its_quietest_place_near_each_piece_end() {
    let rate = 16_000;
    let mut samples = vec![0.5f32; 40 * rate];
    let quiet = 27 * rate;
    samples[quiet..quiet + rate / 10].fill(0.0);
    let pieces = quiet_pieces(&samples, rate, 30);
    assert_eq!(pieces, vec![0..quiet + rate / 20, quiet + rate / 20..samples.len()]);
    assert_eq!(quiet_pieces(&samples[..rate], rate, 30), vec![0..rate], "a short sound is one piece");
}

#[test]
fn a_piece_whose_last_seconds_are_all_speech_ends_at_the_pause_before_them() {
    let rate = 16_000;
    let mut samples = vec![0.5f32; 40 * rate];
    let pause = 21 * rate;
    samples[pause..pause + rate].fill(0.0);
    let gap = 28 * rate; // between two words: quieter, but not a pause
    samples[gap..gap + rate / 10].fill(0.01);
    let pieces = quiet_pieces(&samples, rate, 30);
    assert!(pieces[0].end > pause && pieces[0].end < pause + rate, "cut in the pause, not at the gap: {pieces:?}");
}

#[test]
fn only_sound_over_the_floor_is_given_to_the_engine() {
    let rate = 1000;
    // 2 s quiet, 1 s loud, 0.5 s quiet, 1 s loud, 5 s quiet.
    let mut samples = vec![0.0001f32; 2 * rate];
    samples.extend(vec![0.3f32; rate]);
    samples.extend(vec![0.0001f32; rate / 2]);
    samples.extend(vec![0.3f32; rate]);
    samples.extend(vec![0.0001f32; 5 * rate]);
    let spans = crate::loud_spans(&samples, rate, -42.0);
    assert_eq!(spans.len(), 1, "a short pause joins the two loud parts");
    assert_eq!((spans[0].start, spans[0].end), (2 * rate - rate / 3, 4500 + rate / 3));
    assert!(crate::loud_spans(&vec![0.0001f32; 3 * rate], rate, -42.0).is_empty());
}
