//! The fine wave of a cut (user 2026-10-25: "with our tools it is very hard to cut the sound
//! exactly, to leave out a letter or a short word; zoom the audio so the boundary is seen").
//! A clip's stored wave has 1200 points over the whole clip (a quarter second each on a
//! five minute clip), too coarse for a word. This piece reads the sound of only the few
//! seconds around a cut from the camera original on this machine (`qnc-still-locator`),
//! through the decoder catalog, in the background, keeps the samples in memory and gives a
//! peak for every column of the window the trim zoom draws. It composes the pieces of sound
//! a lane plays there (a clip's own frames, or the segments and covers of a story), each at
//! its level, off silent. No probe, no database, nothing written; a passive aid.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{mpsc, Arc, Mutex},
};

use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder};
use qnc_media_metadata::Rational;
use qnc_program_waveform::Placement;
use qnc_still_locator::{LocatedSound, StillLocator};

pub const MODULE_ID: &str = "qnc.module.fine-wave";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Seconds read on each side of what a window needs, so the next arrow steps are in memory.
const MARGIN_SECONDS: f64 = 1.5;
/// Chunks of samples kept per clip channel (about 4 s mono each).
const KEPT_CHUNKS: usize = 6;

/// Samples of one camera channel of a clip from `from_seconds` (media timestamps).
#[derive(Debug, Clone)]
struct Chunk {
    from_seconds: f64,
    rate: u32,
    samples: Vec<f32>,
    /// The clip frames this chunk was read for: whatever the media gave there is all there
    /// is (a clip starts or ends inside them), so it is never asked again.
    frames: (u64, u64),
}

impl Chunk {
    fn to_seconds(&self) -> f64 {
        self.from_seconds + self.samples.len() as f64 / f64::from(self.rate.max(1))
    }

    fn covers(&self, from: f64, to: f64) -> bool {
        self.from_seconds <= from && to <= self.to_seconds()
    }

    /// Whether this chunk answers clip frames `[first, last)`: its samples cover them, or it
    /// was read for them (the media has no more sound there).
    fn answers(&self, first: u64, last: u64, seconds: impl Fn(f64) -> f64) -> bool {
        (self.frames.0 <= first && last <= self.frames.1) || self.covers(seconds(first as f64), seconds(last as f64))
    }
}

/// A read of `(clip, camera channel)` over clip frames `[from, to)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Ask {
    clip_id: String,
    channel: u16,
    from: u64,
    to: u64,
}

impl Ask {
    /// Whether this read is of the clip channel and covers clip frames `[first, last)`.
    fn within(&self, clip_id: &str, channel: u16, first: u64, last: u64) -> bool {
        self.clip_id == clip_id && self.channel == channel && self.from <= first && last <= self.to
    }
}

type Done = (Ask, Result<(Chunk, (f64, (u32, u32))), String>);

/// The fine waves of one form, kept between frames.
#[derive(Default)]
pub struct FineWave {
    project: Option<String>,
    chunks: HashMap<(String, u16), VecDeque<Chunk>>,
    /// Each clip's time of frame 0 and its frame rate, once its sound was located.
    timing: HashMap<String, (f64, (u32, u32))>,
    /// Reads queued or running (the worker takes the newest first).
    wanted: Arc<Mutex<Vec<Ask>>>,
    asked: HashSet<Ask>,
    /// Reads that failed: only their frames are not asked again (another part of the same
    /// clip channel still is).
    failed: Vec<Ask>,
    done: Option<mpsc::Receiver<Done>>,
    sender: Option<mpsc::Sender<Done>>,
    running: bool,
}

impl FineWave {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a read is still running or queued.
    pub fn loading(&self) -> bool {
        self.running || self.wanted.lock().map(|wanted| !wanted.is_empty()).unwrap_or(false)
    }

    /// The peak of every column of program frames `[from, to)` for the pieces of one lane
    /// (0 to 1, each piece at its level), and whether every piece's sound was there. A piece
    /// whose sound is not in memory yet is read in the background and is silent until then.
    pub fn lane_peaks(&mut self, locator: &StillLocator, pieces: &[Placement], from: u64, to: u64, columns: usize) -> (Vec<f32>, bool) {
        self.take_done(locator);
        let mut peaks = vec![0f32; columns];
        let mut complete = true;
        if to <= from || columns == 0 {
            return (peaks, complete);
        }
        for piece in pieces.iter().filter(|piece| !piece.off && piece.program_start < to && from < piece.program_end) {
            let length = piece.program_end - piece.program_start;
            let source_length = piece.source_out.saturating_sub(piece.source_in);
            // Program frame `p` of the piece is this clip frame (a source of another rate
            // counts its own frames).
            let source_at = |p: f64| piece.source_in as f64 + (p - piece.program_start as f64) * source_length as f64 / length.max(1) as f64;
            let (a, b) = (piece.program_start.max(from), piece.program_end.min(to));
            let (first, last) = (source_at(a as f64).floor() as u64, source_at(b as f64).ceil() as u64);
            let key = (piece.clip_id.clone(), piece.channel);
            let Some(&(zero, fps)) = self.timing.get(&piece.clip_id) else {
                complete = false;
                self.ask(locator, &piece.clip_id, piece.channel, first, last);
                continue;
            };
            let seconds = |frame: f64| zero + frame * f64::from(fps.1) / f64::from(fps.0.max(1));
            let chunk = self.chunks.get(&key).and_then(|chunks| chunks.iter().find(|chunk| chunk.answers(first, last, seconds)));
            let Some(chunk) = chunk.cloned() else {
                complete &= self.failed.iter().any(|ask| ask.within(&piece.clip_id, piece.channel, first, last));
                self.ask(locator, &piece.clip_id, piece.channel, first, last);
                continue;
            };
            let gain = 10f32.powf(f32::from(piece.level_db) / 20.0);
            let span = (to - from) as f64 / columns as f64;
            for (column, peak) in peaks.iter_mut().enumerate() {
                let (c0, c1) = (from as f64 + column as f64 * span, from as f64 + (column + 1) as f64 * span);
                let (p0, p1) = (c0.max(a as f64), c1.min(b as f64));
                if p1 <= p0 {
                    continue;
                }
                let sample = |p: f64| ((seconds(source_at(p)) - chunk.from_seconds) * f64::from(chunk.rate)).max(0.0) as usize;
                let (s0, s1) = (sample(p0).min(chunk.samples.len()), sample(p1).min(chunk.samples.len()));
                let loudest = chunk.samples[s0..s1.max(s0)].iter().fold(0f32, |most, value| most.max(value.abs()));
                *peak = peak.max((loudest * gain).min(1.0));
            }
        }
        (peaks, complete)
    }

    /// Queues a read of clip frames `[first, last)` with a margin, unless one covering it is
    /// asked already.
    fn ask(&mut self, locator: &StillLocator, clip_id: &str, channel: u16, first: u64, last: u64) {
        if self.project.as_deref() != Some(locator.project_id()) {
            *self = Self { project: Some(locator.project_id().to_string()), ..Self::default() };
        }
        let covered = self.asked.iter().chain(&self.failed).any(|ask| ask.within(clip_id, channel, first, last));
        if covered {
            return;
        }
        let margin = self.timing.get(clip_id).map_or(75, |(_, fps)| (MARGIN_SECONDS * f64::from(fps.0) / f64::from(fps.1.max(1))) as u64);
        let ask = Ask { clip_id: clip_id.to_string(), channel, from: first.saturating_sub(margin), to: last + margin };
        self.asked.insert(ask.clone());
        if let Ok(mut wanted) = self.wanted.lock() {
            wanted.push(ask);
        }
        self.start(locator);
    }

    /// The worker: one thread reading the newest wanted read, then the next, until none.
    fn start(&mut self, locator: &StillLocator) {
        if self.running {
            return;
        }
        let (sender, receiver) = match self.sender.clone() {
            Some(sender) => (sender, None),
            None => {
                let (sender, receiver) = mpsc::channel();
                (sender, Some(receiver))
            }
        };
        if let Some(receiver) = receiver {
            self.done = Some(receiver);
            self.sender = Some(sender.clone());
        }
        let (wanted, locator) = (self.wanted.clone(), locator.clone());
        self.running = true;
        let started = std::thread::Builder::new().name("qnc-fine-wave".into()).spawn(move || {
            let mut located: HashMap<String, Result<LocatedSound, String>> = HashMap::new();
            loop {
                let Some(ask) = wanted.lock().ok().and_then(|mut wanted| wanted.pop()) else { break };
                let place = located.entry(ask.clip_id.clone()).or_insert_with(|| locator.locate_sound(&ask.clip_id)).clone();
                let started = std::time::Instant::now();
                let result = place.and_then(|place| read(&place, &ask).map(|chunk| (chunk, (place.frame_zero_seconds, place.fps))));
                qnc_dev_diagnostics::log_line(
                    qnc_dev_diagnostics::DiagnosticsStream::Player,
                    format!("fine-wave read {} ch{} frames {}..{} ms={} ok={}", ask.clip_id, ask.channel + 1, ask.from, ask.to, started.elapsed().as_millis(), result.is_ok()),
                );
                if sender.send((ask, result)).is_err() {
                    break;
                }
            }
            let _ = sender.send((Ask { clip_id: String::new(), channel: 0, from: 0, to: 0 }, Err(String::new())));
        });
        if started.is_err() {
            self.running = false;
        }
    }

    /// Takes the reads that finished.
    fn take_done(&mut self, locator: &StillLocator) {
        if self.project.as_deref() != Some(locator.project_id()) {
            *self = Self { project: Some(locator.project_id().to_string()), ..Self::default() };
            return;
        }
        let Some(done) = &self.done else { return };
        let finished: Vec<Done> = done.try_iter().collect();
        for (ask, result) in finished {
            if ask.clip_id.is_empty() {
                self.running = false; // the worker ended
                continue;
            }
            match result {
                Ok((chunk, timing)) => {
                    self.timing.insert(ask.clip_id.clone(), timing);
                    let chunks = self.chunks.entry((ask.clip_id.clone(), ask.channel)).or_default();
                    chunks.push_back(chunk);
                    while chunks.len() > KEPT_CHUNKS {
                        chunks.pop_front();
                    }
                }
                Err(error) => {
                    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("fine-wave {} ch{}: {error}", ask.clip_id, ask.channel + 1));
                    self.failed.push(ask.clone());
                }
            }
            self.asked.remove(&ask);
        }
        if self.loading() && !self.running {
            self.start(locator);
        }
    }
}

/// The samples of the asked camera channel over clip frames `[from, to)`, from the camera
/// original through the decoder catalog.
fn read(place: &LocatedSound, ask: &Ask) -> Result<Chunk, String> {
    let (stream_index, inside, width, rate) = place.channel(ask.channel).ok_or_else(|| format!("no camera channel {}", ask.channel + 1))?;
    let config = qnc_decoder_catalog::installed_config().map_err(|error| error.to_string())?;
    // The decoder starts at the time of the first frame from the media's start (as the
    // trim pictures and the player seek).
    let start = (ask.from > 0).then(|| Rational { numerator: (ask.from * u64::from(place.fps.1)) as i64, denominator: i64::from(place.fps.0.max(1)) });
    let request = DecodeRequest {
        version: qnc_media_decode::VERSION.into(),
        media: place.media.clone(),
        stream_index,
        start,
        output_pixel_format: None,
        progressive: false,
        delivered_rate: None,
    };
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(&place.file, &place.media.media_uri).map_err(|error| error.to_string())?;
    let mut decoder = Decoder::open_endpoint(request, endpoint, place.stamp.clone(), config).map_err(|error| error.to_string())?;
    let (want_from, want_to) = (place.seconds_of(ask.from as f64), place.seconds_of(ask.to as f64));
    let mut chunk: Option<Chunk> = None;
    loop {
        let packet = match decoder.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            // Past the end of the clip the decoder refuses: the sound read so far is all
            // there is (an OUT near the clip's end).
            Err(_) if chunk.is_some() => break,
            Err(error) => return Err(error.to_string()),
        };
        let DecodedFormat::Audio { sample_rate_hz, channels, .. } = &packet.format else { continue };
        if *channels != u32::from(width) || *sample_rate_hz != rate {
            return Err("the decoded sound is not the stored format".into());
        }
        let at = packet.pts as f64 * packet.time_base.numerator as f64 / packet.time_base.denominator.max(1) as f64;
        if at > want_to {
            break;
        }
        let samples = packet
            .bytes
            .chunks_exact(usize::from(width) * 4)
            .map(|frame| f32::from_le_bytes(frame[usize::from(inside) * 4..usize::from(inside) * 4 + 4].try_into().expect("sample")));
        match &mut chunk {
            Some(chunk) => chunk.samples.extend(samples),
            None => chunk = Some(Chunk { from_seconds: at, rate, samples: samples.collect(), frames: (ask.from, ask.to) }),
        }
        if chunk.as_ref().is_some_and(|chunk| chunk.to_seconds() >= want_to) {
            break;
        }
    }
    let chunk = chunk.ok_or("no sound in the asked part")?;
    if chunk.from_seconds > want_from + 0.05 {
        // The decoder started after the asked part (the start of the media): still drawn.
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("fine-wave {} starts {:.3} s late", ask.clip_id, chunk.from_seconds - want_from));
    }
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunk_covers_the_seconds_of_its_samples() {
        let chunk = Chunk { from_seconds: 10.0, rate: 48_000, samples: vec![0.0; 96_000], frames: (500, 600) };
        assert!(chunk.covers(10.5, 11.9));
        assert!(!chunk.covers(9.9, 11.0));
        assert!(!chunk.covers(10.0, 12.1));
    }

    #[test]
    fn a_chunk_read_for_frames_answers_them_even_where_the_media_has_no_sound() {
        // The end of a clip: the read stopped at the end of the media, short of the asked part.
        let chunk = Chunk { from_seconds: 10.0, rate: 48_000, samples: vec![0.0; 48_000], frames: (500, 600) };
        let seconds = |frame: f64| frame / 50.0;
        assert!(chunk.answers(520, 590, seconds), "asked once, never again");
        assert!(!chunk.answers(590, 610, seconds), "frames past the read are asked");
    }
}
