use crate::*;
use std::time::{Duration, Instant};

fn config() -> OutputConfig {
    OutputConfig {
        version: VERSION.into(),
        session_id: "test-session".into(),
        generation: 1,
        width: 8,
        height: 4,
        pixel_format: PixelFormat::Rgba8Srgb,
        slots: 2,
        pool_budget_bytes: 256,
    }
}

fn header(sequence: u64) -> FrameHeader {
    FrameHeader {
        version: VERSION.into(),
        session_id: "test-session".into(),
        generation: 1,
        sequence,
        source_id: "test-pattern".into(),
        frame_number: sequence,
        width: 8,
        height: 4,
        pixel_format: PixelFormat::Rgba8Srgb,
    }
}

#[test]
fn version_is_not_guessed() {
    let mut c = config();
    c.version = "0.0.0".into();
    assert_eq!(c.validate(), Err(OutputError::Version));
    let mut h = header(1);
    h.version = "future".into();
    assert_eq!(h.validate(&config(), 128), Err(OutputError::Version));
}

#[test]
fn pool_limits_count_all_source_slots() {
    assert!(config().validate().is_ok());
    let mut c = config();
    c.slots = 3;
    assert_eq!(c.validate(), Err(OutputError::Budget));
    c.slots = 0;
    assert_eq!(c.validate(), Err(OutputError::Budget));
    c.slots = MAX_OUTPUT_SLOTS + 1;
    c.pool_budget_bytes = MAX_POOL_BYTES;
    assert_eq!(c.validate(), Err(OutputError::Budget));
    c.slots = MAX_OUTPUT_SLOTS;
    c.pool_budget_bytes = MAX_OUTPUT_SLOTS as u64 * 128;
    assert!(c.validate().is_ok());
    c.slots = 1;
    c.pool_budget_bytes = MAX_POOL_BYTES + 1;
    assert_eq!(c.validate(), Err(OutputError::Budget));
}

#[test]
fn zero_dimensions_and_overflow_are_rejected() {
    let mut c = config();
    c.width = 0;
    assert_eq!(c.validate(), Err(OutputError::Dimensions));
    c.width = u32::MAX;
    c.height = u32::MAX;
    assert_eq!(c.validate(), Err(OutputError::Dimensions));
}

#[test]
fn session_and_generation_are_required() {
    let mut c = config();
    c.session_id.clear();
    assert_eq!(c.validate(), Err(OutputError::Identity));
    c = config();
    c.generation = 0;
    assert_eq!(c.validate(), Err(OutputError::Identity));
    let mut h = header(1);
    h.generation = 2;
    assert_eq!(h.validate(&config(), 128), Err(OutputError::Identity));
    h = header(1);
    h.session_id = "other".into();
    assert_eq!(h.validate(&config(), 128), Err(OutputError::Identity));
}

#[test]
fn metadata_and_payload_size_must_agree_exactly() {
    assert!(header(1).validate(&config(), 128).is_ok());
    for size in [0, 127, 129] {
        assert_eq!(
            header(1).validate(&config(), size),
            Err(OutputError::Pixels)
        );
    }
    let mut h = header(1);
    h.width = 4;
    assert_eq!(h.validate(&config(), 128), Err(OutputError::Pixels));
}

#[test]
fn wire_header_has_no_implicit_pixel_conversion() {
    let h = header(1);
    let json = serde_json::to_string(&h).unwrap();
    assert_eq!(serde_json::from_str::<FrameHeader>(&json).unwrap(), h);
    assert!(
        serde_json::from_str::<FrameHeader>(&json.replace("rgba8_srgb", "yuv422p10le")).is_err()
    );
    let mut value = serde_json::to_value(h).unwrap();
    value["local_path"] = serde_json::json!("hidden-path");
    assert!(serde_json::from_value::<FrameHeader>(value).is_err());
}

#[test]
fn identity_strings_are_bounded() {
    let mut h = header(1);
    h.source_id = "x".repeat(513);
    assert_eq!(h.validate(&config(), 128), Err(OutputError::Identity));
    h.source_id = "bad\nidentity".into();
    assert_eq!(h.validate(&config(), 128), Err(OutputError::Identity));
}

#[test]
fn letterbox_preserves_display_aspect_ratio() {
    assert_eq!(crate::model::viewport((8, 4), (8, 8)), (0.0, 2.0, 8.0, 4.0));
    assert_eq!(crate::model::viewport((4, 8), (8, 8)), (2.0, 0.0, 4.0, 8.0));
    assert_eq!(
        crate::model::viewport((8, 4), (16, 8)),
        (0.0, 0.0, 16.0, 8.0)
    );
}

#[test]
fn completion_reports_gpu_not_scanout() {
    let completion = GpuCompletion {
        submission: Submission {
            version: VERSION.into(),
            id: 1,
            frame: header(7),
            target: SubmissionTarget::Offscreen,
        },
    };
    let json = serde_json::to_string(&completion).unwrap();
    assert!(!json.contains("presented"));
    assert_eq!(
        serde_json::from_str::<GpuCompletion>(&json).unwrap(),
        completion
    );
}

fn wait_ready(output: &mut VideoOutput, frame: &PreparedFrame) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        output.poll().unwrap();
        if output.is_ready(frame).unwrap() {
            return;
        }
        assert!(Instant::now() < deadline, "GPU upload timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn wait_done(output: &mut VideoOutput) -> GpuCompletion {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(done) = output.poll().unwrap() {
            return done;
        }
        assert!(Instant::now() < deadline, "GPU output timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn open() -> VideoOutput {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("live GPU adapter is required, not a skipped pass");
    println!("GPU adapter: {:?}", adapter.get_info());
    pollster::block_on(VideoOutput::open(&adapter, None, config(), (8, 8))).unwrap()
}

fn read_pixels(output: &VideoOutput, width: u32, height: u32) -> Vec<u8> {
    let stride = (width * 4).div_ceil(256) * 256;
    let buffer = output.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test-only GPU pixel readback"),
        size: u64::from(stride * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = output.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: output.offscreen_texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    output.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
    output.device.poll(wgpu::Maintain::Wait);
    rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let pixels = mapped
        .chunks_exact(stride as usize)
        .flat_map(|row| &row[..width as usize * 4])
        .copied()
        .collect();
    drop(mapped);
    buffer.unmap();
    pixels
}

#[test]
#[ignore = "Requires a real graphics device; run explicitly for output verification"]
fn live_gpu_pixels_and_lifecycle() {
    let mut output = open();
    let pixels: Vec<u8> = (0..4)
        .flat_map(|y| {
            (0..8).flat_map(move |x| match (x < 4, y < 2) {
                (true, true) => [255, 0, 0, 255],
                (false, true) => [0, 255, 0, 255],
                (true, false) => [0, 0, 255, 255],
                (false, false) => [128, 128, 128, 255],
            })
        })
        .collect();
    let first = output.prepare(header(1), &pixels).unwrap();
    if !output.is_ready(&first).unwrap() {
        assert_eq!(output.reset(2), Err(OutputError::Busy));
        assert!(matches!(output.submit(&first), Err(OutputError::NotReady)));
    }
    wait_ready(&mut output, &first);
    assert!(matches!(
        output.prepare(header(1), &pixels),
        Err(OutputError::Sequence)
    ));
    let second = output.prepare(header(2), &[255; 128]).unwrap();
    assert!(matches!(
        output.prepare(header(3), &pixels),
        Err(OutputError::Full)
    ));
    wait_ready(&mut output, &second);
    let start = Instant::now();
    let submitted = output.submit(&first).unwrap();
    println!("Prepared submit API: {:?}", start.elapsed());
    assert!(matches!(output.submit(&second), Err(OutputError::Busy)));
    assert_eq!(output.release(&first), Err(OutputError::Busy));
    assert_eq!(wait_done(&mut output).submission, submitted);
    println!("GPU completion observed: {:?}", start.elapsed());
    let actual = read_pixels(&output, 8, 8);
    for y in 0..8 {
        for x in 0..8 {
            let expected: &[u8] = if !(2..6).contains(&y) {
                &[0, 0, 0, 255]
            } else {
                &pixels[((y - 2) * 8 + x) * 4..((y - 2) * 8 + x) * 4 + 4]
            };
            assert_eq!(
                &actual[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4],
                expected,
                "pixel {x},{y}"
            );
        }
    }
    // Repeat submission keeps the prepared frame; a pause needs no new upload.
    output.submit(&first).unwrap();
    wait_done(&mut output);
    output.submit(&second).unwrap();
    wait_done(&mut output);
    assert_ne!(actual, read_pixels(&output, 8, 8));
    output.release(&first).unwrap();
    assert_eq!(output.is_ready(&first), Err(OutputError::StaleToken));
    let reused = output.prepare(header(3), &pixels).unwrap();
    wait_ready(&mut output, &reused);
    assert_eq!(output.is_ready(&first), Err(OutputError::StaleToken));
    let other = open();
    assert_eq!(other.is_ready(&reused), Err(OutputError::StaleToken));
    output.reset(2).unwrap();
    assert_eq!(output.is_ready(&reused), Err(OutputError::StaleToken));
    assert!(matches!(
        output.prepare(header(4), &pixels),
        Err(OutputError::Identity)
    ));
    let mut next = header(1);
    next.generation = 2;
    let third = output.prepare(next.clone(), &pixels).unwrap();
    wait_ready(&mut output, &third);
    output.resize((0, 0)).unwrap();
    assert!(matches!(output.submit(&third), Err(OutputError::Suspended)));
    assert!(matches!(
        output.prepare(next.clone(), &pixels),
        Err(OutputError::Suspended)
    ));
    output.resize((16, 8)).unwrap();
    assert_eq!(output.is_ready(&third), Err(OutputError::StaleToken));
    next.sequence = 2;
    let resized = output.prepare(next, &pixels).unwrap();
    wait_ready(&mut output, &resized);
    output.submit(&resized).unwrap();
    wait_done(&mut output);
    let actual = read_pixels(&output, 16, 8);
    assert_eq!(&actual[0..4], &[255, 0, 0, 255]);
    assert_eq!(&actual[16 * 7 * 4..16 * 7 * 4 + 4], &[0, 0, 255, 255]);
    output.device.destroy();
    assert!(matches!(output.poll(), Err(OutputError::Device(_))));
    assert!(matches!(
        output.submit(&resized),
        Err(OutputError::Device(_))
    ));
    assert!(matches!(
        output.is_ready(&resized),
        Err(OutputError::Device(_))
    ));
    println!(
        "GPU pixel readback, two images, retained frame, pool reuse, token isolation, resize and device loss: PASS"
    );
}
