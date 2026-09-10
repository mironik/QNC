#![cfg(test)]
use crate::*;
use qnc_media_metadata::{ColorMetadata, Fact, ScanMode, Signal, VideoMetadata};

pub(crate) fn spec(layout: PixelLayout) -> ConversionSpec {
    ConversionSpec {
        version: VERSION.into(),
        width: 4,
        height: 2,
        layout,
        primaries: "bt709".into(),
        matrix: "bt709".into(),
        scan_mode: ScanMode::Progressive,
        range: Range::Limited,
        transfer: Transfer::Bt709,
    }
}
fn fact<T>(value: T) -> Option<Fact<T>> {
    Some(Fact {
        value,
        evidence_id: "saved".into(),
        locator: "/stream".into(),
    })
}
fn signal(value: &str) -> Option<Fact<Signal<String>>> {
    fact(Signal::Known(value.into()))
}
fn saved() -> VideoMetadata {
    VideoMetadata {
        width: fact(4),
        height: fact(2),
        frame_rate: None,
        frame_rate_mode: None,
        frame_count: None,
        scan_mode: fact(ScanMode::Progressive),
        pixel_format: fact("yuv420p".into()),
        sample_aspect_ratio: None,
        rotation_degrees: None,
        color: ColorMetadata {
            primaries: signal("bt709"),
            matrix: signal("bt709"),
            transfer: signal("bt709"),
            range: signal("tv"),
        },
    }
}
pub(crate) fn gray(spec: &ConversionSpec, luma: u16) -> Vec<u8> {
    let pixels = spec.width as usize * spec.height as usize;
    let n = spec.input_bytes().unwrap() / if spec.layout.ten_bit() { 2 } else { 1 };
    let center = if spec.layout.ten_bit() { 512 } else { 128 };
    (0..n)
        .flat_map(|i| {
            let value = if i < pixels { luma } else { center };
            if spec.layout.ten_bit() {
                value.to_le_bytes().to_vec()
            } else {
                vec![value as u8]
            }
        })
        .collect()
}
fn converted(spec: ConversionSpec, luma: u16) -> Vec<u8> {
    let input = gray(&spec, luma);
    let mut output = vec![0; spec.output_bytes().unwrap()];
    Converter::prepare(spec.clone(), spec.scratch_bytes().unwrap())
        .unwrap()
        .convert(&input, &mut output)
        .unwrap();
    output
}

#[test]
fn saved_facts_create_spec_without_fps_or_application_state() {
    assert_eq!(
        ConversionSpec::from_saved(&saved()).unwrap(),
        spec(PixelLayout::Yuv420p)
    );
}

#[test]
fn public_payload_validation_checks_exact_layout_and_ten_bit_words() {
    for layout in [
        PixelLayout::Yuv420p,
        PixelLayout::Yuv422p,
        PixelLayout::Yuv444p,
        PixelLayout::Yuv420p10le,
        PixelLayout::Yuv422p10le,
        PixelLayout::Yuv444p10le,
    ] {
        let s = spec(layout);
        let mut input = gray(&s, 128);
        assert!(s.validate_payload(&input).is_ok());
        assert_eq!(
            s.validate_payload(&input[..input.len() - 1]),
            Err(ConversionError::Payload)
        );
        if layout.ten_bit() {
            input[..2].copy_from_slice(&1024u16.to_le_bytes());
            assert_eq!(s.validate_payload(&input), Err(ConversionError::BitDepth));
        }
    }
}
#[test]
fn missing_and_unspecified_are_not_rec709_defaults() {
    let mut video = saved();
    video.color.matrix = None;
    assert_eq!(
        ConversionSpec::from_saved(&video),
        Err(ConversionError::Missing("color.matrix"))
    );
    video = saved();
    video.color.transfer = fact(Signal::Unspecified);
    assert_eq!(
        ConversionSpec::from_saved(&video),
        Err(ConversionError::Missing("color.transfer"))
    );
    video = saved();
    video.pixel_format = None;
    assert_eq!(
        ConversionSpec::from_saved(&video),
        Err(ConversionError::Missing("pixel_format"))
    );
}
#[test]
fn hdr_interlace_and_unimplemented_layouts_are_rejected() {
    for (transfer, primaries, pixel, scan) in [
        ("smpte2084", "bt709", "yuv420p", ScanMode::Progressive),
        ("bt709", "bt2020", "yuv420p", ScanMode::Progressive),
        ("bt709", "bt709", "p010le", ScanMode::Progressive),
        (
            "bt709",
            "bt709",
            "yuv420p",
            ScanMode::InterlacedTopFieldFirst,
        ),
    ] {
        let mut video = saved();
        video.color.transfer = signal(transfer);
        video.color.primaries = signal(primaries);
        video.pixel_format = fact(pixel.into());
        video.scan_mode = fact(scan);
        assert!(matches!(
            ConversionSpec::from_saved(&video),
            Err(ConversionError::Unsupported(_))
        ));
    }
}
#[test]
fn wire_payload_cannot_bypass_spec_validation() {
    let mut value = serde_json::to_value(spec(PixelLayout::Yuv420p)).unwrap();
    value["matrix"] = serde_json::json!("bt601");
    let forged: ConversionSpec = serde_json::from_value(value.clone()).unwrap();
    assert!(matches!(
        Converter::prepare(forged, 1024),
        Err(ConversionError::Unsupported("color.matrix"))
    ));
    value["project_path"] = serde_json::json!("forbidden");
    assert!(serde_json::from_value::<ConversionSpec>(value).is_err());
    let mut invalid = spec(PixelLayout::Yuv420p);
    invalid.version = "future".into();
    assert!(matches!(
        Converter::prepare(invalid, 1024),
        Err(ConversionError::Version)
    ));
}
#[test]
fn dimensions_and_scratch_are_bounded_before_allocation() {
    let mut s = spec(PixelLayout::Yuv444p10le);
    s.width = u32::MAX;
    assert!(matches!(
        Converter::prepare(s, 1024),
        Err(ConversionError::Dimensions)
    ));
    let mut s = spec(PixelLayout::Yuv444p10le);
    s.height = 0;
    assert!(matches!(
        Converter::prepare(s, 1024),
        Err(ConversionError::Dimensions)
    ));
    let s = spec(PixelLayout::Yuv422p10le);
    assert!(matches!(
        Converter::prepare(s.clone(), s.scratch_bytes().unwrap() - 1),
        Err(ConversionError::Budget)
    ));
    assert!(matches!(
        Converter::prepare(s, crate::model::MAX_BYTES + 1),
        Err(ConversionError::Budget)
    ));
}
#[test]
fn limited_black_and_white_for_all_six_layouts() {
    for layout in [
        PixelLayout::Yuv420p,
        PixelLayout::Yuv422p,
        PixelLayout::Yuv444p,
        PixelLayout::Yuv420p10le,
        PixelLayout::Yuv422p10le,
        PixelLayout::Yuv444p10le,
    ] {
        let s = spec(layout);
        let scale = if layout.ten_bit() { 4 } else { 1 };
        for (luma, expected) in [(16 * scale, 0), (235 * scale, 255)] {
            let pixels = converted(s.clone(), luma);
            assert!(
                pixels
                    .chunks_exact(4)
                    .all(|p| p == [expected, expected, expected, 255]),
                "{layout:?} {luma}: {pixels:?}"
            );
        }
    }
}
#[test]
fn full_range_differs_from_limited_and_srgb_from_bt709() {
    let mut full = spec(PixelLayout::Yuv444p);
    full.range = Range::Full;
    full.transfer = Transfer::Srgb;
    assert_eq!(&converted(full.clone(), 0)[..4], &[0, 0, 0, 255]);
    assert_eq!(&converted(full.clone(), 255)[..4], &[255; 4]);
    // SIMD fixed-point matrix conversion may round by one encoded RGB code.
    assert!(
        converted(full.clone(), 128)
            .chunks_exact(4)
            .all(|p| p[..3].iter().all(|v| v.abs_diff(128) <= 1) && p[3] == 255)
    );
    let mut bt709 = full.clone();
    bt709.transfer = Transfer::Bt709;
    let mid = converted(bt709, 128)[0];
    assert!((139..=141).contains(&mid), "{mid}");
    let mut limited = full.clone();
    limited.range = Range::Limited;
    assert_ne!(converted(full, 16), converted(limited, 16));
}
#[test]
fn uv_planes_are_not_swapped() {
    let s = spec(PixelLayout::Yuv444p);
    let mut input = vec![63; 8];
    input.extend([102; 8]);
    input.extend([240; 8]);
    let mut pixels = vec![0; 32];
    Converter::prepare(s, 1024)
        .unwrap()
        .convert(&input, &mut pixels)
        .unwrap();
    assert!(
        pixels
            .chunks_exact(4)
            .all(|p| p[0] >= 250 && p[1] < 15 && p[2] < 15 && p[3] == 255),
        "{pixels:?}"
    );
}
#[test]
fn odd_sizes_and_row_order_preserve_pixels() {
    for layout in [PixelLayout::Yuv420p, PixelLayout::Yuv422p10le] {
        let mut s = spec(layout);
        s.width = 3;
        s.height = 3;
        let pixels = converted(s, if layout.ten_bit() { 940 } else { 235 });
        assert_eq!(pixels, vec![255; 36]);
    }
    let s = spec(PixelLayout::Yuv420p);
    let mut input = gray(&s, 16);
    input[4..8].fill(235);
    let mut pixels = vec![0; 32];
    Converter::prepare(s, 1024)
        .unwrap()
        .convert(&input, &mut pixels)
        .unwrap();
    assert_eq!(&pixels[..4], &[0, 0, 0, 255]);
    assert_eq!(&pixels[16..20], &[255; 4]);
}
#[test]
fn malformed_payloads_leave_output_untouched() {
    let s = spec(PixelLayout::Yuv422p10le);
    let mut input = gray(&s, 512);
    let mut converter = Converter::prepare(s.clone(), s.scratch_bytes().unwrap()).unwrap();
    let mut pixels = vec![21; 32];
    assert_eq!(
        converter.convert(&input[..input.len() - 1], &mut pixels),
        Err(ConversionError::Payload)
    );
    input[0..2].copy_from_slice(&1024_u16.to_le_bytes());
    assert_eq!(
        converter.convert(&input, &mut pixels),
        Err(ConversionError::BitDepth)
    );
    assert_eq!(pixels, vec![21; 32]);
}
#[test]
fn input_and_saved_description_are_not_mutated() {
    let video = saved();
    let before = video.clone();
    let s = ConversionSpec::from_saved(&video).unwrap();
    let input = gray(&s, 126);
    let old = input.clone();
    let mut pixels = vec![0; 32];
    Converter::prepare(s, 1024)
        .unwrap()
        .convert(&input, &mut pixels)
        .unwrap();
    assert_eq!(input, old);
    assert_eq!(video, before);
}
