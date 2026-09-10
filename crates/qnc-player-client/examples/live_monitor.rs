//! Explicit read-only saved-media test of the same client used by Ingest.
use qnc_player_client::{Action, Launch, MediaBinding, Player, View};
use qnc_player_input::InputReader;
use qnc_work_settings::SettingsReader;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn wait(player: &Player, check: impl Fn(&View) -> bool) -> Result<View, String> {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let view = player.view();
        if let Some(error) = &view.error {
            return Err(error.clone());
        }
        if check(&view) {
            return Ok(view);
        }
        if Instant::now() >= until {
            return Err(format!("Timed out: {:?}", view.reply));
        }
        thread::sleep(Duration::from_millis(5));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err("Usage: live_monitor ROOT SOURCE_ROOT CLIP_ID [PLAY_SECONDS]".into());
    }
    let root = PathBuf::from(&args[0]);
    let sleep_start = Instant::now();
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(1));
    }
    println!(
        "1 ms sleep average: {} us",
        sleep_start.elapsed().as_micros() / 20
    );
    let settings = SettingsReader::from_root(&root)?;
    let workspace = settings.read()?.workspace_db_uri;
    let input = InputReader::new(settings).load(&workspace, &args[2])?;
    let video = input.layout.video.as_ref().ok_or("video required")?;
    println!(
        "INPUT representation={:?}; video={video:?}",
        input.representation
    );
    let seconds = args
        .get(3)
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(10);
    if !(1..=600).contains(&seconds) {
        return Err("PLAY_SECONDS must be 1..600".into());
    }
    let sustained_frame = (seconds * video.timebase.fps_num as u64 / video.timebase.fps_den as u64)
        .min(video.duration_frames.saturating_sub(10));
    let source = qnc_source_reader::SourceReference::from_uri(&input.media()?.media_uri)?
        .source_uri()
        .to_string();
    let prepare = || {
        let input = input.clone();
        let source = source.clone();
        let source_root = PathBuf::from(&args[1]);
        let executable = root.join("target").join("release").join(format!(
            "qnc-broadcast-player{}",
            std::env::consts::EXE_SUFFIX
        ));
        move || {
            Ok(Launch {
                executable,
                input,
                media_binding: MediaBinding::Local {
                    source_uri: source,
                    root: source_root,
                },
            })
        }
    };
    let player = Player::new()?;
    player.prepare(prepare());
    let ready = wait(&player, View::ready)?;
    assert!(!ready.video_visible);
    assert!(ready.picture.is_none());
    let session = ready.reply.as_ref().unwrap().session_id.clone();
    println!("READY with thumbnail-only monitor; session={session}");
    player.send(Action::TogglePlayPause)?;
    let moving = wait(&player, |v| {
        v.playing() && v.picture.as_ref().is_some_and(|p| p.header.frame >= 10)
    })?;
    let picture = moving.picture.as_ref().unwrap();
    assert!(moving.video_visible);
    assert!(
        picture
            .rgba
            .chunks_exact(4)
            .any(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
    );
    let next = wait(&player, |v| {
        v.picture
            .as_ref()
            .is_some_and(|p| p.header.frame > picture.header.frame + 5)
    })?;
    assert_ne!(next.picture.as_ref().unwrap().rgba, picture.rgba);
    println!(
        "PLAY real moving RGBA {}x{}",
        picture.header.width, picture.header.height
    );
    let began = Instant::now();
    let mut observed = next;
    while observed.picture.as_ref().unwrap().header.frame < sustained_frame {
        if began.elapsed() > Duration::from_secs(seconds + 10) {
            return Err("sustained playback deadline exceeded".into());
        }
        let previous = observed.picture.as_ref().unwrap().header.frame;
        observed = wait(&player, |v| {
            v.picture
                .as_ref()
                .is_some_and(|p| p.header.frame > previous)
        })?;
    }
    let sustained = observed;
    println!(
        "SUSTAINED PLAY through frame {}",
        sustained.picture.as_ref().unwrap().header.frame
    );
    player.send(Action::TogglePlayPause)?;
    let paused = wait(&player, |v| !v.playing() && v.ready())?;
    let frame = paused.picture.as_ref().unwrap().header.frame;
    thread::sleep(Duration::from_millis(250));
    assert_eq!(player.view().picture.as_ref().unwrap().header.frame, frame);
    player.send(Action::Step(1))?;
    wait(&player, |v| {
        v.ready()
            && v.picture
                .as_ref()
                .is_some_and(|p| p.header.frame == frame + 1)
    })?;
    println!("PASS play/pause/exact step; paused frame={frame}");
    player.send(Action::TogglePlayPause)?;
    wait(&player, View::playing)?;
    player.prepare(prepare());
    assert!(player.view().picture.is_none());
    let replacement = wait(&player, View::ready)?;
    assert_ne!(replacement.reply.as_ref().unwrap().session_id, session);
    assert!(!replacement.playing());
    assert!(replacement.picture.is_none());
    assert!(!replacement.video_visible);
    println!("PASS replacement cuts old playback; new session shows only thumbnail until Play");
    drop(player);
    thread::sleep(Duration::from_secs(3));
    Ok(())
}
