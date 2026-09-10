use crate::{
    queue::{Callback, Shared},
    *,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}
fn info(device: &cpal::Device) -> Result<DeviceInfo> {
    Ok(DeviceInfo {
        id: device.id().map_err(failure)?.to_string(),
        name: device.description().map_err(failure)?.name().to_string(),
    })
}
pub fn list() -> Result<Vec<DeviceInfo>> {
    cpal::default_host()
        .output_devices()
        .map_err(failure)?
        .map(|device| info(&device))
        .collect()
}
pub(crate) fn open(
    config: &Config,
    mut callback: Callback,
    shared: Arc<Shared>,
) -> Result<(cpal::Stream, DeviceInfo)> {
    let host = cpal::default_host();
    let device = if let Some(id) = &config.device_id {
        let mut matches = host
            .output_devices()
            .map_err(failure)?
            .filter(|d| d.id().is_ok_and(|value| value.to_string() == *id));
        let device = matches
            .next()
            .ok_or_else(|| Error::new(Code::Device, "configured output device not found"))?;
        if matches.next().is_some() {
            return Err(Error::new(Code::Device, "ambiguous device identity"));
        }
        device
    } else {
        host.default_output_device()
            .ok_or_else(|| Error::new(Code::Device, "no default audio output device"))?
    };
    let ranges: Vec<_> = device
        .supported_output_configs()
        .map_err(failure)?
        .collect();
    let supported = ranges.iter()
        .find(|range| supports(range, config.format))
        .ok_or_else(|| {
            let mut counts: Vec<_> = ranges.iter()
                .filter(|r| r.sample_format() == cpal::SampleFormat::F32
                    && r.min_sample_rate() <= config.format.sample_rate_hz
                    && config.format.sample_rate_hz <= r.max_sample_rate())
                .map(|r| r.channels()).collect();
            counts.sort_unstable();
            counts.dedup();
            Error::new(
                Code::Unsupported,
                format!(
                    "Audio output '{}' cannot open {} discrete channels at {} Hz (f32); supported channel counts at this rate: {:?}. No stereo fallback or downmix.",
                    info(&device).map(|d| d.name).unwrap_or_else(|_| "selected device".into()),
                    config.format.channels, config.format.sample_rate_hz, counts,
                ),
            )
        })?;
    let stream_config = supported
        .clone()
        .with_sample_rate(config.format.sample_rate_hz)
        .config();
    let errors = shared.clone();
    let stream = device
        .build_output_stream(
            &stream_config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let stamp = info.timestamp();
                let delay = stamp
                    .playback
                    .duration_since(&stamp.callback)
                    .map(|d| d.as_nanos().min((queue::NONE - 1) as u128) as u64);
                callback.render(data, shared.elapsed_ns(), delay);
            },
            move |_| {
                errors.device_failed.store(true, Ordering::Release);
            },
            Some(ACK_TIMEOUT),
        )
        .map_err(failure)?;
    // Backend callbacks are warmed with silence; Start later changes only the atomic gate.
    stream.play().map_err(failure)?;
    Ok((stream, info(&device)?))
}
fn supports(range: &cpal::SupportedStreamConfigRange, format: Format) -> bool {
    range.channels() == format.channels
        && range.sample_format() == cpal::SampleFormat::F32
        && range.min_sample_rate() <= format.sample_rate_hz
        && format.sample_rate_hz <= range.max_sample_rate()
}
fn failure(error: impl std::fmt::Display) -> Error {
    Error::new(Code::Device, error.to_string())
}
