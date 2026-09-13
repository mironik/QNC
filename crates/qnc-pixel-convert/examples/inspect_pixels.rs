//! Read-only native-pixel diagnostic, not an application workflow or player clock.
use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder};
use qnc_media_metadata::StreamDetails;
use qnc_media_stream::{LocalSource, MediaStream, SourceReference};
use qnc_pixel_convert::{ConversionSpec, Converter};
use qnc_player_input::InputReader;
use qnc_video_output::{FrameHeader, OutputConfig, PixelFormat, PreparedFrame, VideoOutput};
use qnc_work_settings::SettingsReader;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    io::{Read, Seek},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Pixels {
    rgba: Vec<u8>,
    ordinal: u64,
}
struct Sample {
    spec: ConversionSpec,
    images: Vec<Pixels>,
}

fn source_hash(stream: &mut MediaStream) -> Result<String> {
    stream.rewind()?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn load(args: &[std::ffi::OsString]) -> Result<Sample> {
    if args.len() < 3 {
        return Err(
            "Usage: inspect_pixels ROOT OWNER_SOURCE_DIRECTORY CLIP_ID [--original] [--show]"
                .into(),
        );
    }
    let root = PathBuf::from(&args[0]);
    let owner = PathBuf::from(&args[1]);
    let id = args[2].to_str().ok_or("invalid clip id")?;
    if args[3..].iter().any(|a| a != "--original" && a != "--show") {
        return Err("unknown option".into());
    }
    let reader = SettingsReader::from_root(&root)?;
    let settings = reader.read()?;
    let input = InputReader::new(reader.clone()).load(&settings.workspace_db_uri, id)?;
    let original = args[3..].iter().any(|a| a == "--original");
    let media = if original {
        &input.snapshot.metadata.original
    } else {
        input.media()?
    };
    let (index, video) = media
        .streams
        .iter()
        .find_map(|stream| match &stream.details {
            StreamDetails::Video(video) => Some((stream.index.as_ref()?.value, video)),
            _ => None,
        })
        .ok_or("saved video stream missing")?;
    let spec = ConversionSpec::from_saved(video)?;
    println!("Saved conversion spec: {}", serde_json::to_string(&spec)?);
    let mut converter = Converter::prepare(spec.clone(), spec.scratch_bytes()?)?;
    let mut rgba = vec![0; spec.output_bytes()?];
    let reference = SourceReference::from_uri(&media.media_uri)?;
    let source = LocalSource::new(reference.source_uri(), &owner)?;
    let mut verify = MediaStream::local(&source, &media.media_uri)?;
    let before = source_hash(&mut verify)?;
    let request = DecodeRequest {
        version: qnc_media_decode::VERSION.into(),
        media: media.clone(),
        stream_index: index,
        start: None,
    };
    let mut decoder = Decoder::open(
        request,
        MediaStream::local(&source, &media.media_uri)?,
        qnc_decoder_catalog::installed_config()?,
    )?;
    let pid = decoder.process_id();
    let mut images = Vec::new();
    let mut timings = Vec::new();
    let mut hashes = Vec::new();
    let mut time_base = None;
    let mut last_pts = None;
    let started = Instant::now();
    for _ in 0..32 {
        let Some(packet) = decoder.next_packet()? else {
            break;
        };
        if decoder.process_id() != pid
            || packet.media_uri != media.media_uri
            || packet.stream_index != index
            || packet.format
                != (DecodedFormat::Video {
                    width: spec.width,
                    height: spec.height,
                    pixel_format: spec.layout.name().into(),
                })
        {
            return Err("decoder identity/format changed".into());
        }
        if time_base.is_some_and(|base| base != packet.time_base)
            || last_pts.is_some_and(|pts| pts >= packet.pts)
        {
            return Err("non-advancing source timestamps".into());
        }
        time_base = Some(packet.time_base);
        last_pts = Some(packet.pts);
        let native_hash = Sha256::digest(&packet.bytes);
        let begin = Instant::now();
        converter.convert(&packet.bytes, &mut rgba)?;
        timings.push(begin.elapsed().as_micros());
        if native_hash != Sha256::digest(&packet.bytes) {
            return Err("converter changed source pixels".into());
        }
        hashes.push(Sha256::digest(&rgba));
        if packet.ordinal == 0 || packet.ordinal == 31 {
            images.push(Pixels {
                rgba: rgba.clone(),
                ordinal: packet.ordinal,
            });
        }
    }
    decoder.cancel();
    if decoder.process_id().is_some() {
        return Err("decoder not reaped".into());
    }
    if source_hash(&mut verify)? != before || reader.read()? != settings {
        return Err("source/settings changed".into());
    }
    if images.len() != 2 || hashes.windows(2).all(|p| p[0] == p[1]) {
        return Err("diagnostic requires two different prepared images".into());
    }
    timings.sort_unstable();
    println!(
        "{}",
        serde_json::json!({"project":settings.project_name, "clip":id,
        "policy_representation": input.representation, "original_diagnostic_override":original,
        "frames":timings.len(), "convert_us_min":timings[0], "convert_us_median":timings[timings.len()/2],
        "convert_us_max":timings[timings.len()-1], "elapsed_ms":started.elapsed().as_millis(),
        "source_unchanged":true, "settings_unchanged":true, "native_bytes_unchanged":true,
        "database_writes":0, "process_reaped":true, "player_runtime":false})
    );
    Ok(Sample { spec, images })
}

struct Diagnostic {
    sample: Sample,
    window: Option<Arc<Window>>,
    output: Option<VideoOutput>,
    tokens: Vec<PreparedFrame>,
    begin: Option<Instant>,
    next: Option<Instant>,
    inflight: bool,
    image: usize,
}
impl ApplicationHandler for Diagnostic {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("QNC Saved Pixels Diagnostic")
                        .with_resizable(false)
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
        .unwrap();
        let size = window.inner_size();
        let spec = &self.sample.spec;
        let mut output = pollster::block_on(VideoOutput::open(
            &adapter,
            Some(surface),
            OutputConfig {
                version: qnc_video_output::VERSION.into(),
                session_id: "saved-pixel-diagnostic".into(),
                generation: 1,
                width: spec.width,
                height: spec.height,
                pixel_format: PixelFormat::Rgba8Srgb,
                slots: 2,
                pool_budget_bytes: (spec.output_bytes().unwrap() * 2) as u64,
            },
            (size.width, size.height),
        ))
        .unwrap();
        for (i, image) in self.sample.images.iter().enumerate() {
            self.tokens.push(
                output
                    .prepare(
                        FrameHeader {
                            version: qnc_video_output::VERSION.into(),
                            session_id: "saved-pixel-diagnostic".into(),
                            generation: 1,
                            sequence: i as u64,
                            source_id: "saved-raster-diagnostic".into(),
                            frame_number: image.ordinal,
                            width: spec.width,
                            height: spec.height,
                            pixel_format: PixelFormat::Rgba8Srgb,
                        },
                        &image.rgba,
                    )
                    .unwrap(),
            );
        }
        self.window = Some(window);
        self.output = Some(output);
        self.begin = Some(Instant::now());
        self.next = Some(Instant::now());
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .begin
            .is_some_and(|t| t.elapsed() > Duration::from_secs(60))
        {
            event_loop.exit();
            return;
        }
        let Some(output) = self.output.as_mut() else {
            return;
        };
        if output.poll().unwrap().is_some() {
            self.inflight = false;
        }
        if !self.inflight
            && self.tokens.iter().all(|t| output.is_ready(t).unwrap())
            && self.next.is_some_and(|t| Instant::now() >= t)
        {
            let begin = Instant::now();
            output.submit(&self.tokens[self.image]).unwrap();
            println!(
                "Prepared image {} surface submit {:?}",
                self.sample.images[self.image].ordinal,
                begin.elapsed()
            );
            self.inflight = true;
            self.image = 1 - self.image;
            self.next = Some(Instant::now() + Duration::from_secs(4));
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(5),
        ));
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let sample = load(&args)?;
    if args.iter().any(|a| a == "--show") {
        EventLoop::new()?.run_app(&mut Diagnostic {
            sample,
            window: None,
            output: None,
            tokens: Vec::new(),
            begin: None,
            next: None,
            inflight: false,
            image: 0,
        })?;
        println!("Native saved-pixel diagnostic closed.");
    }
    Ok(())
}
