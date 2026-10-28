use super::*;

const W: usize = 32;
const H: usize = 18;

/// A picture of a scene: stripes of `level` and `level + 60`, shifted by `shift` (a moving
/// camera shifts them), or flat when `stripes` is false (no detail: soft).
fn scene(frame: u64, level: u8, shift: usize, stripes: bool) -> Picture {
    let luma = (0..W * H)
        .map(|i| {
            let x = i % W + shift;
            if stripes && (x / 2) % 2 == 0 { level.saturating_add(60) } else { level }
        })
        .collect();
    Picture { frame, width: W, height: H, luma }
}

#[test]
fn a_change_of_scene_starts_a_shot_and_each_shot_has_its_look() {
    let mut pictures = Vec::new();
    // 0-5 s: a still bright scene; 6-11 s: a moving dark-grey scene; 12-15 s: black.
    for second in 0..6 {
        pictures.push(scene(second * 50, 150, 0, true));
    }
    for second in 6..12 {
        pictures.push(scene(second * 50, 70, second as usize, true));
    }
    for second in 12..16 {
        pictures.push(scene(second * 50, 5, 0, false));
    }
    let shots = shots(pictures, 800);
    assert_eq!(shots.iter().map(|s| (s.from, s.to)).collect::<Vec<_>>(), [(0, 300), (300, 600), (600, 800)]);
    assert_eq!(shots[0].looks, [Look::Still]);
    assert_eq!(shots[1].looks, [Look::Moving]);
    assert_eq!(shots[2].looks, [Look::Black]);
}

#[test]
fn a_scene_without_detail_beside_sharp_ones_is_soft() {
    let mut pictures = Vec::new();
    for second in 0..5 {
        pictures.push(scene(second * 25, 120, 0, true));
    }
    for second in 5..10 {
        pictures.push(scene(second * 25, 190, 0, false));
    }
    let shots = shots(pictures, 250);
    assert_eq!(shots.len(), 2);
    assert!(shots[1].looks.contains(&Look::Soft), "{:?}", shots[1].looks);
    assert!(!shots[0].looks.contains(&Look::Soft));
}

#[test]
fn a_one_picture_flash_is_no_shot_and_no_pictures_give_no_shots() {
    let mut pictures: Vec<_> = (0..8).map(|second| scene(second * 50, 140, 0, true)).collect();
    pictures[4] = scene(200, 10, 0, false);
    let shots_found = shots(pictures, 400);
    assert_eq!(shots_found.len(), 1, "{shots_found:?}");
    assert_eq!((shots_found[0].from, shots_found[0].to), (0, 400));
    assert!(shots(Vec::new(), 100).is_empty());
}

/// A wider picture with detail everywhere (a checker of `cell` pixels), shifted by `shift`.
fn detailed(frame: u64, shift: i32, level: u8) -> Picture {
    let (w, h) = (160usize, 90usize);
    let luma = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as i32 + shift, (i / w) as i32);
            let n = (x.rem_euclid(97) * 13 + y * 7 + (x / 5) * (y / 5)) % 23;
            level.saturating_add((n * 6) as u8)
        })
        .collect();
    Picture { frame, width: w, height: h, luma }
}

#[test]
fn a_shift_of_the_whole_picture_is_found() {
    let moved = camera_move(&detailed(0, 0, 60), &detailed(25, 7, 60));
    assert_eq!((moved.dx, moved.dy), (-7, 0), "{moved:?}");
    let still = camera_move(&detailed(0, 0, 60), &detailed(25, 0, 60));
    assert_eq!((still.dx, still.dy), (0, 0));
}

#[test]
fn a_take_gives_its_steady_part_and_its_even_pan_but_not_its_jerks() {
    // 50 fps, a picture every 25 frames: 0-6 s the camera stands, 6-12 s it jerks, 12-18 s it
    // pans evenly 4 pixels a picture.
    let mut pictures = Vec::new();
    let mut shift = 0;
    for i in 0..73u64 {
        let second = i as f64 * 0.5;
        if (6.0..12.0).contains(&second) {
            shift += if i % 2 == 0 { 25 } else { -22 };
        } else if second >= 12.0 {
            shift += 4;
        }
        pictures.push(detailed(i * 25, shift, 60));
    }
    let frames = 73 * 25;
    let found_shots = shots(pictures.clone(), frames);
    let found = parts(pictures, &found_shots, 50.0);
    let summary: Vec<_> = found.iter().map(|p| (p.from, p.to, p.camera)).collect();
    assert_eq!(summary.len(), 2, "{summary:?}");
    assert_eq!(summary[0].2, Camera::Steady);
    assert!(summary[0].0 == 0 && summary[0].1 >= 275, "{summary:?}");
    assert_eq!(summary[1].2, Camera::Pan);
    assert!(summary[1].0 >= 550 && summary[1].1 == 1800, "{summary:?}");
}
