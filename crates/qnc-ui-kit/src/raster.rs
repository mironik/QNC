use eframe::egui::{self, Color32, PaintCallbackInfo, Rect, TextureHandle, TextureOptions, Ui, Vec2};
use std::collections::HashMap;
use std::sync::Arc;

fn stream_frame_valid(size: [usize; 2], rgba: &[u8]) -> bool {
    !size.contains(&0)
        && size[0].checked_mul(size[1]).and_then(|n| n.checked_mul(4)) == Some(rgba.len())
        && rgba.len() <= 32 * 1024 * 1024
}

/// Same blit as the first correct picture, in window NDC. egui shrinks the
/// viewport to the callback rect; paint restores the full framebuffer so the
/// quad is monitor-sized, not a cropped enlarge.
pub fn paint_stream_frame(
    ui: &mut Ui,
    rect: Rect,
    surface: egui::Id,
    key: (&str, u64, u64),
    size: [usize; 2],
    rgba: &[u8],
) -> bool {
    if !ui.is_rect_visible(rect) || !stream_frame_valid(size, rgba) {
        return false;
    }
    let aspect = size[0] as f32 / size[1] as f32;
    let width = rect.width().min(rect.height() * aspect);
    let fitted = Rect::from_center_size(rect.center(), Vec2::new(width, width / aspect));
    let diagnostic_key = (key.0.to_string(), key.1, key.2);
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        fitted,
        StreamFrameCallback {
            surface: surface.value(),
            size: [size[0] as u32, size[1] as u32],
            rgba: Arc::<[u8]>::from(rgba),
            fitted,
        },
    ));
    report_stream_frame(&diagnostic_key);
    true
}

struct StreamFrameCallback {
    surface: u64,
    size: [u32; 2],
    rgba: Arc<[u8]>,
    fitted: Rect,
}

impl egui_wgpu::CallbackTrait for StreamFrameCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if resources.get::<MonitorBlit>().is_none() {
            resources.insert(MonitorBlit::new(device));
        }
        if let Some(blit) = resources.get_mut::<MonitorBlit>() {
            blit.upload(device, queue, screen_descriptor, self);
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let [sw, sh] = info.screen_size_px;
        if sw == 0 || sh == 0 {
            return;
        }
        render_pass.set_viewport(0.0, 0.0, sw as f32, sh as f32, 0.0, 1.0);
        if let Some(blit) = resources.get::<MonitorBlit>() {
            blit.paint(self.surface, render_pass);
        }
    }
}

struct MonitorSlot {
    size: [u32; 2],
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}

struct MonitorBlit {
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    slots: HashMap<u64, MonitorSlot>,
}

impl MonitorBlit {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("qnc-monitor-preview"),
            source: wgpu::ShaderSource::Wgsl(MONITOR_BLIT_WGSL.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qnc-monitor-preview-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("qnc-monitor-preview-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("qnc-monitor-preview-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        // Same format egui prefers on the window surface.
        let format = wgpu::TextureFormat::Bgra8Unorm;
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qnc-monitor-preview-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            sampler,
            bind_group_layout,
            pipeline,
            slots: HashMap::new(),
        }
    }

    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        frame: &StreamFrameCallback,
    ) {
        let [width, height] = frame.size;
        let recreate = self
            .slots
            .get(&frame.surface)
            .is_none_or(|slot| slot.size != frame.size);
        if recreate {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("qnc-monitor-preview-frame"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("qnc-monitor-preview-rect"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("qnc-monitor-preview-bg"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            self.slots.insert(
                frame.surface,
                MonitorSlot {
                    size: frame.size,
                    texture,
                    bind_group,
                    uniform,
                },
            );
        }
        let slot = self.slots.get(&frame.surface).unwrap();
        let bytes_per_row = padded_bytes_per_row(width);
        write_rgba_texture(queue, &slot.texture, width, height, bytes_per_row, &frame.rgba);
        let ppp = screen.pixels_per_point;
        let sw = screen.size_in_pixels[0] as f32;
        let sh = screen.size_in_pixels[1] as f32;
        if sw <= 0.0 || sh <= 0.0 {
            return;
        }
        let px = frame.fitted.min.x * ppp;
        let py = frame.fitted.min.y * ppp;
        let pw = frame.fitted.width() * ppp;
        let ph = frame.fitted.height() * ppp;
        let ndc = [
            px / sw * 2.0 - 1.0,
            1.0 - py / sh * 2.0,
            (px + pw) / sw * 2.0 - 1.0,
            1.0 - (py + ph) / sh * 2.0,
        ];
        queue.write_buffer(&slot.uniform, 0, ndc_bytes(&ndc));
    }

    fn paint(&self, surface: u64, render_pass: &mut wgpu::RenderPass<'static>) {
        let Some(slot) = self.slots.get(&surface) else {
            return;
        };
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &slot.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

fn write_rgba_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    bytes_per_row: u32,
    rgba: &[u8],
) {
    if bytes_per_row == width * 4 {
        queue.write_texture(
            texture.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        return;
    }
    let mut padded = vec![0u8; (bytes_per_row * height) as usize];
    let src_stride = (width * 4) as usize;
    let dst_stride = bytes_per_row as usize;
    for y in 0..height as usize {
        let src = y * src_stride;
        let dst = y * dst_stride;
        padded[dst..dst + src_stride].copy_from_slice(&rgba[src..src + src_stride]);
    }
    queue.write_texture(
        texture.as_image_copy(),
        &padded,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn padded_bytes_per_row(width: u32) -> u32 {
    let unpadded = width.saturating_mul(4);
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    unpadded.div_ceil(align).saturating_mul(align)
}

fn ndc_bytes(ndc: &[f32; 4]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(ndc.as_ptr() as *const u8, 16) }
}

const MONITOR_BLIT_WGSL: &str = r#"
struct RectNdc {
    min: vec2<f32>,
    max: vec2<f32>,
};

@group(0) @binding(0) var frame_tex: texture_2d<f32>;
@group(0) @binding(1) var frame_samp: sampler;
@group(0) @binding(2) var<uniform> rect: RectNdc;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let uv = uvs[i];
    let pos = mix(rect.min, rect.max, uv);
    var out: VsOut;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(frame_tex, frame_samp, in.uv, 0.0);
}
"#;

fn report_stream_frame(diagnostic_key: &(String, u64, u64)) {
    if !qnc_dev_diagnostics::player_diagnostics_enabled() {
        return;
    }
    let unix_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    static LAST_REPORT: std::sync::OnceLock<
        std::sync::Mutex<Option<((String, u64, u64), u64)>>,
    > = std::sync::OnceLock::new();
    let last_report = LAST_REPORT.get_or_init(|| std::sync::Mutex::new(None));
    let unix_ns_u64 = unix_ns.min(u128::from(u64::MAX)) as u64;
    let should_report = {
        let mut last = last_report.lock().unwrap();
        let key = diagnostic_key.clone();
        let report = last.as_ref().is_none_or(|(previous_key, previous_ns)| {
            previous_key != &key && unix_ns_u64.saturating_sub(*previous_ns) >= 250_000_000
        });
        if report {
            *last = Some((key, unix_ns_u64));
        }
        report
    };
    if should_report {
        qnc_dev_diagnostics::log_line(
            qnc_dev_diagnostics::DiagnosticsStream::Player,
            format!(
                "AV_V session={} generation={} sequence={} unix_ns={}",
                diagnostic_key.0, diagnostic_key.1, diagnostic_key.2, unix_ns
            ),
        );
    }
}

#[derive(Clone)]
struct Texture {
    handle: TextureHandle,
    used: u64,
}
#[derive(Clone, Default)]
struct RasterCache {
    clock: u64,
    textures: HashMap<(String, u64), Texture>,
}

/// Passive upload/paint of already decoded pixels. No URI fetching or decoding.
pub fn paint_rgba_image(
    ui: &mut Ui,
    rect: Rect,
    uri: &str,
    content_key: u64,
    size: [usize; 2],
    rgba: &[u8],
) -> bool {
    if !ui.is_rect_visible(rect) {
        return false;
    }
    if size.contains(&0)
        || size[0].checked_mul(size[1]).and_then(|n| n.checked_mul(4)) != Some(rgba.len())
        || rgba.len() > 4 * 1024 * 1024
    {
        return false;
    }
    let key = (uri.to_string(), content_key);
    let cached = ui.data_mut(|data| {
        let cache =
            data.get_temp_mut_or_default::<RasterCache>(egui::Id::new("qnc-ui-kit-raster-cache"));
        cache.clock = cache.clock.wrapping_add(1);
        cache.textures.get_mut(&key).map(|entry| {
            entry.used = cache.clock;
            entry.handle.clone()
        })
    });
    // Upload outside the egui memory lock: load_texture also accesses the context.
    let texture = cached.unwrap_or_else(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba);
        let handle = ui.ctx().load_texture(
            format!("{uri}:{content_key}"),
            image,
            TextureOptions::LINEAR,
        );
        ui.data_mut(|data| {
            let cache = data
                .get_temp_mut_or_default::<RasterCache>(egui::Id::new("qnc-ui-kit-raster-cache"));
            if cache.textures.len() >= 128 {
                if let Some(oldest) = cache
                    .textures
                    .iter()
                    .min_by_key(|(_, t)| t.used)
                    .map(|(k, _)| k.clone())
                {
                    cache.textures.remove(&oldest);
                }
            }
            cache.textures.insert(
                key,
                Texture {
                    handle: handle.clone(),
                    used: cache.clock,
                },
            );
        });
        handle
    });
    let aspect = size[0] as f32 / size[1] as f32;
    let width = rect.width().min(rect.height() * aspect);
    let fitted = Rect::from_center_size(rect.center(), Vec2::new(width, width / aspect));
    ui.painter().image(
        texture.id(),
        fitted,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_replaces_one_texture_without_accumulating_playback_frames() {
        let ctx = egui::Context::default();
        for sequence in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let id = egui::Id::new("test-stream");
                    let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(160.0, 90.0));
                    assert!(paint_stream_frame(
                        ui,
                        rect,
                        id,
                        ("session", 1, sequence),
                        [2, 1],
                        &[255; 8]
                    ));
                    assert!(!paint_stream_frame(
                        ui,
                        rect,
                        id,
                        ("session", 1, sequence),
                        [2, 1],
                        &[255; 7]
                    ));
                });
            });
        }
    }
    #[test]
    fn paints_supplied_pixels_without_resource_fetching() {
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(160.0, 90.0));
                assert!(paint_rgba_image(
                    ui,
                    rect,
                    "qnc://local/artifact/test",
                    1,
                    [2, 2],
                    &[255; 16]
                ));
                assert!(!paint_rgba_image(
                    ui,
                    rect,
                    "qnc://local/artifact/invalid",
                    1,
                    [2, 2],
                    &[255; 4]
                ));
            });
        });
        assert!(output
            .textures_delta
            .set
            .iter()
            .any(|(_, delta)| delta.image.size() == [2, 2]));
    }
}
