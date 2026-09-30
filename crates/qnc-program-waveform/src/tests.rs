use super::*;

fn at(clip_id: &str, channel: u16, program: (u64, u64), source: (u64, u64)) -> Placement {
    Placement {
        clip_id: clip_id.into(),
        channel,
        program_start: program.0,
        program_end: program.1,
        source_in: source.0,
        source_out: source.1,
    }
}

fn wave(lanes: Vec<Vec<f32>>, duration_frames: u64) -> ClipWave {
    ClipWave { lanes, duration_frames }
}

// v5 `base_segments_map_source_peaks_to_program_a1`.
#[test]
fn base_segments_map_source_peaks_to_program_a1() {
    let waves = HashMap::from([("clip_a".to_string(), wave(vec![vec![0.1, 0.8, 0.2, 0.3]], 400))]);
    let peaks = compose(100, &[at("clip_a", 0, (0, 100), (100, 200))], &[], &waves);
    assert!(peaks.a1.iter().any(|peak| *peak >= 0.8));
    assert!(peaks.a2.iter().all(|peak| *peak == 0.0));
}

// v5 `covers_map_primary_source_audio_to_program_a2`.
#[test]
fn covers_map_source_audio_to_program_a2() {
    let waves = HashMap::from([
        ("clip_a".to_string(), wave(vec![vec![0.1]], 100)),
        ("clip_b".to_string(), wave(vec![vec![0.1, 0.2, 0.9, 0.3]], 400)),
    ]);
    let peaks = compose(
        100,
        &[at("clip_a", 0, (0, 100), (0, 100))],
        &[at("clip_b", 0, (25, 75), (200, 300))],
        &waves,
    );
    assert!(peaks.a2.iter().any(|peak| *peak >= 0.9));
    // The cover sits in the middle of the program only.
    assert_eq!(peaks.a2[0], 0.0);
}

// v5 `base_segment_uses_a2_when_a1_is_missing`.
#[test]
fn channel_one_uses_the_next_lane_when_the_first_is_empty() {
    let waves = HashMap::from([("clip_a".to_string(), wave(vec![vec![], vec![0.0, 0.6, 0.0]], 50))]);
    let peaks = compose(50, &[at("clip_a", 0, (0, 50), (0, 50))], &[], &waves);
    assert!(peaks.a1.iter().any(|peak| *peak >= 0.6));
}

#[test]
fn the_chosen_channel_is_drawn_and_a_missing_one_draws_nothing() {
    let lanes = vec![vec![0.1; 4], vec![0.1; 4], vec![0.7; 4]];
    let waves = HashMap::from([("clip_a".to_string(), wave(lanes, 40))]);
    let third = compose(40, &[at("clip_a", 2, (0, 40), (0, 40))], &[], &waves);
    assert!(third.a1.iter().all(|peak| *peak == 0.7));
    let fourth = compose(40, &[at("clip_a", 3, (0, 40), (0, 40))], &[], &waves);
    assert!(fourth.a1.iter().all(|peak| *peak == 0.0));
}

#[test]
fn a_row_takes_its_own_part_of_the_program_wave() {
    let mut program = vec![0.0; 100];
    program[80..].fill(0.5);
    let first = row_peaks(&program, 100, 0, 50);
    let second = row_peaks(&program, 100, 50, 100);
    assert!(first.iter().all(|peak| *peak == 0.0));
    assert!(second.iter().any(|peak| *peak == 0.5));
    assert!(row_peaks(&[], 100, 0, 50).is_empty());
}

#[test]
fn an_empty_program_has_no_wave() {
    assert_eq!(compose(0, &[], &[], &HashMap::new()), ProgramPeaks::default());
}
