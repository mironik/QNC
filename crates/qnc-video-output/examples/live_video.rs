//! Finite native output diagnostic. Timing here is a test stimulus, not a player clock.
use qnc_video_output::{
    FrameHeader, OutputConfig, PixelFormat, PreparedFrame, VERSION, VideoOutput,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

#[derive(Default)]
struct Diagnostic {
    window: Option<Arc<Window>>,
    output: Option<VideoOutput>,
    frames: Vec<PreparedFrame>,
    sequence: u64,
    started: Option<Instant>,
    next_frame: Option<Instant>,
    submitted_at: Option<Instant>,
    image: usize,
    resize: Option<(u32, u32)>,
}

impl Diagnostic {
    fn prepare(&mut self) {
        self.frames.clear();
        for image in 0..4 {
            let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let quadrant = usize::from(x >= WIDTH / 2) + 2 * usize::from(y >= HEIGHT / 2);
                    let palette = [
                        [230, 30, 40, 255],
                        [20, 210, 80, 255],
                        [30, 80, 230, 255],
                        [240, 210, 30, 255],
                    ];
                    let color = if x.abs_diff(80 + image * 160) < 10 {
                        [255; 4]
                    } else {
                        palette[(quadrant + image as usize) % 4]
                    };
                    pixels.extend_from_slice(&color);
                }
            }
            self.sequence += 1;
            let header = FrameHeader {
                version: VERSION.into(),
                session_id: "native-output-diagnostic".into(),
                generation: 1,
                sequence: self.sequence,
                source_id: "prepared-test-pattern".into(),
                frame_number: u64::from(image),
                width: WIDTH,
                height: HEIGHT,
                pixel_format: PixelFormat::Rgba8Srgb,
            };
            self.frames.push(
                self.output
                    .as_mut()
                    .unwrap()
                    .prepare(header, &pixels)
                    .unwrap(),
            );
        }
    }
}

impl ApplicationHandler for Diagnostic {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let begin = Instant::now();
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("QNC Video Output Diagnostic")
                        .with_inner_size(winit::dpi::PhysicalSize::new(960, 640)),
                )
                .unwrap(),
        );
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("native output requires a graphics adapter");
        println!("Adapter: {:?}", adapter.get_info());
        let size = window.inner_size();
        self.output = Some(
            pollster::block_on(VideoOutput::open(
                &adapter,
                Some(surface),
                OutputConfig {
                    version: VERSION.into(),
                    session_id: "native-output-diagnostic".into(),
                    generation: 1,
                    width: WIDTH,
                    height: HEIGHT,
                    pixel_format: PixelFormat::Rgba8Srgb,
                    slots: 4,
                    pool_budget_bytes: u64::from(WIDTH * HEIGHT * 4 * 4),
                },
                (size.width, size.height),
            ))
            .unwrap(),
        );
        self.window = Some(window);
        self.prepare();
        println!(
            "Device/pipeline/pool preparation and uploads queued: {:?}",
            begin.elapsed()
        );
        self.started = Some(Instant::now());
        self.next_frame = Some(Instant::now());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize = Some((size.width, size.height)),
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .started
            .is_some_and(|start| start.elapsed() > Duration::from_secs(45))
        {
            event_loop.exit();
            return;
        }
        let Some(output) = self.output.as_mut() else {
            return;
        };
        if let Some(done) = output.poll().unwrap() {
            println!(
                "Frame {} GPU completion observed: {:?} (not scanout)",
                done.submission.frame.frame_number,
                self.submitted_at.take().unwrap().elapsed()
            );
        }
        if let Some(size) = self.resize {
            match output.resize(size) {
                Ok(()) => {
                    self.resize = None;
                    if size.0 == 0 || size.1 == 0 {
                        self.frames.clear();
                    } else {
                        self.prepare();
                    }
                }
                Err(qnc_video_output::OutputError::Busy) => (),
                Err(error) => panic!("resize: {error}"),
            }
        }
        let output = self.output.as_mut().unwrap();
        if self.resize.is_none()
            && !self.frames.is_empty()
            && self.submitted_at.is_none()
            && self.frames.iter().all(|f| output.is_ready(f).unwrap())
            && self.next_frame.is_some_and(|next| Instant::now() >= next)
        {
            let start = Instant::now();
            output.submit(&self.frames[self.image]).unwrap();
            println!(
                "Frame {} prepared surface submit API: {:?}",
                self.image,
                start.elapsed()
            );
            self.submitted_at = Some(start);
            self.image = (self.image + 1) % self.frames.len();
            self.next_frame = Some(Instant::now() + Duration::from_secs(3));
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(5),
        ));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EventLoop::new()?.run_app(&mut Diagnostic::default())?;
    println!("Native diagnostic closed; no playback worker remains.");
    Ok(())
}
