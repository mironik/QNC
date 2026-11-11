//! A translation of a sentence (user 2026-11-11: "the OFF into English", so that SigLIP 2, taught
//! on English captions, compares the words of an OFF with the pictures and the captions of the
//! shots). A Marian model of OPUS-MT (Helsinki-NLP; safetensors, its `config.json`, the
//! SentencePiece model of its source language `source.spm` and the shared `vocab.json`) in pure
//! Rust (candle): the sentence is cut into the source model's pieces, encoded and decoded greedily
//! into the target language. It reads only its model's folder and knows no database, media or
//! form.

use std::{collections::HashMap, path::Path};

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::marian::{Config, MTModel};
use tokenizers::{
    models::unigram::Unigram,
    normalizers::unicode::NFKC,
    pre_tokenizers::metaspace::{Metaspace, PrependScheme},
    Tokenizer,
};

pub const MODULE_ID: &str = "qnc.module.translate";

/// The longest translation, in pieces (a sentence of an OFF is far shorter).
const MOST_PIECES: usize = 256;

pub struct Translator {
    model: MTModel,
    source: Tokenizer,
    ids: HashMap<String, u32>,
    pieces: Vec<String>,
    unknown: u32,
    start: u32,
    end: u32,
    device: Device,
}

impl Translator {
    /// The model in `folder`.
    pub fn load(folder: &Path) -> Result<Self, String> {
        let read = |name: &str| std::fs::read(folder.join(name)).map_err(|e| format!("{}: {e}", folder.join(name).display()));
        let config: Config = serde_json::from_slice(&read("config.json")?).map_err(err)?;
        let vocab: HashMap<String, u32> = serde_json::from_slice(&read("vocab.json")?).map_err(err)?;
        let mut pieces = vec![String::new(); vocab.values().max().map_or(0, |id| *id as usize + 1)];
        for (piece, id) in &vocab {
            pieces[*id as usize].clone_from(piece);
        }
        let unknown = *vocab.get("<unk>").ok_or("the vocabulary has no <unk>")?;
        let source = source_tokenizer(&read("source.spm")?)?;
        let device = Device::Cpu;
        // SAFETY: the model file is only read, never written while it is mapped.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[folder.join("model.safetensors")], DType::F32, &device) }.map_err(err)?;
        let model = MTModel::new(&config, vb).map_err(err)?;
        Ok(Self { model, source, ids: vocab, pieces, unknown, start: config.decoder_start_token_id, end: config.eos_token_id, device })
    }

    /// `text` in the model's target language (one sentence or a few; the decoding is greedy).
    pub fn translate(&mut self, text: &str) -> Result<String, String> {
        let encoding = self.source.encode(text, false).map_err(err)?;
        let mut input: Vec<u32> = encoding.get_tokens().iter().map(|piece| *self.ids.get(piece).unwrap_or(&self.unknown)).collect();
        if input.is_empty() {
            return Ok(String::new());
        }
        input.truncate(MOST_PIECES - 1);
        input.push(self.end);
        self.model.reset_kv_cache();
        let input = Tensor::new(input.as_slice(), &self.device).and_then(|t| t.unsqueeze(0)).map_err(err)?;
        let encoded = self.model.encoder().forward(&input, 0).map_err(err)?;
        let mut said = vec![self.start];
        for step in 0..MOST_PIECES {
            let context = if step == 0 { &said[..] } else { &said[said.len() - 1..] };
            let xs = Tensor::new(context, &self.device).and_then(|t| t.unsqueeze(0)).map_err(err)?;
            let logits = self.model.decode(&xs, &encoded, said.len() - context.len()).map_err(err)?;
            let last = logits.i((0, logits.dim(1).map_err(err)? - 1)).map_err(err)?;
            let mut last: Vec<f32> = last.to_vec1().map_err(err)?;
            // The start (the pad) is never said.
            if let Some(start) = last.get_mut(self.start as usize) {
                *start = f32::NEG_INFINITY;
            }
            let next = last.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(self.end, |(id, _)| id as u32);
            if next == self.end {
                break;
            }
            said.push(next);
        }
        let text: String = said[1..].iter().filter_map(|id| self.pieces.get(*id as usize)).filter(|piece| !piece.starts_with('<')).map(String::as_str).collect();
        Ok(text.replace('\u{2581}', " ").trim().to_string())
    }
}

/// The SentencePiece model of the source language as a tokenizer: its pieces and scores (a
/// unigram model) read from the protobuf of `source.spm`, the text normalised (NFKC) and split at
/// spaces marked as SentencePiece marks them.
fn source_tokenizer(spm: &[u8]) -> Result<Tokenizer, String> {
    let pieces = spm_pieces(spm)?;
    let unknown = pieces.iter().position(|(_, _, kind)| *kind == 2);
    let vocab: Vec<(String, f64)> = pieces.into_iter().map(|(piece, score, _)| (piece, f64::from(score))).collect();
    let model = Unigram::from(vocab, unknown, false).map_err(err)?;
    let mut tokenizer = Tokenizer::new(model);
    tokenizer.with_normalizer(Some(NFKC));
    tokenizer.with_pre_tokenizer(Some(Metaspace::new('\u{2581}', PrependScheme::Always, true)));
    Ok(tokenizer)
}

/// The pieces of a SentencePiece model (protobuf `ModelProto`, field 1): each piece, its score
/// and its type (1 normal, 2 unknown, 3 control, ...).
fn spm_pieces(bytes: &[u8]) -> Result<Vec<(String, f32, u64)>, String> {
    let mut pieces = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let (key, next) = varint(bytes, at)?;
        at = next;
        if key == (1 << 3 | 2) {
            let (field, end) = length_delimited(bytes, at)?;
            pieces.push(spm_piece(field)?);
            at = end;
        } else {
            at = skip(bytes, at, key & 7)?;
        }
    }
    if pieces.is_empty() {
        return Err("the SentencePiece model has no pieces".into());
    }
    Ok(pieces)
}

fn spm_piece(bytes: &[u8]) -> Result<(String, f32, u64), String> {
    let (mut piece, mut score, mut kind) = (String::new(), 0.0, 1);
    let mut at = 0;
    while at < bytes.len() {
        let (key, next) = varint(bytes, at)?;
        at = next;
        match key {
            0x0a => {
                let (field, end) = length_delimited(bytes, at)?;
                piece = String::from_utf8(field.to_vec()).map_err(err)?;
                at = end;
            }
            0x15 => {
                let raw: [u8; 4] = bytes.get(at..at + 4).and_then(|b| b.try_into().ok()).ok_or("a score cut short")?;
                score = f32::from_le_bytes(raw);
                at += 4;
            }
            0x18 => (kind, at) = varint(bytes, at)?,
            _ => at = skip(bytes, at, key & 7)?,
        }
    }
    Ok((piece, score, kind))
}

fn length_delimited(bytes: &[u8], at: usize) -> Result<(&[u8], usize), String> {
    let (length, start) = varint(bytes, at)?;
    let end = start.checked_add(usize::try_from(length).map_err(err)?).filter(|end| *end <= bytes.len()).ok_or("a field cut short")?;
    Ok((&bytes[start..end], end))
}

fn varint(bytes: &[u8], mut at: usize) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(at).ok_or("a number cut short")?;
        at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, at));
        }
    }
    Err("a number too long".into())
}

fn skip(bytes: &[u8], at: usize, wire: u64) -> Result<usize, String> {
    let end = match wire {
        0 => varint(bytes, at)?.1,
        1 => at + 8,
        2 => length_delimited(bytes, at)?.1,
        5 => at + 4,
        _ => return Err(format!("a field of wire type {wire}")),
    };
    (end <= bytes.len()).then_some(end).ok_or_else(|| "a field cut short".into())
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pieces_of_a_sentencepiece_model_are_read_from_its_protobuf() {
        // ModelProto { pieces: [ { piece: "<unk>", score: 0, type: 2 }, { piece: "▁luka", score: -3.5 } ] }
        let piece = |text: &str, score: f32, kind: Option<u8>| {
            let mut bytes = vec![0x0a, text.len() as u8];
            bytes.extend(text.as_bytes());
            bytes.push(0x15);
            bytes.extend(score.to_le_bytes());
            if let Some(kind) = kind {
                bytes.extend([0x18, kind]);
            }
            let mut field = vec![0x0a, bytes.len() as u8];
            field.extend(bytes);
            field
        };
        let mut model = piece("<unk>", 0.0, Some(2));
        model.extend(piece("\u{2581}luka", -3.5, None));
        model.extend([0x12, 0x00]); // another field, skipped
        assert_eq!(spm_pieces(&model).unwrap(), [("<unk>".to_string(), 0.0, 2), ("\u{2581}luka".to_string(), -3.5, 1)]);
        assert!(spm_pieces(&model[..5]).is_err(), "cut short");
    }
}
