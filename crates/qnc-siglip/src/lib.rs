//! A SigLIP model in pure Rust (user 2026-11-10: what a picture of a clip shows, for sorting the
//! B-roll by its content, offline). The model folder holds `model.safetensors` and
//! `tokenizer.json` of a SigLIP checkpoint (google/siglip-base-patch16-224). A picture becomes a
//! vector of what it shows; a text becomes a vector in the same space, so the two can be
//! compared: how well "a photo of a hospital" describes a picture is the sigmoid of their scaled
//! cosine, as the model was trained. It reads the model and the pictures it is given and writes
//! nothing.

use std::path::Path;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::siglip;

/// The side of the square picture the model looks at.
pub const IMAGE_SIZE: u32 = 224;

pub struct Siglip {
    model: siglip::Model,
    tokenizer: tokenizers::Tokenizer,
    scale: f32,
    bias: f32,
    pad: u32,
    max_len: usize,
    device: Device,
}

impl Siglip {
    /// The model of `folder`.
    pub fn load(folder: &Path) -> Result<Self, String> {
        let device = Device::Cpu;
        let config = siglip::Config::base_patch16_224();
        let weights = folder.join("model.safetensors");
        // SAFETY: the weights file is only read and not changed while the model is loaded.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[&weights], DType::F32, &device) }.map_err(err)?;
        let model = siglip::Model::new(&config, vb.clone()).map_err(err)?;
        let scalar = |name: &str| -> Result<f32, String> { vb.get(&[1], name).and_then(|t| t.to_vec1::<f32>()).map(|v| v[0]).map_err(err) };
        let scale = scalar("logit_scale")?.exp();
        let bias = scalar("logit_bias")?;
        let tokenizer = tokenizers::Tokenizer::from_file(folder.join("tokenizer.json")).map_err(err)?;
        Ok(Self {
            model,
            tokenizer,
            scale,
            bias,
            pad: config.text_config.pad_token_id,
            max_len: config.text_config.max_position_embeddings,
            device,
        })
    }

    /// The vector of each picture (unit length), in the order given.
    pub fn pictures(&self, pictures: &[image::RgbImage]) -> Result<Vec<Vec<f32>>, String> {
        if pictures.is_empty() {
            return Ok(Vec::new());
        }
        let side = IMAGE_SIZE as usize;
        let mut data = Vec::with_capacity(pictures.len() * 3 * side * side);
        for picture in pictures {
            let square = image::imageops::resize(picture, IMAGE_SIZE, IMAGE_SIZE, image::imageops::FilterType::Triangle);
            // Channels first, each value from [0, 255] to [-1, 1] (mean 0.5, deviation 0.5).
            for channel in 0..3 {
                data.extend(square.pixels().map(|p| f32::from(p.0[channel]) / 127.5 - 1.0));
            }
        }
        let input = Tensor::from_vec(data, (pictures.len(), 3, side, side), &self.device).map_err(err)?;
        rows(&self.model.get_image_features(&input).map_err(err)?)
    }

    /// The vector of each text (unit length), in the order given.
    pub fn texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::with_capacity(texts.len() * self.max_len);
        for text in texts {
            let encoding = self.tokenizer.encode(text.as_str(), true).map_err(err)?;
            let mut row: Vec<u32> = encoding.get_ids().iter().copied().take(self.max_len).collect();
            row.resize(self.max_len, self.pad);
            ids.extend(row);
        }
        let input = Tensor::from_vec(ids, (texts.len(), self.max_len), &self.device).map_err(err)?;
        rows(&self.model.get_text_features(&input).map_err(err)?)
    }

    /// The scale and bias of the model's score: the sigmoid of `cosine * scale + bias`.
    pub fn scoring(&self) -> (f32, f32) {
        (self.scale, self.bias)
    }

    /// How well a text describes a picture, from 0 to 1, as the model was trained (both vectors
    /// of unit length).
    pub fn score(&self, picture: &[f32], text: &[f32]) -> f32 {
        let cosine: f32 = picture.iter().zip(text).map(|(a, b)| a * b).sum();
        1.0 / (1.0 + (-(cosine * self.scale + self.bias)).exp())
    }
}

/// The rows of a 2-D tensor, each made unit length.
fn rows(tensor: &Tensor) -> Result<Vec<Vec<f32>>, String> {
    let rows: Vec<Vec<f32>> = tensor.to_vec2::<f32>().map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let norm = row.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
            row.into_iter().map(|v| v / norm).collect()
        })
        .collect())
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
