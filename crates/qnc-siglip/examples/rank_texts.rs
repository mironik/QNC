//! The captions most like each query (a check of how a model compares two texts, e.g. the
//! words of an OFF with the captions of shots): `rank_texts <model folder> <queries file>
//! <captions file>` (one text a line; a caption line may be "n ms | caption"). Reads only.

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder, queries, captions] = args.as_slice() else {
        return Err("rank_texts <model folder> <queries file> <captions file>".into());
    };
    let model = qnc_siglip::Siglip::load(std::path::Path::new(folder))?;
    let read = |file: &str| -> Result<Vec<String>, String> {
        Ok(std::fs::read_to_string(file).map_err(|e| e.to_string())?.lines().map(|line| line.split_once("| ").map_or(line, |(_, text)| text).trim_start_matches("In this image we can see ").to_string()).filter(|line| !line.trim().is_empty() && !line.starts_with("loaded")).collect())
    };
    let (queries, captions) = (read(queries)?, read(captions)?);
    let (asked, mut said) = (model.texts(&queries)?, model.texts(&captions)?);
    // Without the direction every caption shares (centred on the captions' mean, unit again).
    let centred = std::env::var("CENTRE").is_ok();
    let mean: Vec<f32> = (0..said[0].len()).map(|d| said.iter().map(|v| v[d]).sum::<f32>() / said.len() as f32).collect();
    let centre = |v: &Vec<f32>| { let c: Vec<f32> = v.iter().zip(&mean).map(|(a, m)| a - m).collect(); let n = c.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6); c.into_iter().map(|x| x / n).collect::<Vec<f32>>() };
    let asked: Vec<Vec<f32>> = if centred { asked.iter().map(centre).collect() } else { asked };
    if centred { said = said.iter().map(centre).collect(); }
    for (query, vector) in queries.iter().zip(&asked) {
        let mut like: Vec<(f32, usize)> = said.iter().enumerate().map(|(n, caption)| (caption.iter().zip(vector).map(|(a, b)| a * b).sum(), n)).collect();
        like.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("{query}");
        for (score, n) in like.iter().take(3) {
            println!("   {score:.3} {}", &captions[*n][..captions[*n].len().min(80)]);
        }
    }
    Ok(())
}
