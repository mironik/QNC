//! Each line of a file in English through the plug-in program (the contract's acceptance):
//! `translate_with_program <program> <model name> <file>`. Reads only.
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [program, model, file] = args.as_slice() else { return Err("translate_with_program <program> <model name> <file>".into()) };
    let texts: Vec<String> = std::fs::read_to_string(file).map_err(|e| e.to_string())?.lines().filter(|line| !line.trim().is_empty()).map(str::to_string).collect();
    let client = qnc_text_translate::TranslateClient::new(std::path::Path::new(program));
    let started = std::time::Instant::now();
    let english = client.translate(&qnc_text_translate::TranslateRequest { model: model.clone(), texts: texts.clone() })?;
    for (said, english) in texts.iter().zip(&english) {
        println!("{said}\n   {english}");
    }
    eprintln!("{} texts in {} ms", texts.len(), started.elapsed().as_millis());
    Ok(())
}
