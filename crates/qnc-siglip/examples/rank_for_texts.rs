//! The pictures most like each text (a check of how a model finds B-roll for the words of an
//! OFF): `rank_for_texts <model folder> <texts file> <picture>...` (one text a line). Reads only.

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder, texts, pictures @ ..] = args.as_slice() else {
        return Err("rank_for_texts <model folder> <texts file> <picture>...".into());
    };
    let model = qnc_siglip::Siglip::load(std::path::Path::new(folder))?;
    let texts: Vec<String> = std::fs::read_to_string(texts).map_err(|e| e.to_string())?.lines().filter(|line| !line.trim().is_empty()).map(str::to_string).collect();
    let vectors = model.texts(&texts)?;
    let mut seen = Vec::new();
    for path in pictures {
        let image = image::open(path).map_err(|e| e.to_string())?.to_rgb8();
        seen.push(model.pictures(&[image])?.remove(0));
    }
    let name = |path: &str| {
        let path = std::path::Path::new(path);
        let folder = path.parent().and_then(|p| p.file_name()).map_or(String::new(), |f| f.to_string_lossy().into_owned());
        format!("{folder}/{}", path.file_stem().map_or(String::new(), |s| s.to_string_lossy().into_owned()))
    };
    for (text, vector) in texts.iter().zip(&vectors) {
        let mut like: Vec<(f32, &str)> = seen.iter().zip(pictures).map(|(picture, path)| (model.score(picture, vector), path.as_str())).collect();
        like.sort_by(|a, b| b.0.total_cmp(&a.0));
        let best: Vec<String> = like.iter().take(3).map(|(score, path)| format!("{}:{score:.4}", name(path))).collect();
        println!("{text}\n   {}", best.join("  "));
    }
    Ok(())
}
