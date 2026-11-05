//! The letters of a known text in its speech: the wav2vec2 model's letter probabilities
//! (`qnc-wav2vec2`) and the forced alignment of the text on them (`qnc-ctc-align`). Speech
//! longer than the model hears at once goes in pieces of [`PIECE_SECONDS`], each word laid on
//! the piece it lies in; the pieces overlap so a word on the edge of one is whole in the next.

use std::path::{Path, PathBuf};

use qnc_letter_align::{Letter, WordLetters};
use qnc_wav2vec2::{Wav2Vec2, FRAME_SECONDS, SAMPLE_RATE};

/// The longest piece of speech the model hears at once (its attention grows with the square).
pub const PIECE_SECONDS: f64 = 30.0;

#[derive(Default)]
pub struct Engine {
    model: Option<(PathBuf, Wav2Vec2)>,
}

impl Engine {
    fn model(&mut self, folder: &Path) -> Result<&Wav2Vec2, String> {
        if self.model.as_ref().map(|(loaded, _)| loaded.as_path()) != Some(folder) {
            self.model = Some((folder.to_path_buf(), Wav2Vec2::load(folder)?));
        }
        Ok(&self.model.as_ref().expect("just loaded").1)
    }

    /// The letters of `words` said in `samples` (16 kHz mono).
    pub fn align(&mut self, folder: &Path, samples: &[f32], words: &[String]) -> Result<Vec<WordLetters>, String> {
        let model = self.model(folder)?;
        if samples.len() as f64 > PIECE_SECONDS * f64::from(SAMPLE_RATE) {
            return Err(format!("more than {PIECE_SECONDS} s of speech at once"));
        }
        let emissions = model.emissions(samples)?;
        let blank = model.blank().ok_or("the model has no [PAD] blank")?;
        let space = model.tokens().iter().position(|token| token == " ");
        let texts: Vec<&str> = words.iter().map(String::as_str).collect();
        let laid = qnc_ctc_align::align(&emissions.frames, model.tokens(), blank, space, &texts).ok_or("the text does not fit on this speech")?;
        Ok(laid
            .into_iter()
            .map(|word| WordLetters {
                letters: word
                    .letters
                    .into_iter()
                    .map(|(letter, span)| Letter { letter, start: span.map(|(from, _)| from as f64 * FRAME_SECONDS), end: span.map(|(_, to)| to as f64 * FRAME_SECONDS) })
                    .collect(),
            })
            .collect())
    }

    pub fn hear(&mut self, folder: &Path, samples: &[f32]) -> Result<String, String> {
        let model = self.model(folder)?;
        Ok(model.greedy(&model.emissions(samples)?))
    }
}
