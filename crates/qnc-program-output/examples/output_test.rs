//! Diagnostic: the program output process on the external screen (or the main one
//! without it), fed for eight seconds with a moving 50p test picture through a
//! shared-memory frame map, as the Broadcast Player does. The output logs what it showed
//! (`program-output refreshes=.. new_pictures=.. skipped=..`, with QNC_PLAYER_DIAGNOSTICS=1).
//! Build `qnc-program-output-host` first; run from the workspace:
//! `cargo run --release -p qnc-program-output --example output_test`

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use qnc_player_contract::{session::MonitorHeader, Timebase, VERSION};
use qnc_player_frame_transport::{LatestFrameWriter, MONITOR_PREVIEW_FRAME_CAPACITY};

fn main() -> Result<(), String> {
    let screens = qnc_program_output::screens()?;
    let screen = qnc_program_output::output_screen(&screens)
        .or_else(|| screens.iter().copied().find(|screen| screen.primary))
        .ok_or("no screen")?;
    println!("screens {screens:?} -> output on {screen:?}");
    let map = std::env::temp_dir().join(format!("qnc-output-test-{}.map", std::process::id()));
    let mut writer = LatestFrameWriter::create(&map, MONITOR_PREVIEW_FRAME_CAPACITY)?;
    let host = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .and_then(|dir| dir.parent())
        .map(|dir| dir.join(format!("qnc-program-output-host{}", std::env::consts::EXE_SUFFIX)))
        .ok_or("no host path")?;
    let mut child = Command::new(host)
        .args(["--x", &screen.x.to_string(), "--y", &screen.y.to_string()])
        .args(["--width", &screen.width.to_string(), "--height", &screen.height.to_string()])
        .args(["--refresh-hz", &screen.refresh_hz.unwrap_or(0).to_string()])
        .args(["--interlaced", if screen.interlaced { "1" } else { "0" }])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    writeln!(stdin, "map {}", map.display()).map_err(|error| error.to_string())?;
    let (width, height) = (960usize, 540usize);
    let started = Instant::now();
    let mut frame = 0u64;
    while started.elapsed() < Duration::from_secs(8) {
        // Exactly every 20 ms, as the player clock: the OS sleep alone is late by up
        // to 15 ms, which itself makes pictures repeat and drop.
        let due = started + Duration::from_millis(frame * 20);
        while let Some(wait) = due.checked_duration_since(Instant::now()) {
            if wait > Duration::from_millis(3) {
                std::thread::sleep(wait - Duration::from_millis(2));
            } else {
                std::hint::spin_loop();
            }
        }
        let mut rgba = vec![0u8; width * height * 4];
        for (index, pixel) in rgba.chunks_exact_mut(4).enumerate() {
            let bar = ((index % width) as u64 + frame * 8) % (width as u64) < 60;
            pixel.copy_from_slice(&if bar { [255, 255, 255, 255] } else { [20, 60, 140, 255] });
        }
        let header = MonitorHeader {
            contract_version: VERSION.into(),
            session_id: "output-test".into(),
            source_generation: 1,
            output_generation: 1,
            sequence: frame + 1,
            source_id: "test-pattern".into(),
            frame,
            timebase: Timebase::new(50, 1).map_err(|error| error.to_string())?,
            width: width as u32,
            height: height as u32,
        };
        writer.publish(&header, &rgba)?;
        frame += 1;
    }
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_file(&map);
    Ok(())
}
