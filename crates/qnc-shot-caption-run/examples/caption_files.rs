//! The captions of pictures given, through the shot-caption program of this host's AI catalog:
//! `caption_files <picture>...`. The pictures are copied into an empty temporary folder first
//! (they are only read); nothing is kept.

fn main() -> Result<(), String> {
    let captioner = qnc_ai_catalog::installed_shot_captioner()?;
    let client = qnc_shot_caption::CaptionClient::new(&qnc_plugin_process::beside_application(&captioner.program)?);
    let given: Vec<String> = std::env::args().skip(1).collect();
    let files: Vec<std::path::PathBuf> = given.iter().map(std::path::PathBuf::from).collect();
    let started = std::time::Instant::now();
    let captions = client.captions(&qnc_shot_caption::CaptionsRequest::of_files(&captioner.model, &files)?);
    println!("{} pictures in {} ms", given.len(), started.elapsed().as_millis());
    for (path, caption) in given.iter().zip(captions?) {
        println!("{path} | {caption}");
    }
    Ok(())
}
