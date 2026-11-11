//! Keeps a caption and a correction and reads them back where the QNC root of this run keeps them
//! (on its AI server when its ai-place.json names one): `through_server ROOT`. Writes one caption
//! and one example of the model "proba".
fn main() -> Result<(), String> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("through_server ROOT")?);
    let key = qnc_shot_examples::key("qnc://local/project/proba", "c1", 0, 100);
    qnc_shot_captions::save(&root, "proba", &key, "a harbour with boats")?;
    let example = qnc_shot_examples::Example { model: "proba".into(), key: key.clone(), label: "harbour".into(), vector: vec![0.6, 0.8] };
    qnc_shot_examples::save(&root, &example)?;
    println!("captions: {:?}", qnc_shot_captions::read_clip(&root, "proba", "qnc://local/project/proba", "c1")?);
    println!("examples: {:?}", qnc_shot_examples::read(&root, "proba")?.iter().map(|e| (&e.key, &e.label)).collect::<Vec<_>>());
    Ok(())
}
