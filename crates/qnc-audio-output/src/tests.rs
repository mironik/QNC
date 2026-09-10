use super::*;
use queue::{Callback, Queue};

fn config(channels: u16, capacity: u32, ready: u32) -> Config {
    Config {
        version: VERSION.into(),
        format: Format {
            sample_rate_hz: 48_000,
            channels,
        },
        device_id: None,
        capacity_frames: capacity,
        ready_frames: ready,
    }
}
fn prepared(channels: u16) -> (Queue, Callback, u64) {
    let (mut q, mut c) = Queue::new(config(channels, 8, 2)).unwrap();
    let generation = q.reset(10, Status::Preparing).unwrap();
    c.render(&mut [0.0; 0], 0, None);
    assert!(q.acknowledged());
    (q, c, generation)
}

#[test]
fn driver_timing_tracks_buffer_start_and_source_anchor_not_queued_samples() {
    let (mut q, mut c, g) = prepared(1);
    q.push(g, 10, &[0.1; 8]).unwrap();
    q.commit(g).unwrap();
    assert!(q.driver_timing().is_none());
    q.start(g).unwrap();
    assert!(q.driver_timing().is_none());
    c.render(&mut [0.0; 2], 1_000_000, Some(10_000_000));
    let first = q.driver_timing().unwrap();
    assert_eq!(first.generation, g);
    assert_eq!(first.first_sample_frame, 10);
    assert_eq!(first.sample_rate_hz, 48_000);
    c.render(&mut [0.0; 2], 2_000_000, Some(12_000_000));
    let second = q.driver_timing().unwrap();
    assert_eq!(second.first_sample_frame, 12);
    assert_eq!(second.playback_unix_ns - first.playback_unix_ns, 3_000_000);
    c.render(&mut [0.0; 2], 3_000_000, None);
    assert!(q.driver_timing().is_none());
    c.render(&mut [0.0; 2], 4_000_000, Some(10_000_000));
    assert!(q.driver_timing().is_some());
    q.reset(100, Status::Preparing).unwrap();
    assert!(q.driver_timing().is_none());
    c.render(&mut [0.0; 2], 5_000_000, None);
    assert!(q.driver_timing().is_none());
}

#[test]
fn open_begin_queue_commit_remain_silent_until_explicit_start() {
    let (mut q, mut c, g) = prepared(2);
    assert_eq!(q.start(g).unwrap_err().code, Code::NotReady);
    q.push(g, 10, &[0.1, -0.1, 0.2, -0.2]).unwrap();
    q.commit(g).unwrap();
    let mut out = [1.0; 4];
    for _ in 0..100 {
        c.render(&mut out, 100, Some(5));
        assert_eq!(out, [0.0; 4]);
    }
    assert_eq!(q.telemetry().submitted_frames, 0);
    assert_eq!(q.telemetry().queued_frames, 2);
    assert_eq!(q.telemetry().start_to_first_callback_ns, None);
    q.start(g).unwrap();
    let started = q.shared.started_ns.load(Ordering::Acquire);
    c.render(&mut out, started + 100, Some(500));
    assert_eq!(out, [0.1, -0.1, 0.2, -0.2]);
    assert_eq!(q.telemetry().submitted_frames, 2);
    assert_eq!(q.telemetry().start_to_first_callback_ns, Some(100));
    assert_eq!(q.telemetry().first_driver_delay_ns, Some(500));
    assert_eq!(q.start(g).unwrap_err().code, Code::NotReady);
}

#[test]
fn media_clock_follows_device_buffers_and_never_runs_beyond_submitted_pcm() {
    let (mut q, mut callback) = Queue::new(config(1, 4800, 960)).unwrap();
    let g = q.reset(48_000, Status::Preparing).unwrap();
    callback.render(&mut [], 0, None);
    q.push(g, 48_000, &[0.1; 2400]).unwrap();
    q.commit(g).unwrap();
    q.start(g).unwrap();
    assert_eq!(q.playback_position_at(10_000_000), 1_000_000_000);
    callback.render(&mut [0.0; 480], 10_000_000, Some(10_000_000));
    assert_eq!(q.playback_position_at(10_000_000), 1_000_000_000);
    assert_eq!(q.playback_position_at(25_000_000), 1_005_000_000);
    assert_eq!(q.playback_position_at(99_000_000), 1_010_000_000);
    callback.render(&mut [0.0; 480], 20_000_000, Some(10_000_000));
    assert_eq!(q.playback_position_at(25_000_000), 1_005_000_000);
    assert_eq!(q.playback_position_at(35_000_000), 1_015_000_000);
    callback.render(&mut [0.0; 480], 30_000_000, None);
    assert_eq!(q.playback_position_at(40_000_000), 1_015_000_000);
    q.reset(96_000, Status::Preparing).unwrap();
    assert_eq!(q.playback_position_at(40_000_000), 2_000_000_000);
    callback.render(&mut [], 40_000_000, None);
    assert_eq!(q.playback_position_at(40_000_000), 2_000_000_000);
}

#[test]
fn pause_discards_old_generation_and_requires_fresh_preparation() {
    let (mut q, mut c, g) = prepared(1);
    q.push(g, 10, &[0.1, 0.2, 0.3, 0.4]).unwrap();
    q.commit(g).unwrap();
    q.start(g).unwrap();
    let mut out = [0.0; 2];
    c.render(&mut out, 0, None);
    assert_eq!(out, [0.1, 0.2]);
    let new = q.reset(50, Status::Preparing).unwrap();
    assert_eq!(
        q.push(new, 50, &[0.9, 0.8]).unwrap_err().code,
        Code::NotReady
    );
    c.render(&mut out, 0, None);
    assert_eq!(out, [0.0; 2]);
    assert_eq!(q.start(g).unwrap_err().code, Code::StaleGeneration);
    assert_eq!(
        q.push(g, 50, &[0.1]).unwrap_err().code,
        Code::StaleGeneration
    );
    q.push(new, 50, &[0.9, 0.8]).unwrap();
    q.commit(new).unwrap();
    q.start(new).unwrap();
    c.render(&mut out, 0, None);
    assert_eq!(out, [0.9, 0.8]);
    assert_eq!(q.telemetry().submitted_frames, 2);
}

#[test]
fn overflow_rejects_entire_block_and_preserves_next_position() {
    let (mut q, mut c, g) = prepared(1);
    q.push(g, 10, &[0.1; 7]).unwrap();
    assert_eq!(q.push(g, 17, &[0.9; 2]).unwrap_err().code, Code::Full);
    assert_eq!(q.telemetry().queued_frames, 7);
    q.push(g, 17, &[0.2]).unwrap();
    q.finish(g).unwrap();
    q.commit(g).unwrap();
    q.start(g).unwrap();
    let mut out = [0.0; 8];
    c.render(&mut out, 0, None);
    assert_eq!(&out[..7], &[0.1; 7]);
    assert_eq!(out[7], 0.2);
}

#[test]
fn queue_never_accepts_partial_channels_nonfinite_clipped_or_discontinuous_data() {
    let (mut q2, callback, g2) = prepared(2);
    for data in [
        &[0.1][..],
        &[0.1, f32::NAN],
        &[f32::INFINITY, 0.0],
        &[1.01, 0.0],
        &[],
    ] {
        assert_eq!(q2.push(g2, 10, data).unwrap_err().code, Code::Contract);
        assert_eq!(q2.telemetry().queued_frames, 0);
    }
    assert_eq!(
        q2.push(g2, 11, &[0.0, 0.0]).unwrap_err().code,
        Code::Contract
    );
    drop(callback);
    assert_eq!(q2.push(g2, 10, &[0.0, 0.0]).unwrap_err().code, Code::Device);
}

#[test]
fn native_channels_and_wrapped_ring_order_are_preserved() {
    let (mut q, mut c, g) = prepared(4);
    let a = [0.1, 0.2, 0.3, 0.4];
    q.push(g, 10, &a.repeat(6)).unwrap();
    q.commit(g).unwrap();
    q.start(g).unwrap();
    let mut out = [0.0; 16];
    c.render(&mut out, 0, None);
    assert_eq!(out.as_slice(), a.repeat(4));
    let b = [-0.1, -0.2, -0.3, -0.4];
    q.push(g, 16, &b.repeat(4)).unwrap();
    q.finish(g).unwrap();
    c.render(&mut out, 0, None);
    assert_eq!(&out[..8], a.repeat(2));
    assert_eq!(&out[8..], b.repeat(2));
    c.render(&mut out, 0, None);
    assert_eq!(&out[..8], b.repeat(2));
    assert_eq!(&out[8..], [0.0; 8]);
    assert_eq!(q.telemetry().status, Status::Drained);
    assert_eq!(q.telemetry().submitted_frames, 10);
}

#[test]
fn underrun_stops_instead_of_inventing_audible_frames_or_restarting() {
    let (mut q, mut c, g) = prepared(2);
    q.push(g, 10, &[0.1; 4]).unwrap();
    q.commit(g).unwrap();
    q.start(g).unwrap();
    let mut out = [0.0; 4];
    c.render(&mut out, 0, None);
    c.render(&mut out, 0, None);
    assert_eq!(out, [0.0; 4]);
    assert_eq!(q.telemetry().status, Status::Failed);
    assert_eq!(q.telemetry().submitted_frames, 2);
    assert_eq!(q.push(g, 12, &[0.1; 4]).unwrap_err().code, Code::Underrun);
    assert_eq!(q.start(g).unwrap_err().code, Code::Underrun);
    let next = q.reset(12, Status::Preparing).unwrap();
    c.render(&mut out, 0, None);
    q.push(next, 12, &[0.2; 4]).unwrap();
    q.commit(next).unwrap();
    q.start(next).unwrap();
    c.render(&mut out, 0, None);
    assert_eq!(out, [0.2; 4]);
}

#[test]
fn short_explicit_end_pads_driver_buffer_but_counts_only_media_samples() {
    let (mut q, mut c, g) = prepared(2);
    q.push(g, 10, &[0.5, -0.5]).unwrap();
    assert_eq!(q.commit(g).unwrap_err().code, Code::NotReady);
    q.finish(g).unwrap();
    q.commit(g).unwrap();
    q.start(g).unwrap();
    let mut out = [1.0; 8];
    c.render(&mut out, 0, None);
    assert_eq!(out, [0.5, -0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(q.telemetry().status, Status::Drained);
    assert_eq!(q.telemetry().submitted_frames, 1);
    c.render(&mut out, 0, None);
    assert_eq!(out, [0.0; 8]);
    assert_eq!(q.start(g).unwrap_err().code, Code::NotReady);
}

#[test]
fn ready_requires_at_least_the_observed_device_callback_size() {
    let (mut q, mut c, g) = prepared(1);
    c.render(&mut [0.0; 4], 0, None);
    q.push(g, 10, &[0.1; 2]).unwrap();
    assert_eq!(q.commit(g).unwrap_err().code, Code::NotReady);
    q.push(g, 12, &[0.1; 2]).unwrap();
    q.commit(g).unwrap();
}

#[test]
fn device_failure_cannot_be_silently_reopened_or_marked_ready() {
    let (mut q, mut c, g) = prepared(1);
    q.push(g, 10, &[0.1; 2]).unwrap();
    q.commit(g).unwrap();
    q.shared.device_failed.store(true, Ordering::Release);
    assert_eq!(q.start(g).unwrap_err().code, Code::Device);
    c.render(&mut [0.0; 2], 0, None);
    assert_eq!(q.telemetry().status, Status::Failed);
}

#[test]
fn bad_configuration_and_memory_requests_are_rejected_before_worker_spawn() {
    for (rate, channels, capacity, ready) in [
        (0, 2, 10, 2),
        (48000, 0, 10, 2),
        (48000, 65, 10, 2),
        (48000, 2, 10, 0),
        (48000, 2, 1, 2),
        (48000, 2, 96001, 2),
        (384000, 64, 768000, 2),
    ] {
        let mut c = config(channels, capacity, ready);
        c.format.sample_rate_hz = rate;
        assert_eq!(c.validate().unwrap_err().code, Code::Contract);
    }
    let mut c = config(2, 10, 2);
    c.version = "future".into();
    assert!(c.validate().is_err());
    let mut v = serde_json::to_value(config(2, 10, 2)).unwrap();
    v["media_path"] = "raw".into();
    assert!(serde_json::from_value::<Config>(v).is_err());
}

#[test]
fn independent_instances_do_not_share_gate_or_queue() {
    let (mut a, mut ca, ga) = prepared(1);
    let (mut b, mut cb, gb) = prepared(1);
    a.push(ga, 10, &[0.1; 2]).unwrap();
    a.commit(ga).unwrap();
    b.push(gb, 10, &[0.9; 2]).unwrap();
    b.commit(gb).unwrap();
    a.start(ga).unwrap();
    let mut out = [0.0; 2];
    cb.render(&mut out, 0, None);
    assert_eq!(out, [0.0; 2]);
    ca.render(&mut out, 0, None);
    assert_eq!(out, [0.1; 2]);
    a.reset(0, Status::Paused).unwrap();
    b.start(gb).unwrap();
    cb.render(&mut out, 0, None);
    assert_eq!(out, [0.9; 2]);
}

#[test]
#[ignore = "explicit run: opens the real default device and plays a low-level test signal"]
fn real_device_preroll_start_pause_resume_and_drain() {
    let config = config(2, 24_000, 4_800);
    let mut output = AudioOutput::open(config).unwrap();
    println!("device={:?}", output.device());
    let samples: Vec<f32> = (0..9_600)
        .flat_map(|i| {
            let value = (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.015;
            [value, value]
        })
        .collect();
    for anchor in [0, 9600] {
        let g = output.begin(anchor).unwrap();
        output.queue(g, anchor, &samples).unwrap();
        output.finish(g).unwrap();
        output.commit(g).unwrap();
        thread::sleep(Duration::from_millis(40));
        assert_eq!(output.telemetry().submitted_frames, 0);
        let start = Instant::now();
        output.start(g).unwrap();
        let api_ns = start.elapsed().as_nanos();
        let deadline = Instant::now() + ACK_TIMEOUT;
        loop {
            let t = output.telemetry();
            assert_ne!(t.status, Status::Failed, "{t:?}");
            if t.submitted_frames > 0 {
                println!("start_api_ns={api_ns}, telemetry={t:?}");
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        if anchor == 0 {
            output.pause().unwrap();
            assert_eq!(output.telemetry().status, Status::Paused);
            assert_eq!(output.telemetry().submitted_frames, 0);
        } else {
            while output.telemetry().status != Status::Drained {
                assert!(Instant::now() < deadline);
                assert_ne!(output.telemetry().status, Status::Failed);
                thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(output.telemetry().submitted_frames, 9600);
        }
    }
}
