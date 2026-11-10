//! How well each text describes one picture, as the model scores it (a check of the model
//! against the numbers its makers publish): `score_image <model folder> <picture> <text>...`.

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder, picture, texts @ ..] = args.as_slice() else {
        return Err("score_image <model folder> <picture> <text>...".into());
    };
    let model = qnc_siglip::Siglip::load(std::path::Path::new(folder))?;
    let image = image::open(picture).map_err(|e| e.to_string())?.to_rgb8();
    let picture = model.pictures(&[image])?.remove(0);
    for (text, vector) in texts.iter().zip(model.texts(texts)?) {
        println!("{:6.2}%  {text}", model.score(&picture, &vector) * 100.0);
    }
    Ok(())
}
