//! The captions of pictures given, through the shot-caption program of this host's AI catalog:
//! `caption_files <picture>...`. The pictures are copied into an empty temporary folder first
//! (they are only read); nothing is kept.

fn main() -> Result<(), String> {
    let captioner = qnc_ai_catalog::installed_shot_captioner()?;
    let client = qnc_shot_caption::CaptionClient::new(&qnc_plugin_process::beside_application(&captioner.program)?);
    let temp = std::env::temp_dir().join("qnc-shot-caption-example").join(std::process::id().to_string());
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let given: Vec<String> = std::env::args().skip(1).collect();
    let mut pictures = Vec::new();
    for (n, path) in given.iter().enumerate() {
        let copy = temp.join(format!("{n:03}.jpg"));
        std::fs::copy(path, &copy).map_err(|e| format!("{path}: {e}"))?;
        pictures.push(copy);
    }
    let started = std::time::Instant::now();
    let captions = client.captions(&qnc_shot_caption::CaptionsRequest { model: captioner.model.clone(), pictures });
    let _ = std::fs::remove_dir_all(&temp);
    println!("{} pictures in {} ms", given.len(), started.elapsed().as_millis());
    for (path, caption) in given.iter().zip(captions?) {
        println!("{path} | {caption}");
    }
    Ok(())
}
