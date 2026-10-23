use super::*;

#[test]
fn without_a_project_nothing_is_decoded() {
    let mut stills = TrimStills::new();
    let picture = StillPicture { clip_id: "c".into(), source_frame: 3 };
    let mut view = SegmentsView {
        edit_stills: vec![qnc_program_view::EditStill::new(10, true, Some(picture))],
        ..Default::default()
    };
    assert!(!stills.fill(&mut view, None, None));
    assert!(!stills.loading());
    assert!(view.edit_stills[0].image.is_none());
}

#[test]
fn the_pictures_ahead_of_the_arrow_are_wanted_in_its_direction() {
    let mut stills = TrimStills::new();
    let at = |frame| SegmentsView {
        edit_stills: vec![qnc_program_view::EditStill::new(
            10,
            true,
            Some(StillPicture { clip_id: "c".into(), source_frame: frame }),
        )],
        ..Default::default()
    };
    assert!(stills.ahead(&at(100)).is_empty(), "no direction before a move");
    let right = stills.ahead(&at(101));
    assert_eq!(right, vec![(StillPicture { clip_id: "c".into(), source_frame: 102 }, 102)]);
    let left = stills.ahead(&at(100));
    assert_eq!(left, vec![(StillPicture { clip_id: "c".into(), source_frame: 99 }, 99 - u64::from(RUN) + 1)]);
    // Standing still keeps the direction of the last move.
    assert_eq!(stills.ahead(&at(100)), left);
}

/// QNC_TRIM_FILE=<a local 50p media file, read only> QNC_TRIM_OUT=<scratch dir>.
#[test]
#[ignore = "reads a real media file"]
fn a_run_of_pictures_comes_out_in_order() {
    let file = PathBuf::from(std::env::var("QNC_TRIM_FILE").unwrap());
    let out = PathBuf::from(std::env::var("QNC_TRIM_OUT").unwrap());
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor().unwrap().unwrap();
    let started = Instant::now();
    extractor
        .extract_run_with_cancel(&file, (188.0 - 0.25) / 50.0, RUN, STILL_SIZE, &out, &AtomicBool::new(false))
        .unwrap();
    println!("run of {RUN} in {} ms", started.elapsed().as_millis());
}
