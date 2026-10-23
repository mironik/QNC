use super::*;
use qnc_mxf_index::Entry;

/// A Long GOP of 6 (I B B P B B in display order, stored I P B B P B B ...), as the
/// cameras write it: entry d says which stored picture is shown at place d.
fn index(groups: usize) -> MxfIndex {
    let mut entries = Vec::new();
    for g in 0..groups {
        // stored order per group: I, B, B, P, B, B  (the Bs before I and P shown first)
        let pattern: [(i8, u8); 6] = [(1, 0x40), (1, 0x33), (-2, 0x33), (1, 0x22), (1, 0x33), (-2, 0x33)];
        for (i, (to, flags)) in pattern.iter().enumerate() {
            let k = g * 6 + i;
            let key = -((k - g * 6) as i8);
            entries.push(Entry { temporal_offset: *to, key_frame_offset: key, flags: *flags, stream_offset: k as u64 * 1024 });
        }
    }
    // The last places show what follows; keep the offsets inside the file.
    let n = entries.len();
    entries[n - 2].temporal_offset = 0;
    entries[n - 1].temporal_offset = 0;
    MxfIndex { rate: (50, 1), essence_start: 0, entries }
}

#[test]
fn a_part_starts_on_the_intra_picture_before_its_in_and_covers_its_out() {
    let index = index(6);
    let plan = plan(&index, 8, 15).unwrap();
    assert_eq!(plan.first_frame, 8, "the intra picture stored at 6 is shown at 8");
    assert!(plan.first_frame + plan.frames >= 15);
    assert!(!plan.stored.contains(&7) && !plan.stored.contains(&8), "the pictures shown before it are left out");
    assert_eq!(plan.stored[0], 6);
}

#[test]
fn a_range_outside_the_clip_is_refused() {
    let index = index(2);
    assert!(plan(&index, 5, 5).is_err());
    assert!(plan(&index, 0, 13).is_err());
}

/// On a real original (`QNC_MXF_SAMPLE`, read only), the part is written beside the
/// system temp folder and read back: its index says the frames of the plan.
#[test]
fn a_real_original_is_cut_into_an_mxf_whose_index_has_the_frames_of_the_part() {
    let Ok(sample) = std::env::var("QNC_MXF_SAMPLE") else { return };
    let source = std::path::Path::new(&sample);
    let index = MxfIndex::read(source).unwrap().unwrap();
    let plan = plan(&index, 450, 950).unwrap();
    let dest = std::env::temp_dir().join("qnc-mxf-cut-test.mxf");
    write(source, &index, &plan, &dest, &AtomicBool::new(false)).unwrap();
    let cut = MxfIndex::read(&dest).unwrap().expect("the part has an index");
    assert_eq!(cut.entries.len() as u64, plan.frames);
    let _ = std::fs::remove_file(dest);
}
