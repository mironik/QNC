use eframe::egui::{self, Color32, Frame, Id, Vec2};
use qnc_monitor::{GpuPreviewTestSignal, GpuPreviewTestSignalConfig, MonitorChrome};
use qnc_player_contract::Timebase;

fn main() -> eframe::Result<()> {
    let timebase = parse_timebase().unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    let app = MonitorLab {
        signal: GpuPreviewTestSignal::new(GpuPreviewTestSignalConfig {
            source_size: [1920, 1080],
            timebase,
        })
        .unwrap_or_else(|message| {
            eprintln!("{message}");
            std::process::exit(2);
        }),
    };
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(960.0, 540.0))
            .with_min_inner_size(Vec2::new(640.0, 360.0)),
        ..Default::default()
    };
    eframe::run_native(
        "QNC GPU/DMA Monitor Lab",
        options,
        Box::new(|_| Ok(Box::new(app))),
    )
}

struct MonitorLab {
    signal: GpuPreviewTestSignal,
}

impl eframe::App for MonitorLab {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::BLACK))
            .show(ctx, |ui| {
                let chrome = MonitorChrome {
                    fill: Color32::BLACK,
                    border: Color32::from_rgb(55, 66, 84),
                    muted: Color32::from_rgb(168, 174, 187),
                    font_size: 18.0,
                };
                self.signal
                    .paint(ui, ui.max_rect(), Id::new("qnc-dma-monitor-lab"), chrome);
            });
    }
}

fn parse_timebase() -> Result<Timebase, String> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--timebase"), Some(value), None) => parse_rational_timebase(&value),
        _ => Err("Usage: qnc-dma-monitor-lab --timebase 50/1".into()),
    }
}

fn parse_rational_timebase(value: &str) -> Result<Timebase, String> {
    let Some((num, den)) = value.split_once('/') else {
        return Err("Timebase must be written as fps_num/fps_den.".into());
    };
    let fps_num = num
        .parse::<i64>()
        .map_err(|_| "Timebase numerator is invalid.".to_string())?;
    let fps_den = den
        .parse::<i64>()
        .map_err(|_| "Timebase denominator is invalid.".to_string())?;
    Timebase::new(fps_num, fps_den).map_err(|e| e.to_string())
}
