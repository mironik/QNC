//! A wav2vec2 CTC model in pure Rust (user 2026-11-05: "cisti Rust bez drugih jezika"): the
//! letters of speech every 20 ms, for the fine times of the letters of a statement (the
//! Croatian ParlaSpeech-HR model of CLASSLA). The weights are read from the model's own
//! `model.safetensors`; the layers are those of `Wav2Vec2ForCTC` with the stable layer norm
//! (XLS-R): seven convolutions, a feature projection, a positional convolution, the transformer
//! layers and the head of the letters. Nothing is written; the model never leaves this process.

use std::path::Path;

use candle_core::{DType, Device, Module, Tensor, D};
use candle_nn::{Conv1d, Conv1dConfig, LayerNorm, Linear, VarBuilder};

pub const MODULE_ID: &str = "qnc.module.wav2vec2";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The rate the model hears.
pub const SAMPLE_RATE: u32 = 16_000;
/// One frame of the model's letters: 320 samples at 16 kHz.
pub const FRAME_SECONDS: f64 = 0.02;

/// The log probabilities of the model's letters, frame by frame.
#[derive(Debug, Clone)]
pub struct Emissions {
    /// `frames[t][token]`, natural log.
    pub frames: Vec<Vec<f32>>,
}

#[derive(Debug, Clone)]
struct Config {
    hidden: usize,
    heads: usize,
    layers: usize,
    eps: f64,
    conv_dim: Vec<usize>,
    conv_kernel: Vec<usize>,
    conv_stride: Vec<usize>,
    pos_kernel: usize,
    pos_groups: usize,
    vocab: usize,
}

impl Config {
    fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        let number = |key: &str| json[key].as_u64().map(|value| value as usize).ok_or_else(|| format!("config.json: no {key}"));
        let list = |key: &str| -> Result<Vec<usize>, String> {
            json[key].as_array().ok_or_else(|| format!("config.json: no {key}"))?.iter().map(|value| value.as_u64().map(|value| value as usize).ok_or_else(|| format!("config.json: {key}"))).collect()
        };
        if json["do_stable_layer_norm"].as_bool() != Some(true) || json["feat_extract_norm"].as_str() != Some("layer") {
            return Err("config.json: only the stable layer norm wav2vec2 (XLS-R) is read".into());
        }
        Ok(Self {
            hidden: number("hidden_size")?,
            heads: number("num_attention_heads")?,
            layers: number("num_hidden_layers")?,
            eps: json["layer_norm_eps"].as_f64().ok_or("config.json: no layer_norm_eps")?,
            conv_dim: list("conv_dim")?,
            conv_kernel: list("conv_kernel")?,
            conv_stride: list("conv_stride")?,
            pos_kernel: number("num_conv_pos_embeddings")?,
            pos_groups: number("num_conv_pos_embedding_groups")?,
            vocab: number("vocab_size")?,
        })
    }
}

struct ConvLayer {
    conv: Conv1d,
    norm: LayerNorm,
}

struct Layer {
    norm: LayerNorm,
    q: Linear,
    k: Linear,
    v: Linear,
    out: Linear,
    final_norm: LayerNorm,
    up: Linear,
    down: Linear,
}

pub struct Wav2Vec2 {
    config: Config,
    convs: Vec<ConvLayer>,
    projection_norm: LayerNorm,
    projection: Linear,
    pos_weight: Tensor,
    pos_bias: Tensor,
    layers: Vec<Layer>,
    encoder_norm: LayerNorm,
    head: Linear,
    /// The letters of the tokens (`vocab.json`), by id.
    tokens: Vec<String>,
    device: Device,
}

fn error(error: candle_core::Error) -> String {
    error.to_string()
}

impl Wav2Vec2 {
    /// The model of a folder with `config.json`, `vocab.json` and `model.safetensors`.
    pub fn load(folder: &Path) -> Result<Self, String> {
        let config = Config::read(&folder.join("config.json"))?;
        let tokens = read_vocab(&folder.join("vocab.json"), config.vocab)?;
        let device = Device::Cpu;
        let weights = folder.join("model.safetensors");
        // The file is only read (memory mapped); nothing changes it while the model lives.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[&weights], DType::F32, &device) }.map_err(error)?;
        let model = vb.pp("wav2vec2");
        let mut convs = Vec::new();
        let mut channels = 1;
        for (index, ((&dim, &kernel), &stride)) in config.conv_dim.iter().zip(&config.conv_kernel).zip(&config.conv_stride).enumerate() {
            let layer = model.pp("feature_extractor.conv_layers").pp(index.to_string());
            let conv = candle_nn::conv1d(channels, dim, kernel, Conv1dConfig { stride, ..Default::default() }, layer.pp("conv")).map_err(error)?;
            let norm = candle_nn::layer_norm(dim, config.eps, layer.pp("layer_norm")).map_err(error)?;
            convs.push(ConvLayer { conv, norm });
            channels = dim;
        }
        let projection_norm = candle_nn::layer_norm(channels, config.eps, model.pp("feature_projection.layer_norm")).map_err(error)?;
        let projection = candle_nn::linear(channels, config.hidden, model.pp("feature_projection.projection")).map_err(error)?;
        let pos = model.pp("encoder.pos_conv_embed.conv");
        let pos_weight = pos_conv_weight(&pos, &config)?;
        let pos_bias = pos.get(config.hidden, "bias").map_err(error)?;
        let mut layers = Vec::new();
        for index in 0..config.layers {
            let layer = model.pp("encoder.layers").pp(index.to_string());
            let (hidden, inner) = (config.hidden, config.hidden * 4);
            let linear = |name: &str, input: usize, output: usize| candle_nn::linear(input, output, layer.pp(name)).map_err(error);
            layers.push(Layer {
                norm: candle_nn::layer_norm(hidden, config.eps, layer.pp("layer_norm")).map_err(error)?,
                q: linear("attention.q_proj", hidden, hidden)?,
                k: linear("attention.k_proj", hidden, hidden)?,
                v: linear("attention.v_proj", hidden, hidden)?,
                out: linear("attention.out_proj", hidden, hidden)?,
                final_norm: candle_nn::layer_norm(hidden, config.eps, layer.pp("final_layer_norm")).map_err(error)?,
                up: linear("feed_forward.intermediate_dense", hidden, inner)?,
                down: linear("feed_forward.output_dense", inner, hidden)?,
            });
        }
        let encoder_norm = candle_nn::layer_norm(config.hidden, config.eps, model.pp("encoder.layer_norm")).map_err(error)?;
        let head = candle_nn::linear(config.hidden, config.vocab, vb.pp("lm_head")).map_err(error)?;
        Ok(Self { config, convs, projection_norm, projection, pos_weight, pos_bias, layers, encoder_norm, head, tokens, device })
    }

    /// The letter of every token id (empty for an id `vocab.json` has not).
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// The log probabilities of the letters of 16 kHz mono speech, a frame every 20 ms.
    pub fn emissions(&self, samples: &[f32]) -> Result<Emissions, String> {
        if samples.len() < 400 {
            return Err("too little speech for the model".into());
        }
        // The model hears speech of zero mean and unit variance (its feature extractor's
        // normalisation).
        let mean = samples.iter().map(|&value| f64::from(value)).sum::<f64>() / samples.len() as f64;
        let variance = samples.iter().map(|&value| (f64::from(value) - mean).powi(2)).sum::<f64>() / samples.len() as f64;
        let scale = 1.0 / (variance + 1e-7).sqrt();
        let normal: Vec<f32> = samples.iter().map(|&value| ((f64::from(value) - mean) * scale) as f32).collect();
        let logits = self.logits(&normal).map_err(error)?;
        let logits = candle_nn::ops::log_softmax(&logits, D::Minus1).map_err(error)?;
        Ok(Emissions { frames: logits.to_vec2::<f32>().map_err(error)? })
    }

    fn logits(&self, samples: &[f32]) -> candle_core::Result<Tensor> {
        let mut x = Tensor::from_slice(samples, (1, 1, samples.len()), &self.device)?;
        for layer in &self.convs {
            x = layer.conv.forward(&x)?;
            x = layer.norm.forward(&x.transpose(1, 2)?)?.transpose(1, 2)?.gelu_erf()?;
        }
        // [1, channels, time] -> [1, time, channels]
        let x = self.projection_norm.forward(&x.transpose(1, 2)?.contiguous()?)?;
        let mut h = self.projection.forward(&x)?;
        // The positional convolution: on the time axis, its last frame dropped (an even kernel).
        let time = h.dim(1)?;
        let pos = h.transpose(1, 2)?.contiguous()?.conv1d(&self.pos_weight, self.config.pos_kernel / 2, 1, 1, self.config.pos_groups)?;
        let pos = pos.broadcast_add(&self.pos_bias.reshape((1, self.config.hidden, 1))?)?.narrow(2, 0, time)?.gelu_erf()?;
        h = (h + pos.transpose(1, 2)?)?;
        let heads = self.config.heads;
        let head_dim = self.config.hidden / heads;
        let scaling = (head_dim as f64).powf(-0.5);
        for layer in &self.layers {
            let residual = h.clone();
            let x = layer.norm.forward(&h)?;
            let split = |t: Tensor| -> candle_core::Result<Tensor> { t.reshape((1, time, heads, head_dim))?.transpose(1, 2)?.contiguous() };
            let q = split((layer.q.forward(&x)? * scaling)?)?;
            let k = split(layer.k.forward(&x)?)?;
            let v = split(layer.v.forward(&x)?)?;
            let weights = candle_nn::ops::softmax_last_dim(&q.matmul(&k.t()?)?)?;
            let attended = weights.matmul(&v)?.transpose(1, 2)?.contiguous()?.reshape((1, time, self.config.hidden))?;
            h = (residual + layer.out.forward(&attended)?)?;
            let x = layer.final_norm.forward(&h)?;
            let x = layer.down.forward(&layer.up.forward(&x)?.gelu_erf()?)?;
            h = (h + x)?;
        }
        let h = self.encoder_norm.forward(&h)?;
        self.head.forward(&h)?.squeeze(0)
    }

    /// What the model hears, letter by letter (the best letter of every frame, repeats and
    /// blanks dropped): the proof the model is read right.
    pub fn greedy(&self, emissions: &Emissions) -> String {
        let blank = self.blank();
        let mut text = String::new();
        let mut last = None;
        for frame in &emissions.frames {
            let best = frame.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(id, _)| id);
            if best != last {
                if let Some(id) = best.filter(|&id| Some(id) != blank) {
                    text.push_str(self.tokens.get(id).map_or("", String::as_str));
                }
            }
            last = best;
        }
        text
    }

    /// The CTC blank (`[PAD]`).
    pub fn blank(&self) -> Option<usize> {
        self.tokens.iter().position(|token| token == "[PAD]")
    }
}

/// The positional convolution's weight, its weight norm applied (`weight_g`, `weight_v`, as an
/// older or newer save of the model names them).
fn pos_conv_weight(pos: &VarBuilder, config: &Config) -> Result<Tensor, String> {
    let shape_v = (config.hidden, config.hidden / config.pos_groups, config.pos_kernel);
    let shape_g = (1, 1, config.pos_kernel);
    let (g, v) = if pos.contains_tensor("weight_g") {
        (pos.get(shape_g, "weight_g").map_err(error)?, pos.get(shape_v, "weight_v").map_err(error)?)
    } else {
        let parts = pos.pp("parametrizations.weight");
        (parts.get(shape_g, "original0").map_err(error)?, parts.get(shape_v, "original1").map_err(error)?)
    };
    // weight = g * v / |v|, the norm over all but the kernel axis.
    let norm = v.sqr().and_then(|s| s.sum_keepdim(0)).and_then(|s| s.sum_keepdim(1)).and_then(|s| s.sqrt()).map_err(error)?;
    v.broadcast_mul(&g.broadcast_div(&norm).map_err(error)?).map_err(error)
}

fn read_vocab(path: &Path, size: usize) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let json: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut tokens = vec![String::new(); size];
    for (letter, id) in json {
        let id = id.as_u64().ok_or("vocab.json: an id is not a number")? as usize;
        *tokens.get_mut(id).ok_or("vocab.json: an id past the model's letters")? = letter;
    }
    Ok(tokens)
}
