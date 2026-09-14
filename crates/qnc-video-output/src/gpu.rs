use crate::{
    FrameHeader, GpuCompletion, MAX_POOL_BYTES, OutputConfig, OutputError, Submission,
    SubmissionTarget, VERSION,
    model::{pixel_bytes, viewport},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Session-private capability; cannot be deserialized or transferred as a GPU handle.
#[derive(Clone, Debug)]
pub struct PreparedFrame {
    owner: Arc<()>,
    epoch: u64,
    slot: usize,
    revision: u64,
}

struct Slot {
    texture: wgpu::Texture,
    bundle: wgpu::RenderBundle,
    revision: u64,
    frame: Option<FrameHeader>,
    ready: Arc<AtomicBool>,
}

struct Pending {
    submission: Submission,
    done: Arc<AtomicBool>,
}

enum Target {
    Surface(wgpu::Surface<'static>, wgpu::SurfaceConfiguration),
    Offscreen(wgpu::Texture),
}

pub struct VideoOutput {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    target: Target,
    config: OutputConfig,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    slots: Vec<Slot>,
    owner: Arc<()>,
    epoch: u64,
    last_sequence: Option<u64>,
    next_submission: u64,
    pending: Option<Pending>,
    failed: Arc<AtomicBool>,
}

impl VideoOutput {
    /// Preparation only. The native host supplies a surface, not any playback logic.
    /// With no surface the same GPU draw path targets an offscreen texture.
    pub async fn open(
        adapter: &wgpu::Adapter,
        surface: Option<wgpu::Surface<'static>>,
        config: OutputConfig,
        size: (u32, u32),
    ) -> Result<Self, OutputError> {
        config.validate()?;
        if pixel_bytes(size.0, size.1)? > MAX_POOL_BYTES {
            return Err(OutputError::Budget);
        }
        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        for (w, h) in [(config.width, config.height), size] {
            if w == 0
                || h == 0
                || w > limits.max_texture_dimension_2d
                || h > limits.max_texture_dimension_2d
            {
                return Err(OutputError::Dimensions);
            }
        }
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("QNC video output"),
                    required_features: wgpu::Features::empty(),
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|e| OutputError::Device(e.to_string()))?;
        let failed = Arc::new(AtomicBool::new(false));
        let on_error = failed.clone();
        device.on_uncaptured_error(Box::new(move |_| {
            on_error.store(true, Ordering::Release);
        }));
        let on_loss = failed.clone();
        device.set_device_lost_callback(move |_, _| {
            on_loss.store(true, Ordering::Release);
        });
        let (target, format) = if let Some(surface) = surface {
            let caps = surface.get_capabilities(adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f| {
                    matches!(
                        f,
                        wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Rgba8UnormSrgb
                    )
                })
                .ok_or(OutputError::UnsupportedSurface)?;
            let alpha_mode = caps
                .alpha_modes
                .first()
                .copied()
                .ok_or(OutputError::UnsupportedSurface)?;
            if !caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                return Err(OutputError::UnsupportedSurface);
            }
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.0,
                height: size.1,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 1,
                alpha_mode,
                view_formats: vec![],
            };
            surface.configure(&device, &surface_config);
            (Target::Surface(surface, surface_config), format)
        } else {
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            (
                Target::Offscreen(target_texture(&device, size, format)),
                format,
            )
        };
        device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let slots = create_slots(&device, &config, format);
        // Exercise the prepared pipeline without publishing an unrequested image.
        let warmup = target_texture(&device, (1, 1), format);
        let view = warmup.create_view(&Default::default());
        let commands = draw(&device, &view, &slots[0].bundle, (0.0, 0.0, 1.0, 1.0));
        queue.submit([commands]);
        let validation = device.pop_error_scope().await;
        let memory = device.pop_error_scope().await;
        if let Some(error) = validation.or(memory) {
            return Err(OutputError::Device(error.to_string()));
        }
        let output = Self {
            device,
            queue,
            target,
            config,
            size,
            format,
            slots,
            owner: Arc::new(()),
            epoch: 1,
            last_sequence: None,
            next_submission: 1,
            pending: None,
            failed,
        };
        output.healthy()?;
        Ok(output)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn has_free_slot(&self) -> Result<bool, OutputError> {
        self.healthy()?;
        Ok(self.slots.iter().any(|slot| slot.frame.is_none()))
    }

    /// Upload outside Play. At most `slots` uploads can exist until completion/release.
    pub fn prepare(
        &mut self,
        frame: FrameHeader,
        pixels: &[u8],
    ) -> Result<PreparedFrame, OutputError> {
        frame.validate(&self.config, pixels.len())?;
        let (index, revision, ready) = self.reserve_slot(&frame)?;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.slots[index].texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.config.width * 4),
                rows_per_image: Some(self.config.height),
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([]);
        self.queue
            .on_submitted_work_done(move || ready.store(true, Ordering::Release));
        self.publish_slot(index, frame);
        Ok(PreparedFrame {
            owner: self.owner.clone(),
            epoch: self.epoch,
            slot: index,
            revision,
        })
    }

    /// Fill a reserved slot on this output device without CPU RGBA upload.
    pub fn prepare_external(
        &mut self,
        frame: FrameHeader,
        fill: impl FnOnce(&wgpu::Texture) -> Result<(), String>,
    ) -> Result<PreparedFrame, OutputError> {
        let (index, revision, ready) = self.reserve_slot(&frame)?;
        if let Err(error) = fill(&self.slots[index].texture) {
            self.slots[index].frame = None;
            return Err(OutputError::Device(format!("GPU texture fill: {error}")));
        }
        self.queue.submit([]);
        self.queue
            .on_submitted_work_done(move || ready.store(true, Ordering::Release));
        self.publish_slot(index, frame);
        Ok(PreparedFrame {
            owner: self.owner.clone(),
            epoch: self.epoch,
            slot: index,
            revision,
        })
    }

    /// Nonblocking driver progress. Completion refers to GPU work, not scanout.
    pub fn poll(&mut self) -> Result<Option<GpuCompletion>, OutputError> {
        self.device.poll(wgpu::Maintain::Poll);
        self.healthy()?;
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.done.load(Ordering::Acquire))
        {
            return Ok(self.pending.take().map(|p| GpuCompletion {
                submission: p.submission,
            }));
        }
        Ok(None)
    }

    pub fn is_ready(&self, token: &PreparedFrame) -> Result<bool, OutputError> {
        self.healthy()?;
        Ok(self.slot(token)?.ready.load(Ordering::Acquire))
    }

    /// No upload, device/pipeline creation, decode, clock or wait-for-GPU here.
    /// Surface acquisition/presentation can still wait for the platform compositor.
    pub fn submit(&mut self, token: &PreparedFrame) -> Result<Submission, OutputError> {
        self.healthy()?;
        if self.size.0 == 0 || self.size.1 == 0 {
            return Err(OutputError::Suspended);
        }
        if self.pending.is_some() {
            return Err(OutputError::Busy);
        }
        let slot = self.slot(token)?;
        if !slot.ready.load(Ordering::Acquire) {
            return Err(OutputError::NotReady);
        }
        let frame = slot.frame.as_ref().ok_or(OutputError::StaleToken)?.clone();
        let following_id = self
            .next_submission
            .checked_add(1)
            .ok_or(OutputError::Sequence)?;
        let surface_frame = match &self.target {
            Target::Surface(surface, _) => Some(
                surface
                    .get_current_texture()
                    .map_err(|e| OutputError::Surface(e.to_string()))?,
            ),
            Target::Offscreen(_) => None,
        };
        let view = match (&surface_frame, &self.target) {
            (Some(frame), _) => frame.texture.create_view(&Default::default()),
            (None, Target::Offscreen(texture)) => texture.create_view(&Default::default()),
            _ => return Err(OutputError::UnsupportedSurface),
        };
        let commands = draw(
            &self.device,
            &view,
            &slot.bundle,
            viewport((self.config.width, self.config.height), self.size),
        );
        self.queue.submit([commands]);
        let target = if let Some(frame) = surface_frame {
            frame.present();
            SubmissionTarget::Surface
        } else {
            SubmissionTarget::Offscreen
        };
        self.healthy()?;
        let submission = Submission {
            version: VERSION.to_owned(),
            id: self.next_submission,
            frame,
            target,
        };
        self.next_submission = following_id;
        let done = Arc::new(AtomicBool::new(false));
        let callback = done.clone();
        self.queue
            .on_submitted_work_done(move || callback.store(true, Ordering::Release));
        self.pending = Some(Pending {
            submission: submission.clone(),
            done,
        });
        Ok(submission)
    }

    pub fn release(&mut self, token: &PreparedFrame) -> Result<(), OutputError> {
        self.healthy()?;
        let slot = self.slot(token)?;
        if !slot.ready.load(Ordering::Acquire) || self.pending.is_some() {
            return Err(OutputError::Busy);
        }
        self.slots[token.slot].frame = None;
        Ok(())
    }

    /// Explicit generation change only after draining output. Never auto-repair stale work.
    pub fn reset(&mut self, generation: u64) -> Result<(), OutputError> {
        self.healthy()?;
        if generation <= self.config.generation {
            return Err(OutputError::Identity);
        }
        self.invalidate()?;
        self.config.generation = generation;
        self.last_sequence = None;
        Ok(())
    }

    /// Resize is preparation, not a fallback inside submit. Zero size suspends output.
    pub fn resize(&mut self, size: (u32, u32)) -> Result<(), OutputError> {
        self.healthy()?;
        let max = self.device.limits().max_texture_dimension_2d;
        if size.0 > max || size.1 > max {
            return Err(OutputError::Dimensions);
        }
        if size.0 != 0 && size.1 != 0 && pixel_bytes(size.0, size.1)? > MAX_POOL_BYTES {
            return Err(OutputError::Budget);
        }
        self.invalidate()?;
        self.size = size;
        if size.0 == 0 || size.1 == 0 {
            return Ok(());
        }
        match &mut self.target {
            Target::Surface(surface, config) => {
                config.width = size.0;
                config.height = size.1;
                surface.configure(&self.device, config);
            }
            Target::Offscreen(texture) => {
                *texture = target_texture(&self.device, size, self.format);
            }
        }
        self.healthy()
    }

    fn invalidate(&mut self) -> Result<(), OutputError> {
        if self.pending.is_some()
            || self
                .slots
                .iter()
                .any(|s| s.frame.is_some() && !s.ready.load(Ordering::Acquire))
        {
            return Err(OutputError::Busy);
        }
        self.epoch = self.epoch.checked_add(1).ok_or(OutputError::Sequence)?;
        for slot in &mut self.slots {
            slot.frame = None;
        }
        Ok(())
    }

    fn reserve_slot(
        &mut self,
        frame: &FrameHeader,
    ) -> Result<(usize, u64, Arc<AtomicBool>), OutputError> {
        self.healthy()?;
        if self.size.0 == 0 || self.size.1 == 0 {
            return Err(OutputError::Suspended);
        }
        frame.validate_geometry(&self.config)?;
        if self
            .last_sequence
            .is_some_and(|sequence| frame.sequence <= sequence)
        {
            return Err(OutputError::Sequence);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.frame.is_none())
            .ok_or(OutputError::Full)?;
        let slot = &mut self.slots[index];
        slot.revision = slot.revision.checked_add(1).ok_or(OutputError::Sequence)?;
        slot.ready = Arc::new(AtomicBool::new(false));
        Ok((index, slot.revision, slot.ready.clone()))
    }

    fn publish_slot(&mut self, index: usize, frame: FrameHeader) {
        self.last_sequence = Some(frame.sequence);
        self.slots[index].frame = Some(frame);
    }

    fn slot(&self, token: &PreparedFrame) -> Result<&Slot, OutputError> {
        if !Arc::ptr_eq(&token.owner, &self.owner) || token.epoch != self.epoch {
            return Err(OutputError::StaleToken);
        }
        self.slots
            .get(token.slot)
            .filter(|s| s.revision == token.revision && s.frame.is_some())
            .ok_or(OutputError::StaleToken)
    }

    fn healthy(&self) -> Result<(), OutputError> {
        if self.failed.load(Ordering::Acquire) {
            return Err(OutputError::Device(
                "GPU device failed; reopen required".into(),
            ));
        }
        Ok(())
    }
}

fn target_texture(
    device: &wgpu::Device,
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("QNC output target"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn create_slots(
    device: &wgpu::Device,
    config: &OutputConfig,
    format: wgpu::TextureFormat,
) -> Vec<Slot> {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("QNC pixels"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("QNC prepared blit"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    });
    (0..config.slots)
        .map(|_| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("QNC bounded pixel slot"),
                size: wgpu::Extent3d {
                    width: config.width,
                    height: config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("QNC prepared frame binding"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            let mut encoder =
                device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                    label: Some("QNC prepared frame draw"),
                    color_formats: &[Some(format)],
                    depth_stencil: None,
                    sample_count: 1,
                    multiview: None,
                });
            encoder.set_pipeline(&pipeline);
            encoder.set_bind_group(0, &binding, &[]);
            encoder.draw(0..3, 0..1);
            let bundle = encoder.finish(&wgpu::RenderBundleDescriptor { label: None });
            Slot {
                texture,
                bundle,
                revision: 0,
                frame: None,
                ready: Arc::new(AtomicBool::new(false)),
            }
        })
        .collect()
}

fn draw(
    device: &wgpu::Device,
    view: &wgpu::TextureView,
    bundle: &wgpu::RenderBundle,
    viewport: (f32, f32, f32, f32),
) -> wgpu::CommandBuffer {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("QNC output submission"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("QNC output"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_viewport(viewport.0, viewport.1, viewport.2, viewport.3, 0.0, 1.0);
        pass.execute_bundles([bundle]);
    }
    encoder.finish()
}

#[cfg(test)]
impl VideoOutput {
    pub(crate) fn offscreen_texture(&self) -> &wgpu::Texture {
        match &self.target {
            Target::Offscreen(texture) => texture,
            _ => panic!("test target is not offscreen"),
        }
    }
}
