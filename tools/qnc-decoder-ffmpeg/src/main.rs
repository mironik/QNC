//! The decoder jobs besides playback as a plug-in program (contract `qnc.decoder-jobs` 1), done
//! by the ffmpeg executable each job names (the one the decoder catalog registers). It is the
//! same ffmpeg work the application did in its own process; replacing this program replaces
//! all the jobs together.

use qnc_decoder_jobs::{AudioStreams, DecodePictures, DecoderJobs, Frames, KeyPictures, OnSamples, PictureMode, Run};
use qnc_ffmpeg_decode as ffmpeg;
use std::{path::Path, sync::atomic::AtomicBool};

struct Ffmpeg;

impl DecoderJobs for Ffmpeg {
    fn frames(&self, job: &Frames, cancel: &AtomicBool) -> Result<(), String> {
        let frames: Vec<ffmpeg::FfmpegFilmstripFrame> = job.seconds.iter().map(|&seek_sec| ffmpeg::FfmpegFilmstripFrame { seek_sec }).collect();
        let timebase = qnc_frame_timebase::FrameTimebase::new(i64::from(job.frame_rate.0), i64::from(job.frame_rate.1))?;
        let mode = match job.mode {
            PictureMode::Random => ffmpeg::FfmpegFilmstripMode::RandomSeek,
            PictureMode::Keyframe => ffmpeg::FfmpegFilmstripMode::KeyframeSeek,
            PictureMode::Exact => ffmpeg::FfmpegFilmstripMode::ExactSeek,
        };
        ffmpeg::extract_filmstrip_frames_with_cancel(&job.decoder, &job.media, &frames, timebase, mode, job.thumb, &job.out_dir, cancel)
    }

    fn run(&self, job: &Run, cancel: &AtomicBool) -> Result<(), String> {
        ffmpeg::extract_frame_run_with_cancel(&job.decoder, &job.media, job.start_sec, job.count, job.thumb, &job.out_dir, cancel)
    }

    fn key_pictures(&self, job: &KeyPictures, cancel: &AtomicBool) -> Result<Vec<f64>, String> {
        ffmpeg::key_pictures_with_cancel(&job.decoder, &job.media, job.start_sec, job.duration_sec, cancel)
    }

    fn audio_streams(&self, job: &AudioStreams, cancel: &AtomicBool, on_samples: &mut OnSamples) -> Result<(), String> {
        ffmpeg::audio_streams_with_cancel(&job.decoder, &job.media, &job.container, &job.streams, job.channels, cancel, on_samples)
    }

    fn audio_bytes(&self, demuxer: &str, bytes: Vec<u8>, channels: u32, decoder: &Path, cancel: &AtomicBool, on_samples: &mut OnSamples) -> Result<(), String> {
        ffmpeg::audio_bytes_with_cancel(decoder, demuxer, bytes, channels, cancel, on_samples)
    }

    fn decode_pictures(&self, job: &DecodePictures, cancel: &AtomicBool) -> Result<(), String> {
        ffmpeg::decode_pictures_with_cancel(&job.decoder, &job.demuxer, &job.pictures, job.thumb, &job.out_dir, cancel)
    }
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-decoder-ffmpeg");
    if let Err(error) = qnc_decoder_jobs::serve(&Ffmpeg, "qnc.decoder.ffmpeg", env!("CARGO_PKG_VERSION")) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
