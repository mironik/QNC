//! Diagnostic: the program output on the external screen, or on the main screen when
//! no second screen is there, with a moving test picture for five seconds; then the
//! window closes by itself. The screens are read inside the frame, as the desktop does
//! (only then the OS gives them in pixels).
//! `cargo run -p qnc-program-output --example on_main_screen`

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

struct Try {
    started: Instant,
}

impl eframe::App for Try {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let screens = qnc_program_output::screens().expect("screens");
        let screen = qnc_program_output::output_screen(&screens)
            .or_else(|| screens.iter().copied().find(|screen| screen.primary))
            .expect("a screen");
        let frame = (self.started.elapsed().as_millis() / 40) as u64;
        let (width, height) = (320usize, 180usize);
        let mut rgba = vec![0u8; width * height * 4];
        for (index, pixel) in rgba.chunks_exact_mut(4).enumerate() {
            let bar = ((index % width) as u64 + frame * 4) % (width as u64) < 40;
            pixel.copy_from_slice(&if bar { [255, 255, 255, 255] } else { [20, 60, 140, 255] });
        }
        egui::CentralPanel::default().show(ctx, |ui| ui.label(format!("frame {frame} {screen:?}")));
        let picture = qnc_program_output::ProgramPicture {
            session_id: "try".into(),
            generation: 1,
            sequence: frame,
            size: [width, height],
            rgba: Arc::from(rgba),
        };
        qnc_program_output::present_picture(ctx, screen, Some(picture));
        if self.started.elapsed() > Duration::from_secs(5) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(40));
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "try",
        eframe::NativeOptions { renderer: eframe::Renderer::Wgpu, ..Default::default() },
        Box::new(|_| Ok(Box::new(Try { started: Instant::now() }))),
    )
}
