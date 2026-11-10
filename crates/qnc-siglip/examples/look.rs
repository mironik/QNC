//! What a picture shows: `look <model folder> <picture.jpg> <label>...` prints how well each
//! "a photo of <label>." describes the picture, and how long it took. Reads only.

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder, picture, labels @ ..] = args.as_slice() else {
        return Err("look <model folder> <picture.jpg> <label>...".into());
    };
    let started = std::time::Instant::now();
    let model = qnc_siglip::Siglip::load(std::path::Path::new(folder))?;
    let loaded = started.elapsed();
    let image = image::open(picture).map_err(|e| e.to_string())?.into_rgb8();
    let started = std::time::Instant::now();
    let seen = model.pictures(&[image])?;
    let picture_ms = started.elapsed().as_millis();
    let prompts: Vec<String> = labels.iter().map(|label| format!("a photo of {label}.")).collect();
    let started = std::time::Instant::now();
    let texts = model.texts(&prompts)?;
    let text_ms = started.elapsed().as_millis();
    println!("load {:?} picture {picture_ms} ms texts {text_ms} ms ({} texts)", loaded, prompts.len());
    let mut scores: Vec<(f32, &String)> = texts.iter().map(|text| model.score(&seen[0], text)).zip(labels).collect();
    scores.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (score, label) in scores {
        println!("{score:.4}  {label}");
    }
    Ok(())
}
