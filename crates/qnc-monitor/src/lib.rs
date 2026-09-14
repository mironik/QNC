//! Passive public monitor / preview surface.
//!
//! Any application embeds this component with a caller-owned surface id and
//! already-confirmed frame payload. It does not own production playback time,
//! decode media, read a database, or know which application hosts it.

use eframe::egui::{
    self, Align2, Color32, FontId, PaintCallbackInfo, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use qnc_player_contract::{session::MonitorHeader, Timebase};
use qnc_player_frame_transport::FramePayloadDescriptor;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const MODULE_ID: &str = "qnc.module.monitor";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorPaint {
    Picture,
    Message,
    Empty,
}

#[derive(Clone, Copy)]
pub struct MonitorDmaPicture<'a> {
    pub header: &'a MonitorHeader,
    pub payload: &'a FramePayloadDescriptor,
}

#[derive(Clone, Copy)]
pub struct MonitorChrome {
    pub fill: Color32,
    pub border: Color32,
    pub muted: Color32,
    pub font_size: f32,
}

#[derive(Clone, Copy)]
pub struct MonitorDmaSurface<'a> {
    pub id: egui::Id,
    pub chrome: MonitorChrome,
    pub picture: Option<MonitorDmaPicture<'a>>,
    pub message: Option<&'a str>,
}

#[derive(Clone, Copy)]
pub struct SourceMonitorSurface<'a> {
    pub id: egui::Id,
    pub chrome: MonitorChrome,
    pub picture: Option<MonitorDmaPicture<'a>>,
    pub message: Option<&'a str>,
    pub placeholder: &'a str,
}

pub fn paint_source_monitor(
    ui: &mut Ui,
    rect: Rect,
    surface: SourceMonitorSurface<'_>,
) -> MonitorPaint {
    paint_shell(ui, rect, surface.chrome);

    if let Some(picture) = surface.picture {
        return paint_dma_picture(ui, rect, surface.chrome, picture, surface.message);
    }

    if let Some(message) = surface.message.filter(|text| !text.is_empty()) {
        paint_message(ui, rect, surface.chrome, message);
        return MonitorPaint::Message;
    }

    paint_placeholder(ui, rect, surface.chrome, surface.placeholder);
    MonitorPaint::Empty
}

pub fn paint_monitor_dma_descriptor(
    ui: &mut Ui,
    rect: Rect,
    surface: MonitorDmaSurface<'_>,
) -> MonitorPaint {
    paint_shell(ui, rect, surface.chrome);
    if let Some(picture) = surface.picture {
        return paint_dma_picture(ui, rect, surface.chrome, picture, surface.message);
    }
    if let Some(message) = surface.message.filter(|text| !text.is_empty()) {
        paint_message(ui, rect, surface.chrome, message);
        return MonitorPaint::Message;
    }
    MonitorPaint::Empty
}

pub fn paint_placeholder(ui: &mut Ui, rect: Rect, chrome: MonitorChrome, label: &str) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(chrome.font_size),
        chrome.muted,
    );
}

#[derive(Clone, Copy)]
pub struct GpuPreviewTestSignalConfig {
    pub source_size: [u32; 2],
    pub timebase: Timebase,
}

impl GpuPreviewTestSignalConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_size.contains(&0) {
            return Err("monitor test source size is invalid".into());
        }
        frame_interval(self.timebase)
            .map(|_| ())
            .ok_or_else(|| "monitor test timebase is invalid".into())
    }
}

/// Public development-only GPU preview signal. It is not a playback clock and
/// is not used by production forms; it lets a blank host test monitor GPU paint
/// without Ingest, Project, shell, DB, decoder, probe, HTTP or CPU frame upload.
pub struct GpuPreviewTestSignal {
    config: GpuPreviewTestSignalConfig,
    started: Instant,
}

impl GpuPreviewTestSignal {
    pub fn new(config: GpuPreviewTestSignalConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            config,
            started: Instant::now(),
        })
    }

    pub fn paint(
        &self,
        ui: &mut Ui,
        rect: Rect,
        surface_id: egui::Id,
        chrome: MonitorChrome,
    ) -> MonitorPaint {
        paint_shell(ui, rect, chrome);
        if !ui.is_rect_visible(rect) {
            return MonitorPaint::Empty;
        }
        let Some(interval) = frame_interval(self.config.timebase) else {
            paint_message(
                ui,
                rect,
                chrome,
                "GPU monitor test source timebase is invalid.",
            );
            return MonitorPaint::Message;
        };
        ui.ctx().request_repaint_after(interval);
        let frame = current_source_frame(self.started, self.config.timebase);
        let fitted = fitted_video_rect(rect.shrink(1.0), self.config.source_size);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            fitted,
            GpuPreviewTestCallback {
                surface: surface_id.value(),
                fitted,
                frame,
            },
        ));
        MonitorPaint::Picture
    }
}

fn paint_shell(ui: &mut Ui, rect: Rect, chrome: MonitorChrome) {
    ui.allocate_rect(rect, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, chrome.fill);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, chrome.border),
        StrokeKind::Inside,
    );
}

fn paint_dma_picture(
    ui: &mut Ui,
    rect: Rect,
    chrome: MonitorChrome,
    picture: MonitorDmaPicture<'_>,
    override_message: Option<&str>,
) -> MonitorPaint {
    let message = match picture.payload.validate(picture.header) {
        Ok(()) => "GPU/DMA preview backend is not connected.",
        Err(_) => "GPU/DMA preview descriptor is invalid.",
    };
    paint_message(ui, rect, chrome, override_message.unwrap_or(message));
    MonitorPaint::Message
}

fn paint_message(ui: &mut Ui, rect: Rect, chrome: MonitorChrome, message: &str) {
    let galley = ui.painter().layout(
        message.to_string(),
        FontId::proportional(chrome.font_size),
        chrome.muted,
        (rect.width() - 24.0).max(1.0),
    );
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, chrome.muted);
}

fn frame_interval(timebase: Timebase) -> Option<Duration> {
    if timebase.fps_num <= 0 || timebase.fps_den <= 0 {
        return None;
    }
    let nanos = 1_000_000_000u128
        .checked_mul(u128::try_from(timebase.fps_den).ok()?)?
        .div_ceil(u128::try_from(timebase.fps_num).ok()?);
    Some(Duration::from_nanos(u64::try_from(nanos).ok()?))
}

fn current_source_frame(started: Instant, timebase: Timebase) -> u64 {
    let elapsed = started.elapsed().as_nanos();
    let fps_num = u128::try_from(timebase.fps_num).unwrap_or(0);
    let fps_den = u128::try_from(timebase.fps_den).unwrap_or(1).max(1);
    let frame = elapsed.saturating_mul(fps_num) / (1_000_000_000u128.saturating_mul(fps_den));
    frame.min(u128::from(u64::MAX)) as u64
}

fn fitted_video_rect(rect: Rect, source_size: [u32; 2]) -> Rect {
    let aspect = source_size[0] as f32 / source_size[1] as f32;
    let width = rect.width().min(rect.height() * aspect);
    Rect::from_center_size(rect.center(), Vec2::new(width, width / aspect))
}

struct GpuPreviewTestCallback {
    surface: u64,
    fitted: Rect,
    frame: u64,
}

impl egui_wgpu::CallbackTrait for GpuPreviewTestCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if resources.get::<GpuPreviewTestRenderer>().is_none() {
            resources.insert(GpuPreviewTestRenderer::new(device));
        }
        if let Some(renderer) = resources.get_mut::<GpuPreviewTestRenderer>() {
            renderer.update(device, queue, screen_descriptor, self);
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let [width, height] = info.screen_size_px;
        if width == 0 || height == 0 {
            return;
        }
        render_pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
        if let Some(renderer) = resources.get::<GpuPreviewTestRenderer>() {
            renderer.paint(self.surface, render_pass);
        }
    }
}

struct GpuPreviewTestSlot {
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct GpuPreviewTestRenderer {
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    slots: HashMap<u64, GpuPreviewTestSlot>,
}

impl GpuPreviewTestRenderer {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("qnc-monitor-gpu-preview-test"),
            source: wgpu::ShaderSource::Wgsl(GPU_PREVIEW_TEST_WGSL.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qnc-monitor-gpu-preview-test-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("qnc-monitor-gpu-preview-test-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("qnc-monitor-gpu-preview-test-pipeline"),
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
                    format: wgpu::TextureFormat::Bgra8Unorm,
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
            bind_group_layout,
            pipeline,
            slots: HashMap::new(),
        }
    }

    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        callback: &GpuPreviewTestCallback,
    ) {
        let slot = self.slots.entry(callback.surface).or_insert_with(|| {
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("qnc-monitor-gpu-preview-test-uniform"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("qnc-monitor-gpu-preview-test-bg"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }],
            });
            GpuPreviewTestSlot {
                uniform,
                bind_group,
            }
        });
        queue.write_buffer(
            &slot.uniform,
            0,
            gpu_preview_test_uniform(callback, screen).as_slice(),
        );
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

fn gpu_preview_test_uniform(
    callback: &GpuPreviewTestCallback,
    screen: &egui_wgpu::ScreenDescriptor,
) -> [u8; 32] {
    let ppp = screen.pixels_per_point;
    let sw = screen.size_in_pixels[0] as f32;
    let sh = screen.size_in_pixels[1] as f32;
    let px = callback.fitted.min.x * ppp;
    let py = callback.fitted.min.y * ppp;
    let pw = callback.fitted.width() * ppp;
    let ph = callback.fitted.height() * ppp;
    let floats = [
        px / sw * 2.0 - 1.0,
        1.0 - py / sh * 2.0,
        (px + pw) / sw * 2.0 - 1.0,
        1.0 - (py + ph) / sh * 2.0,
        (callback.frame % 10_000) as f32,
        0.0,
        0.0,
        0.0,
    ];
    let mut bytes = [0; 32];
    for (i, value) in floats.into_iter().enumerate() {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

const GPU_PREVIEW_TEST_WGSL: &str = r#"
struct Params {
    rect: vec4<f32>,
    frame: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;

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
    let pos = mix(params.rect.xy, params.rect.zw, uv);
    var out: VsOut;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let bar = floor(in.uv.x * 8.0);
    var color = vec3<f32>(0.02, 0.03, 0.05);
    if (bar == 0.0) { color = vec3<f32>(0.86, 0.86, 0.86); }
    if (bar == 1.0) { color = vec3<f32>(0.86, 0.86, 0.02); }
    if (bar == 2.0) { color = vec3<f32>(0.02, 0.86, 0.86); }
    if (bar == 3.0) { color = vec3<f32>(0.02, 0.86, 0.02); }
    if (bar == 4.0) { color = vec3<f32>(0.86, 0.02, 0.86); }
    if (bar == 5.0) { color = vec3<f32>(0.86, 0.02, 0.02); }
    if (bar == 6.0) { color = vec3<f32>(0.02, 0.02, 0.86); }
    let phase = fract(params.frame.x / 64.0);
    let marker = 1.0 - smoothstep(0.0, 0.015, abs(in.uv.x - phase));
    let grid = step(0.985, fract(in.uv.x * 16.0)) + step(0.985, fract(in.uv.y * 9.0));
    color = mix(color, vec3<f32>(1.0, 1.0, 1.0), clamp(marker + grid * 0.18, 0.0, 1.0));
    return vec4<f32>(color, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome() -> MonitorChrome {
        MonitorChrome {
            fill: Color32::BLACK,
            border: Color32::GRAY,
            muted: Color32::GRAY,
            font_size: 12.0,
        }
    }

    fn header() -> MonitorHeader {
        MonitorHeader {
            contract_version: qnc_player_contract::VERSION.into(),
            session_id: "session".into(),
            source_generation: 1,
            output_generation: 1,
            sequence: 1,
            source_id: "clip".into(),
            frame: 7,
            timebase: qnc_player_contract::Timebase::new(50, 1).unwrap(),
            width: 2,
            height: 1,
        }
    }

    #[test]
    fn gpu_preview_test_signal_requires_source_timebase() {
        assert!(GpuPreviewTestSignalConfig {
            source_size: [1920, 1080],
            timebase: Timebase::new(50, 1).unwrap(),
        }
        .validate()
        .is_ok());
        assert!(GpuPreviewTestSignalConfig {
            source_size: [1920, 1080],
            timebase: Timebase {
                fps_num: 0,
                fps_den: 1
            },
        }
        .validate()
        .is_err());
    }

    #[test]
    fn empty_surface_does_not_invent_a_picture() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, egui::vec2(160.0, 90.0));
                assert_eq!(
                    paint_monitor_dma_descriptor(
                        ui,
                        rect,
                        MonitorDmaSurface {
                            id: egui::Id::new("any-app-monitor"),
                            chrome: chrome(),
                            picture: None,
                            message: None,
                        },
                    ),
                    MonitorPaint::Empty
                );
            });
        });
    }

    #[test]
    fn source_monitor_is_the_embedded_widget_not_form_paint() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, egui::vec2(160.0, 90.0));
                assert_eq!(
                    paint_source_monitor(
                        ui,
                        rect,
                        SourceMonitorSurface {
                            id: egui::Id::new("any-app-source-monitor"),
                            chrome: chrome(),
                            picture: None,
                            message: None,
                            placeholder: "Odaberi klip",
                        },
                    ),
                    MonitorPaint::Empty
                );
            });
        });
    }

    #[test]
    fn dma_descriptor_is_public_but_not_painted_without_backend() {
        let ctx = egui::Context::default();
        let header = header();
        let descriptor = qnc_player_frame_transport::DmaFrameDescriptor {
            contract_version: qnc_player_frame_transport::FRAME_TRANSPORT_CONTRACT_VERSION.into(),
            backend: qnc_player_frame_transport::preferred_dma_backend_for_current_os()
                .unwrap_or(qnc_player_frame_transport::FrameTransportBackend::LinuxDmabuf),
            handle_token: "handle".into(),
            sync_token: "sync".into(),
            width: 2,
            height: 1,
            pixel_format: qnc_player_frame_transport::FrameTransportPixelFormat::Bgra8Srgb,
            modifier: None,
        };
        let payload = FramePayloadDescriptor::GpuDma { descriptor };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(ui.cursor().min, egui::vec2(160.0, 90.0));
                assert_eq!(
                    paint_monitor_dma_descriptor(
                        ui,
                        rect,
                        MonitorDmaSurface {
                            id: egui::Id::new("any-app-dma-monitor"),
                            chrome: chrome(),
                            picture: Some(MonitorDmaPicture {
                                header: &header,
                                payload: &payload,
                            }),
                            message: None,
                        },
                    ),
                    MonitorPaint::Message
                );
            });
        });
    }
}
