#![cfg(test)]
//! Explicit adapter test of saved ORIGINAL audio, not an override of project playback.input.
use super::*;
use qnc_media_stream::{CodecEndpoint, LocalSource, SourceReference};
use qnc_player_input::InputReader;
use qnc_work_settings::SettingsReader;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Seek},
    path::PathBuf,
    thread,
    time::Duration,
};

fn hash(stream: &mut MediaStream) -> String {
    stream.rewind().unwrap();
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let size = stream.read(&mut buffer).unwrap();
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    format!("{:x}", hash.finalize())
}

#[test]
#[ignore = "explicit card/device test: requires QNC_LIVE_ROOT, QNC_LIVE_SOURCE, QNC_LIVE_CLIP; plays original mono channels 3/4"]
fn real_saved_original_mono_tracks() {
    let root = PathBuf::from(std::env::var_os("QNC_LIVE_ROOT").expect("explicit root"));
    let source_root = PathBuf::from(std::env::var_os("QNC_LIVE_SOURCE").expect("explicit source"));
    let clip = std::env::var("QNC_LIVE_CLIP").expect("explicit clip");
    let reader = SettingsReader::from_root(&root).unwrap();
    let settings = reader.read().unwrap();
    let saved = InputReader::new(reader.clone())
        .load(&settings.workspace_db_uri, &clip)
        .unwrap();
    assert!(
        !saved
            .snapshot
            .report
            .issues
            .iter()
            .any(|i| i.path == "original" || i.path.starts_with("original."))
    );
    let media = saved.snapshot.metadata.original;
    let (audio_streams, format) = input::native_audio_layout(&media).unwrap();
    assert_eq!(
        audio_streams.len(),
        4,
        "this explicit fixture requires four mono streams"
    );
    assert!(audio_streams.iter().all(|(_, count)| *count == 1));
    let format = format.unwrap();
    let audio_stream_plans: Vec<_> = audio_streams
        .iter()
        .map(|(stream_index, source_channels)| input::AudioStreamPlan {
            stream_index: *stream_index,
            source_channels: *source_channels,
            selected_channels: (0..*source_channels).collect(),
        })
        .collect();
    let stream = media
        .streams
        .iter()
        .find(|s| matches!(s.details, qnc_media_metadata::StreamDetails::Video(_)))
        .unwrap();
    let qnc_media_metadata::StreamDetails::Video(video) = &stream.details else {
        unreachable!()
    };
    let fps = video.frame_rate.as_ref().unwrap().value;
    let timebase = Timebase::new(fps.fps_num, fps.fps_den).unwrap();
    let frames = video.exact_frame_count().unwrap();
    assert!(frames >= input::PREBUFFER_FRAMES as u64);
    let plan = InputPlan {
        source: SourceRuntime::new(&clip, frames, timebase)
            .unwrap()
            .with_audio_format(format.clone()),
        video_index: stream.index.as_ref().unwrap().value,
        origin: (
            stream.start_pts.as_ref().unwrap().value,
            stream.time_base.as_ref().unwrap().value,
        ),
        spec: qnc_pixel_convert::ConversionSpec::from_saved(video).unwrap(),
        audio_streams: audio_stream_plans,
        audio_channels: Some(ChannelMap::new(format.channel_count, vec![2, 3]).unwrap()),
        audio_origin: (
            stream.start_pts.as_ref().unwrap().value,
            stream.time_base.as_ref().unwrap().value,
        ),
        audio_media: media.clone(),
        media,
    };
    let reference = SourceReference::from_uri(&plan.media.media_uri).unwrap();
    let codec_path = source_root
        .join(reference.relative_path())
        .canonicalize()
        .unwrap();
    let source = LocalSource::new(reference.source_uri(), &source_root).unwrap();
    let mut verify = MediaStream::local(&source, &plan.media.media_uri).unwrap();
    let before = hash(&mut verify);
    let storage_stamp = verify.info().storage_stamp.clone();
    let media_uri = plan.media.media_uri.clone();
    let decode_input = Rc::new(DecodeInput::new_access(
        plan.media.clone(),
        qnc_decoder_catalog::installed_config().unwrap(),
        move |uri| {
            if uri != media_uri {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "media source mismatch",
                ));
            }
            Ok(DecodeMediaAccess::Endpoint {
                endpoint: CodecEndpoint::for_local_file(&codec_path, uri)?,
                storage_stamp: storage_stamp.clone(),
            })
        },
    ));
    let map = ChannelMap::new(format.channel_count, vec![2, 3]).unwrap();
    let mut audio = Audio::open(
        &plan,
        decode_input,
        None,
        Some(map),
        input::PREBUFFER_FRAMES,
    )
    .unwrap();
    let limit = sample_boundary(
        input::PREBUFFER_FRAMES as u64,
        timebase,
        format.sample_rate_hz,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut frame = 0;
    let mut playing = false;
    let mut counts = [0u64; 4];
    let mut peaks = [0f32; 4];
    let mut previous_tick = Instant::now();
    let mut max_gap_us = 0;
    let mut max_work_us = 0;
    let mut pending = 0;
    let mut seek_reference = BTreeMap::new();
    audio.begin_audio_preroll().unwrap();
    loop {
        let before_frame = frame;
        let tick = Instant::now();
        max_gap_us = max_gap_us.max(tick.duration_since(previous_tick).as_micros());
        previous_tick = tick;
        assert!(Instant::now() < deadline, "audio diagnostic timeout");
        let telemetry = audio.device.as_ref().unwrap().borrow().telemetry();
        assert_ne!(
            telemetry.status,
            qnc_audio_output::Status::Failed,
            "{telemetry:?}; frame {frame}, max gap {max_gap_us} us, work {max_work_us} us, pending {pending}, tracks {:?}",
            audio
                .tracks
                .iter()
                .map(|t| (t.stream_index, t.decoded_through, t.samples.len()))
                .collect::<Vec<_>>()
        );
        if frame == frames && telemetry.status == qnc_audio_output::Status::Drained {
            break;
        }
        if frame < frames && telemetry.queued_frames < limit {
            let request = EngineFrameRequest::new(
                &EngineSourceHandle::from_source_runtime(&plan.source, None),
                frame,
            )
            .unwrap();
            match audio.render_audio_for_frame(request) {
                Ok(packet) => {
                    assert_eq!(packet.audio_format.as_ref().unwrap().channel_count, 4);
                    if [0, frames / 2, frames - 1].contains(&frame) {
                        seek_reference.insert(frame, packet.payload.clone());
                    }
                    for samples in packet.payload.chunks_exact(4) {
                        for channel in 0..4 {
                            assert!(samples[channel].is_finite());
                            counts[channel] += 1;
                            peaks[channel] = peaks[channel].max(samples[channel].abs());
                        }
                    }
                    audio.submit_audio_packet(packet).unwrap();
                    frame += 1;
                }
                Err(e) if e.kind == BroadcastEngineErrorKind::NotReady => pending += 1,
                Err(e) => panic!("{e}"),
            }
        }
        if !playing && frame >= input::PREBUFFER_FRAMES as u64 {
            audio.commit_audio_preroll().unwrap();
            let start = Instant::now();
            audio.start_audio().unwrap();
            println!(
                "Original audio start {} us, monitor native channels 3/4",
                start.elapsed().as_micros()
            );
            playing = true;
        }
        max_work_us = max_work_us.max(tick.elapsed().as_micros());
        // Catch up immediately after a scheduler delay; only wait when buffered or pending.
        if audio
            .device
            .as_ref()
            .unwrap()
            .borrow()
            .telemetry()
            .queued_frames
            >= limit
            || frame == before_frame
        {
            thread::sleep(Duration::from_millis(1));
        } else {
            thread::yield_now();
        }
    }
    let expected = sample_boundary(frames, timebase, format.sample_rate_hz).unwrap();
    assert_eq!(counts, [expected; 4]);
    println!(
        "All four native mono streams: sample frames {counts:?}, peaks {peaks:?}; device {:?}",
        audio.device.as_ref().unwrap().borrow().telemetry()
    );
    audio.pause_audio().unwrap();
    for target in [frames / 2, frames - 1, 0] {
        audio
            .cue_audio(
                EngineFrameRequest::new(
                    &EngineSourceHandle::from_source_runtime(&plan.source, None),
                    target,
                )
                .unwrap(),
            )
            .unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(15), "seek timeout");
            let request = EngineFrameRequest::new(
                &EngineSourceHandle::from_source_runtime(&plan.source, None),
                target,
            )
            .unwrap();
            match audio.render_audio_for_frame(request) {
                Ok(packet) => {
                    assert_eq!(packet.payload, seek_reference[&target]);
                    println!(
                        "Original mono seek {target}: {} ms, all four channels exactly match sequential PCM",
                        started.elapsed().as_millis()
                    );
                    break;
                }
                Err(e) if e.kind == BroadcastEngineErrorKind::NotReady => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("{e}"),
            }
        }
    }
    drop(audio);
    assert_eq!(hash(&mut verify), before);
    assert_eq!(reader.read().unwrap(), settings);
}
