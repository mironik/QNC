//! The shot analysis and the shot content of one MXF original, as the background worker makes
//! them, without a project: `look_card <original.mxf> <h264|mpeg2video> [pictures per part]`.
//! The original is only read; nothing is written but temporary pictures, removed afterwards.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, codec) = match args.as_slice() {
        [path, codec, ..] => (Path::new(path), codec.as_str()),
        _ => return Err("look_card <original.mxf> <h264|mpeg2video> [pictures per part]".into()),
    };
    let engine = qnc_ai_catalog::installed_shot_content()?;
    let count = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(engine.pictures);
    let cancel = AtomicBool::new(false);
    let pace = qnc_shot_analysis::Pace { player_plays: &|| false, wanted: &|| None, cancel: &cancel };
    let started = Instant::now();
    let found = qnc_shot_analysis::analyse_file("look-card", codec, path, &pace)?;
    println!("analysis: {} pictures, {} shots, {} parts, {} ms", found.pictures, found.shots.len(), found.parts.len(), started.elapsed().as_millis());
    let client = qnc_shot_content::ContentClient::new(&qnc_plugin_process::beside_application(&engine.program)?);
    let labels = qnc_shot_labels::Labels::read(&qnc_shot_labels::installed_path()?)?;
    let started = Instant::now();
    let texts = client.texts(&qnc_shot_content::TextsRequest { model: engine.model.clone(), texts: labels.prompts() })?;
    println!("agent ready and {} labels: {} ms", labels.labels.len(), started.elapsed().as_millis());
    let started = Instant::now();
    let parts = qnc_shot_content_run::look_file("look-card", codec, path, &found.parts, count, &engine.model, &client, &pace)?;
    let pictures: usize = parts.iter().map(|part| part.frames.len()).sum();
    println!("content: {} parts, {pictures} pictures, {} ms", parts.len(), started.elapsed().as_millis());
    for part in &parts {
        let best: Vec<String> = labels.best(&texts.logits(&part.vector)).into_iter().take(3).map(|(code, share)| format!("{code} {share:.2}")).collect();
        println!("{:>6}-{:<6} frames {:?}: {}", part.from, part.to, part.frames, best.join(", "));
    }
    Ok(())
}
