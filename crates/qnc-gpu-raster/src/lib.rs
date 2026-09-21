//! GPU raster preparation only. No media I/O, decoder, clock, UI or database.
use qnc_pixel_convert::{ConversionError, ConversionSpec, Range, Transfer, fit_raster_size as fit};
pub const VERSION: &str = "0.1.0";
const MAX_BYTES: usize = 512 * 1024 * 1024;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt;

/// Prepared SDR raster adapter. Owns no media source, clock or display state.
/// GPU work stays on the GPU until an explicit collect; the consumer still
/// accepts packed RGBA until a shared surface adapter exists.
pub struct GpuRasterConverter {
    spec: ConversionSpec,
    size: [u32; 2],
    device: wgpu::Device,
    queue: wgpu::Queue,
    source: wgpu::Buffer,
    target: wgpu::Buffer,
    readback: [wgpu::Buffer; 2],
    next_readback: usize,
    bindings: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    adapter_name: String,
    failed: Arc<AtomicBool>,
    timing: [u128; 3],
}

/// In-flight GPU raster. Collect copies out; enqueue of the next frame can overlap.
pub struct GpuReadback {
    index: usize,
    mapped: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    upload_us: u128,
    wait_start: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_pixel_convert::{Converter, PixelLayout};
    fn spec(layout: PixelLayout) -> ConversionSpec {
        ConversionSpec {
            version: qnc_pixel_convert::VERSION.into(),
            width: 4,
            height: 2,
            layout,
            primaries: "bt709".into(),
            matrix: "bt709".into(),
            scan_mode: qnc_media_metadata::ScanMode::Progressive,
            range: Range::Limited,
            transfer: Transfer::Bt709,
        }
    }
    fn gray(spec: &ConversionSpec, luma: u16) -> Vec<u8> {
        let depth = if spec.layout.ten_bit() { 2 } else { 1 };
        let y = spec.width as usize * spec.height as usize;
        (0..spec.input_bytes().unwrap() / depth)
            .flat_map(|i| {
                let value = if i < y {
                    luma
                } else if depth == 2 {
                    512
                } else {
                    128
                };
                value.to_le_bytes()[..depth].to_vec()
            })
            .collect()
    }

    #[test]
    fn invalid_saved_description_is_rejected_before_gpu_open() {
        let mut s = spec(PixelLayout::Yuv420p);
        s.matrix = "unknown".into();
        assert!(matches!(
            GpuRasterConverter::prepare(s, [2, 2]),
            Err(ConversionError::Unsupported("color.matrix"))
        ));
        assert!(matches!(
            GpuRasterConverter::prepare(spec(PixelLayout::Yuv420p), [0, 2]),
            Err(ConversionError::Dimensions)
        ));
    }

    #[test]
    #[ignore = "requires a real GPU; explicitly run during pixel/output verification"]
    fn gpu_saved_layout_color_and_reused_buffers() {
        for layout in [
            PixelLayout::Yuv420p,
            PixelLayout::Yuv422p,
            PixelLayout::Yuv444p,
            PixelLayout::Yuv420p10le,
            PixelLayout::Yuv422p10le,
            PixelLayout::Yuv444p10le,
        ] {
            for range in [Range::Limited, Range::Full] {
                for transfer in [Transfer::Bt709, Transfer::Srgb] {
                    let mut s = spec(layout);
                    // Odd sizes also exercise unaligned packed input and ceil chroma planes.
                    s.width = 7;
                    s.height = 5;
                    s.range = range;
                    s.transfer = transfer;
                    let mut gpu = GpuRasterConverter::prepare(s.clone(), [7, 5]).unwrap();
                    let mut cpu =
                        Converter::prepare(s.clone(), s.scratch_bytes().unwrap()).unwrap();
                    let mut actual = vec![0; gpu.output_bytes()];
                    let mut expected = actual.clone();
                    for level in [0, 16, 64, 128, 235, 255] {
                        let input = gray(&s, level * if layout.ten_bit() { 4 } else { 1 });
                        gpu.convert(&input, &mut actual).unwrap();
                        cpu.convert(&input, &mut expected).unwrap();
                        assert!(
                            actual
                                .iter()
                                .zip(&expected)
                                .all(|(a, b)| a.abs_diff(*b) <= 2),
                            "{layout:?} {range:?} {transfer:?} {level}: {:?} {:?}",
                            &actual[..4],
                            &expected[..4]
                        );
                    }
                    assert!(gpu.convert(&[0], &mut actual).is_err());
                    if layout.ten_bit() {
                        let mut invalid = gray(&s, 512);
                        invalid[..2].copy_from_slice(&1024u16.to_le_bytes());
                        assert_eq!(
                            gpu.convert(&invalid, &mut actual),
                            Err(ConversionError::BitDepth)
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires a real GPU; explicitly run during pixel/output verification"]
    fn gpu_scales_color_bars_without_flipping_or_changing_range() {
        let mut s = spec(PixelLayout::Yuv444p);
        s.width = 64;
        s.height = 16;
        s.transfer = Transfer::Srgb;
        let bars = [
            [16u8, 128, 128],
            [235, 128, 128],
            [81, 90, 240],
            [145, 54, 34],
        ];
        let input: Vec<u8> = (0..3)
            .flat_map(|p| {
                (0..16).flat_map(move |y| (0..64).map(move |x| bars[(x / 16 + y / 8) % 4][p]))
            })
            .collect();
        let mut gpu = GpuRasterConverter::prepare(s.clone(), [32, 8]).unwrap();
        let mut output = vec![0; gpu.output_bytes()];
        gpu.convert(&input, &mut output).unwrap();
        assert_eq!(gpu.size(), [32, 8]);
        let mut cpu = Converter::prepare(s.clone(), s.scratch_bytes().unwrap()).unwrap();
        let mut full = vec![0; s.output_bytes().unwrap()];
        cpu.convert(&input, &mut full).unwrap();
        for y in [1, 6] {
            for bar in 0..4 {
                let a = (y * 32 + bar * 8 + 4) * 4;
                let b = ((2 * y) * 64 + bar * 16 + 8) * 4;
                assert!(
                    output[a..a + 4]
                        .iter()
                        .zip(&full[b..b + 4])
                        .all(|(a, b)| a.abs_diff(*b) <= 2)
                );
            }
        }
        assert_eq!(output.len(), 32 * 8 * 4);
        assert!(output.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }
}

fn gpu_error(error: impl std::fmt::Display) -> ConversionError {
    ConversionError::Library(format!("GPU raster: {error}"))
}

impl GpuRasterConverter {
    pub fn prepare(spec: ConversionSpec, bounds: [u32; 2]) -> Result<Self, ConversionError> {
        spec.validate()?;
        let size = fit([spec.width, spec.height], bounds)?;
        let input_bytes = spec.input_bytes()?.next_multiple_of(4) as u64;
        let output_bytes = u64::from(size[0]) * u64::from(size[1]) * 4;
        if input_bytes + output_bytes * 3 > MAX_BYTES as u64 {
            return Err(ConversionError::Budget);
        }
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok_or_else(|| gpu_error("no adapter"))?;
        let limits = wgpu::Limits::default();
        if input_bytes.max(output_bytes) > u64::from(limits.max_storage_buffer_binding_size)
            || size[0].div_ceil(8).max(size[1].div_ceil(8))
                > limits.max_compute_workgroups_per_dimension
        {
            return Err(ConversionError::Budget);
        }
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            return Err(gpu_error("software adapter is not a GPU raster backend"));
        }
        let adapter_name = info.name;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("qnc-pixel-convert"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        ))
        .map_err(gpu_error)?;
        let failed = Arc::new(AtomicBool::new(false));
        let on_error = failed.clone();
        device.on_uncaptured_error(Box::new(move |_| {
            on_error.store(true, Ordering::Release);
        }));
        let on_loss = failed.clone();
        device.set_device_lost_callback(move |_, _| {
            on_loss.store(true, Ordering::Release);
        });
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let source = buffer(
            "saved-planar-input",
            input_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let target = buffer(
            "srgb-output",
            output_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let readback = [
            buffer(
                "srgb-readback-0",
                output_bytes,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            ),
            buffer(
                "srgb-readback-1",
                output_bytes,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            ),
        ];
        let (cw, ch) = spec.layout.chroma_size(spec.width, spec.height);
        let y_len = spec.width * spec.height;
        let params: Vec<u8> = [
            spec.width,
            spec.height,
            cw,
            ch,
            size[0],
            size[1],
            if spec.layout.ten_bit() { 16 } else { 8 },
            u32::from(spec.range == Range::Limited),
            u32::from(spec.transfer == Transfer::Bt709),
            y_len,
            y_len + cw * ch,
            0,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("saved-color-layout"),
            contents: &params,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDR-raster"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SDR-raster"),
            layout: None,
            module: &shader,
            entry_point: Some("convert"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raster-slots"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: target.as_entire_binding(),
                },
            ],
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            return Err(gpu_error(error));
        }
        Ok(Self {
            spec,
            size,
            device,
            queue,
            source,
            target,
            readback,
            next_readback: 0,
            bindings,
            pipeline,
            adapter_name,
            failed,
            timing: [0; 3],
        })
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }
    pub fn output_bytes(&self) -> usize {
        self.size[0] as usize * self.size[1] as usize * 4
    }
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// CPU wall times: upload/enqueue, GPU execution+readback wait, mapped copy.
    /// These are not GPU timestamps or display presentation measurements.
    pub fn timing_us(&self) -> [u128; 3] {
        self.timing
    }

    /// Called only on a preparation worker. All payload storage is reused.
    pub fn convert(&mut self, input: &[u8], rgba: &mut [u8]) -> Result<(), ConversionError> {
        let pending = self.enqueue(input)?;
        self.collect(pending, rgba)
    }

    /// Submit GPU raster. Does not wait for CPU readback.
    pub fn enqueue(&mut self, input: &[u8]) -> Result<GpuReadback, ConversionError> {
        if self.failed.load(Ordering::Acquire) {
            return Err(gpu_error("device failed"));
        }
        self.spec.validate_payload(input)?;
        let upload_start = Instant::now();
        let aligned = input.len() / 4 * 4;
        if aligned != 0 {
            self.queue.write_buffer(&self.source, 0, &input[..aligned]);
        }
        if aligned != input.len() {
            let mut tail = [0; 4];
            tail[..input.len() - aligned].copy_from_slice(&input[aligned..]);
            self.queue.write_buffer(&self.source, aligned as u64, &tail);
        }
        let index = self.next_readback;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("raster"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bindings, &[]);
            pass.dispatch_workgroups(self.size[0].div_ceil(8), self.size[1].div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(
            &self.target,
            0,
            &self.readback[index],
            0,
            self.output_bytes() as u64,
        );
        self.queue.submit(Some(encoder.finish()));
        let (send, receive) = mpsc::sync_channel(1);
        self.readback[index]
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.next_readback ^= 1;
        Ok(GpuReadback {
            index,
            mapped: receive,
            upload_us: upload_start.elapsed().as_micros(),
            wait_start: Instant::now(),
        })
    }

    pub fn collect(
        &mut self,
        pending: GpuReadback,
        rgba: &mut [u8],
    ) -> Result<(), ConversionError> {
        if rgba.len() != self.output_bytes() {
            return Err(ConversionError::Payload);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.device.poll(wgpu::Maintain::Poll);
            if self.failed.load(Ordering::Acquire) {
                self.readback[pending.index].unmap();
                return Err(gpu_error("device failed during conversion"));
            }
            match pending.mapped.try_recv() {
                Ok(result) => {
                    result.map_err(gpu_error)?;
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => return Err(gpu_error("readback closed")),
                Err(mpsc::TryRecvError::Empty) if Instant::now() >= deadline => {
                    self.failed.store(true, Ordering::Release);
                    self.readback[pending.index].unmap();
                    return Err(gpu_error("readback timed out"));
                }
                Err(mpsc::TryRecvError::Empty) => std::thread::yield_now(),
            }
        }
        let wait_us = pending.wait_start.elapsed().as_micros();
        let copy_start = Instant::now();
        {
            let mapped = self.readback[pending.index].slice(..).get_mapped_range();
            rgba.copy_from_slice(&mapped);
        }
        self.readback[pending.index].unmap();
        self.timing = [pending.upload_us, wait_us, copy_start.elapsed().as_micros()];
        Ok(())
    }
}
