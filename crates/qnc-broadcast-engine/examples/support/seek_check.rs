use super::*;

fn wait(runtime: &mut Runtime, stop: &AtomicBool, done: impl Fn(&Runtime) -> bool) -> Result<()> {
    let start = Instant::now();
    while !done(runtime) {
        if stop.load(Ordering::Acquire) || start.elapsed() > Duration::from_secs(15) {
            return Err("seek diagnostic interrupted or timed out".into());
        }
        runtime.tick()?;
        if runtime.state().presented_frame.is_some() {
            return Err("submission relabeled as physical presentation".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

pub fn exercise(runtime: &mut Runtime, frames: u64, stop: &AtomicBool) -> Result<()> {
    if frames < 30 {
        return Err("seek diagnostic needs at least 30 saved frames".into());
    }
    wait(runtime, stop, |r| r.state().play_ready)?;
    let before = runtime.state().clone();
    if runtime.cue_frame(frames, true).is_ok() || runtime.state() != &before {
        return Err("invalid exclusive-end cue changed state".into());
    }
    for target in [frames / 2, 0, frames - 1, frames / 3, 0] {
        let old_frame = runtime.state().carrier_frame;
        // Supersede work after its first preparation tick, not just before dispatch.
        runtime.cue_frame((target + 10) % frames, true)?;
        runtime.tick()?;
        let started = Instant::now();
        runtime.cue_frame(target, true)?;
        if runtime.state().carrier_frame != old_frame || runtime.state().play_ready {
            return Err("cue confirmed an unprepared frame".into());
        }
        if runtime.play().is_ok() {
            return Err("Play accepted during seek preparation".into());
        }
        wait(runtime, stop, |r| r.state().play_ready)?;
        if runtime.state().carrier_frame != target
            || runtime.state().submitted_frame != Some(target)
        {
            return Err("seek prepared or submitted the wrong source frame".into());
        }
        if let Some(audio) = runtime.audio_telemetry()
            && (audio.status != qnc_audio_output::Status::Ready || audio.submitted_frames != 0)
        {
            return Err("seek started audio without Play".into());
        }
        println!(
            "Cue {target}: ready in {} ms, actual target submitted",
            started.elapsed().as_millis()
        );
        let play_at = Instant::now();
        runtime.play()?;
        println!(
            "Ready Play at {target}: {} us",
            play_at.elapsed().as_micros()
        );
        if target == frames - 1 {
            wait(runtime, stop, |r| r.state().at_end)?;
            if runtime.state().submitted_frame != Some(target) {
                return Err("exclusive end did not retain the final frame".into());
            }
            println!("Final frame retained at exclusive end");
        } else {
            wait(runtime, stop, |r| r.state().carrier_frame >= target + 5)?;
            if let Some(audio) = runtime.audio_telemetry()
                && (audio.submitted_frames == 0 || audio.start_to_first_callback_ns.is_none())
            {
                return Err("Play after seek produced no device audio".into());
            }
            runtime.pause()?;
            wait(runtime, stop, |r| r.state().play_ready)?;
            println!(
                "Pause at {}; audio {:?}",
                runtime.state().carrier_frame,
                runtime.audio_telemetry()
            );
        }
    }
    // Leave the last confirmed image visible for the native visual check.
    let hold = Instant::now();
    while hold.elapsed() < Duration::from_secs(15) && !stop.load(Ordering::Acquire) {
        runtime.tick()?;
        thread::sleep(Duration::from_millis(10));
    }
    if stop.load(Ordering::Acquire) {
        return Err("seek diagnostic interrupted".into());
    }
    Ok(())
}
