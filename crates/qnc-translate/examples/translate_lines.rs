//! Each line of a file in the model's target language: `translate_lines <model folder> <file>`.
//! Reads only.
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder, file] = args.as_slice() else { return Err("translate_lines <model folder> <file>".into()) };
    let started = std::time::Instant::now();
    let mut translator = qnc_translate::Translator::load(std::path::Path::new(folder))?;
    eprintln!("loaded in {} ms", started.elapsed().as_millis());
    for line in std::fs::read_to_string(file).map_err(|e| e.to_string())?.lines().filter(|line| !line.trim().is_empty()) {
        let started = std::time::Instant::now();
        let english = translator.translate(line)?;
        println!("{} ms | {english}", started.elapsed().as_millis());
    }
    Ok(())
}
