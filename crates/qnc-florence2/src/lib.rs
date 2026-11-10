//! The Florence-2 model (user 2026-11-10: the second AI engine, it names what it sees without a
//! vocabulary). Microsoft's Florence-2 base (MIT) as its ONNX export: a vision encoder, the
//! token embeddings, an encoder and a decoder with its cache, run through the ONNX Runtime
//! library the caller names (the one beside the program). The detailed caption task, greedy
//! decoding without a repeated word or three-word run. It reads the model and the pictures it
//! is given; it writes nothing.

use std::path::Path;

use ort::session::{Session, SessionInputValue};
use ort::value::Tensor;

pub const MODULE_ID: &str = "qnc.module.florence2";

const LAYERS: usize = 6;
const HEADS: usize = 12;
const HEAD: usize = 64;
const WIDTH: usize = 768;
/// The side of the square picture the vision encoder takes.
const SIDE: u32 = 768;
/// The start, the first and the end token of the decoder.
const START: i64 = 2;
const FIRST: usize = 0;
const END: i64 = 2;
/// The longest caption, in tokens.
const LONGEST: usize = 100;
/// The detailed caption task as the model's processor words it.
const TASK: &str = "Describe in detail what is shown in the image.";
const KINDS: [&str; 4] = ["decoder.key", "decoder.value", "encoder.key", "encoder.value"];

/// A loaded model, kept for the next picture.
pub struct Florence {
    vision: Session,
    embed: Session,
    encoder: Session,
    decoder: Session,
    tokenizer: tokenizers::Tokenizer,
    task: Vec<i64>,
}

impl Florence {
    /// Loads the model of `folder` (its `onnx` files and `tokenizer.json`) with the ONNX Runtime
    /// library `runtime`, on `threads` threads.
    pub fn load(folder: &Path, runtime: &Path, threads: usize) -> Result<Self, String> {
        ort::init_from(runtime.to_string_lossy()).commit().map_err(err)?;
        let open = |name: &str| -> Result<Session, String> {
            Session::builder()
                .and_then(|builder| builder.with_intra_threads(threads))
                .and_then(|builder| builder.commit_from_file(folder.join("onnx").join(format!("{name}.onnx"))))
                .map_err(|error| format!("{name}: {error}"))
        };
        let tokenizer = tokenizers::Tokenizer::from_file(folder.join("tokenizer.json")).map_err(err)?;
        let task = tokenizer.encode(TASK, true).map_err(err)?.get_ids().iter().map(|&id| i64::from(id)).collect();
        Ok(Self { vision: open("vision_encoder")?, embed: open("embed_tokens")?, encoder: open("encoder_model")?, decoder: open("decoder_model_merged")?, tokenizer, task })
    }

    /// A few sentences of what `picture` shows (English).
    pub fn caption(&mut self, picture: &image::RgbImage) -> Result<String, String> {
        let picture = image::imageops::resize(picture, SIDE, SIDE, image::imageops::FilterType::CatmullRom);
        let (mean, std) = ([0.485f32, 0.456, 0.406], [0.229f32, 0.224, 0.225]);
        let mut pixels = Vec::with_capacity(3 * (SIDE * SIDE) as usize);
        for c in 0..3 {
            pixels.extend(picture.pixels().map(|p| (f32::from(p.0[c]) / 255.0 - mean[c]) / std[c]));
        }
        let side = SIDE as usize;
        let mut joined = {
            let out = self.vision.run(ort::inputs!["pixel_values" => Tensor::from_array(([1usize, 3, side, side], pixels)).map_err(err)?]).map_err(err)?;
            out["image_features"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec()
        };
        joined.extend(self.embed_ids(self.task.clone())?);
        let length = joined.len() / WIDTH;
        let mask = vec![1i64; length];
        let hidden = {
            let out = self
                .encoder
                .run(ort::inputs![
                    "inputs_embeds" => Tensor::from_array(([1usize, length, WIDTH], joined)).map_err(err)?,
                    "attention_mask" => Tensor::from_array(([1usize, length], mask.clone())).map_err(err)?
                ])
                .map_err(err)?;
            out["last_hidden_state"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec()
        };
        // The cache before the first step is not read (its branch is off); it only needs a shape.
        let mut cache: Vec<(Vec<usize>, Vec<f32>)> = (0..LAYERS * KINDS.len()).map(|_| (vec![1, HEADS, 1, HEAD], vec![0.0; HEADS * HEAD])).collect();
        let (mut last, mut said) = (START, Vec::<i64>::new());
        for step in 0..LONGEST {
            let mut inputs: Vec<(std::borrow::Cow<str>, SessionInputValue)> = vec![
                ("inputs_embeds".into(), Tensor::from_array(([1usize, 1, WIDTH], self.embed_ids(vec![last])?)).map_err(err)?.into()),
                ("encoder_hidden_states".into(), Tensor::from_array(([1usize, length, WIDTH], hidden.clone())).map_err(err)?.into()),
                ("encoder_attention_mask".into(), Tensor::from_array(([1usize, length], mask.clone())).map_err(err)?.into()),
                ("use_cache_branch".into(), Tensor::from_array(([1usize], vec![step > 0])).map_err(err)?.into()),
            ];
            for layer in 0..LAYERS {
                for (k, kind) in KINDS.iter().enumerate() {
                    let (shape, data) = cache[layer * KINDS.len() + k].clone();
                    inputs.push((format!("past_key_values.{layer}.{kind}").into(), Tensor::from_array((shape, data)).map_err(err)?.into()));
                }
            }
            let out = self.decoder.run(inputs).map_err(err)?;
            let (shape, logits) = out["logits"].try_extract_tensor::<f32>().map_err(err)?;
            let vocabulary = usize::try_from(shape[2]).map_err(err)?;
            let mut scores = logits[logits.len() - vocabulary..].to_vec();
            if step == 0 {
                scores.iter_mut().enumerate().filter(|(i, _)| *i != FIRST).for_each(|(_, s)| *s = f32::NEG_INFINITY);
            }
            forbid_repeats(&said, &mut scores);
            let next = scores.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(END, |(i, _)| i as i64);
            for layer in 0..LAYERS {
                for (k, kind) in KINDS.iter().enumerate() {
                    // The encoder's keys are made at the first step only.
                    if step > 0 && k >= 2 {
                        continue;
                    }
                    let (shape, data) = out[format!("present.{layer}.{kind}").as_str()].try_extract_tensor::<f32>().map_err(err)?;
                    cache[layer * KINDS.len() + k] = (shape.iter().map(|&d| d as usize).collect(), data.to_vec());
                }
            }
            if next == END {
                break;
            }
            said.push(next);
            last = next;
        }
        let ids: Vec<u32> = said.iter().filter_map(|&id| u32::try_from(id).ok()).collect();
        Ok(plain(&self.tokenizer.decode(&ids, true).map_err(err)?))
    }

    fn embed_ids(&mut self, ids: Vec<i64>) -> Result<Vec<f32>, String> {
        let count = ids.len();
        let out = self.embed.run(ort::inputs!["input_ids" => Tensor::from_array(([1usize, count], ids)).map_err(err)?]).map_err(err)?;
        Ok(out["inputs_embeds"].try_extract_tensor::<f32>().map_err(err)?.1.to_vec())
    }
}

/// No token said just before, no run of three said before (the model's own "can can").
fn forbid_repeats(said: &[i64], scores: &mut [f32]) {
    let mut forbid = |id: i64| {
        if let Some(score) = usize::try_from(id).ok().and_then(|id| scores.get_mut(id)) {
            *score = f32::NEG_INFINITY;
        }
    };
    if let Some(&last) = said.last() {
        forbid(last);
    }
    if let [.., a, b] = said {
        for run in said.windows(3) {
            if run[0] == *a && run[1] == *b {
                forbid(run[2]);
            }
        }
    }
}

/// The caption without the model's own opening ("In this image we can see").
fn plain(text: &str) -> String {
    let text = text.trim();
    for opening in ["In this image we can see ", "In this picture we can see ", "In this image, we can see "] {
        if let Some(rest) = text.strip_prefix(opening) {
            let mut chars = rest.chars();
            return chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default();
        }
    }
    text.to_string()
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_models_opening_is_left_out() {
        assert_eq!(plain("In this image we can see a swimming pool, trees."), "A swimming pool, trees.");
        assert_eq!(plain("A car on the road."), "A car on the road.");
    }

    #[test]
    fn a_token_just_said_and_a_run_of_three_said_before_are_not_said_again() {
        let mut scores = vec![0.0; 10];
        forbid_repeats(&[1, 2, 3, 1, 2], &mut scores);
        assert_eq!(scores[2], f32::NEG_INFINITY, "the last token");
        assert_eq!(scores[3], f32::NEG_INFINITY, "1 2 3 again");
        assert_eq!(scores[5], 0.0);
    }
}
