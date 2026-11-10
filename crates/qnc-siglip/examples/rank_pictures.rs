//! The best three labels of each picture, as the model ranks them (a check of a model on pictures
//! of the newsroom's own): `rank_pictures <model folder> <labels file: code|prompt per line> <picture>...`.

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder, labels, pictures @ ..] = args.as_slice() else {
        return Err("rank_pictures <model folder> <labels file> <picture>...".into());
    };
    let model = qnc_siglip::Siglip::load(std::path::Path::new(folder))?;
    let text = std::fs::read_to_string(labels).map_err(|e| e.to_string())?;
    let (codes, prompts): (Vec<&str>, Vec<String>) = text.lines().filter_map(|line| line.split_once('|')).map(|(code, prompt)| (code, prompt.to_string())).unzip();
    let vectors = model.texts(&prompts)?;
    for path in pictures {
        let image = image::open(path).map_err(|e| e.to_string())?.to_rgb8();
        let picture = model.pictures(&[image])?.remove(0);
        let mut scores: Vec<(f32, &str)> = vectors.iter().zip(&codes).map(|(vector, code)| (model.score(&picture, vector), *code)).collect();
        scores.sort_by(|a, b| b.0.total_cmp(&a.0));
        let best: Vec<String> = scores.iter().take(3).map(|(score, code)| format!("{code}:{score:.3}")).collect();
        println!("{} {}", std::path::Path::new(path).file_stem().map_or(String::new(), |s| s.to_string_lossy().into_owned()), best.join(" "));
    }
    Ok(())
}
