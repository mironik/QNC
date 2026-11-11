//! The in-between pictures of the smooth cuts of a script's statements (user 2026-11-08:
//! "joining a composite statement with AI generated in-between pictures"). For every join that
//! asks for a smooth cut and has no clip of its pictures as it is now, off the form thread and
//! only while the player is idle: the picture before and the picture after are decoded from the
//! camera originals through the decoder catalog (no probe; the card is only read), the model of
//! this host (`qnc-frame-interp`) makes the pictures between them, and they are written as a
//! short clip in the voice folder of the project, a clip of the catalog QNC wrote itself
//! (`qnc-voice-clip`, `qnc-voice-catalog`: the pool does not show it). The script then gets the
//! clip's id. The database is the truth; what is held here is the job and the joins that failed
//! in this run.

use std::{
    collections::HashSet,
    path::Path,
    sync::{atomic::AtomicBool, mpsc},
};

use qnc_script_db::Block;
use qnc_still_locator::{LocatedSound, StillLocator};

pub const MODULE_ID: &str = "qnc.module.smooth-joins";

/// A join whose in-between pictures are to be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub block_id: i64,
    pub index: usize,
    pub clip_id: String,
    pub before_clip: String,
    pub after_clip: String,
    pub out_last: u64,
    pub in_first: u64,
    /// The pictures around the cut that give way to in-between ones.
    pub n: u64,
    /// The frames of the pause at the join, each one more in-between picture.
    pub pause: u64,
}

/// The joins of `blocks` that ask for a smooth cut and do not have its clip as they are now.
pub fn wanted(blocks: &[Block]) -> Vec<Wanted> {
    let mut joins = Vec::new();
    for block in blocks {
        for (index, pair) in block.pieces.windows(2).enumerate() {
            let (before, after) = (&pair[0], &pair[1]);
            if let Some((clip_id, _)) = qnc_script_draft::smooth_of(before, after).filter(|(id, _)| *id != after.smooth_clip) {
                joins.push(Wanted { block_id: block.id, index: index + 1, clip_id, before_clip: before.clip_id.clone(), after_clip: after.clip_id.clone(), out_last: before.to_frame - 1, in_first: after.from_frame, n: after.smooth_frames, pause: after.gap_before });
            }
        }
    }
    joins
}

#[derive(Default)]
pub struct SmoothJoins {
    project_id: Option<String>,
    job: Option<(Wanted, mpsc::Receiver<Result<(), String>>)>,
    /// Joins that failed in this run: not asked again (they would be asked every poll).
    failed: HashSet<String>,
}

impl SmoothJoins {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the pictures of a join are being made.
    pub fn working(&self) -> bool {
        self.job.is_some()
    }

    /// Starts the next join of `joins` while the player is `idle`; true when one was finished
    /// (the caller reads the script again).
    pub fn poll(&mut self, locator: &StillLocator, joins: &[Wanted], idle: bool) -> bool {
        if self.project_id.as_deref() != Some(locator.project_id()) {
            *self = Self { project_id: Some(locator.project_id().to_string()), ..Self::default() };
        }
        let finished = self.take_finished();
        if self.job.is_none() && idle && locator.settings().ai_feature(qnc_ai_features::SMOOTH_CUT).unwrap_or(false) {
            if let Some(join) = joins.iter().find(|join| !self.failed.contains(&join.clip_id)) {
                let (send, done) = mpsc::channel();
                let (locator, work) = (locator.clone(), join.clone());
                let spawned = std::thread::Builder::new().name("qnc-smooth-joins".into()).spawn(move || _ = send.send(make(&locator, &work)));
                if spawned.is_ok() {
                    self.job = Some((join.clone(), done));
                }
            }
        }
        finished
    }

    fn take_finished(&mut self) -> bool {
        let Some((join, done)) = &self.job else { return false };
        match done.try_recv() {
            Ok(Ok(())) => {
                log(&format!("smooth-joins {} made", join.clip_id));
                self.job = None;
                true
            }
            Ok(Err(error)) => {
                log(&format!("smooth-joins {} failed: {error}", join.clip_id));
                self.failed.insert(join.clip_id.clone());
                self.job = None;
                false
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.job = None;
                false
            }
            Err(mpsc::TryRecvError::Empty) => false,
        }
    }
}

/// One progressive picture rate of a located original, or why a smooth cut cannot use it.
fn progressive_rate(place: &LocatedSound) -> Result<(u32, u32), String> {
    use qnc_media_metadata::{ScanMode, StreamDetails};
    let video = place.media.streams.iter().find_map(|stream| match &stream.details {
        StreamDetails::Video(video) => Some(video),
        _ => None,
    });
    match video.and_then(|video| video.scan_mode.as_ref()).map(|scan| scan.value) {
        Some(ScanMode::Progressive) => Ok(place.fps),
        _ => Err(qnc_ui_text::text("smooth.progressive_only").into()),
    }
}

fn picture(extractor: &qnc_decoder_catalog::LocalFilmstripExtractor, place: &LocatedSound, frame: u64, size: [u32; 2], out_dir: &Path) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let seek = frame as f64 * f64::from(place.fps.1) / f64::from(place.fps.0.max(1));
    extractor.extract_run_with_cancel(&place.file, seek, 1, size, out_dir, &AtomicBool::new(false))?;
    let made = out_dir.join("000.jpg");
    if made.is_file() {
        Ok(made)
    } else {
        Err(format!("{}: {}", place.file.display(), qnc_ui_text::text("smooth.no_picture")))
    }
}

/// The clip of the join's in-between pictures, in the project and its catalog; the script is
/// told last.
fn make(locator: &StillLocator, join: &Wanted) -> Result<(), String> {
    let settings = locator.settings();
    let (before, after) = (locator.locate_sound(&join.before_clip)?, locator.locate_sound(&join.after_clip)?);
    let rate = progressive_rate(&before)?;
    if progressive_rate(&after)? != rate {
        return Err(qnc_ui_text::text("smooth.same_rate_only").into());
    }
    let fps = f64::from(rate.0) / f64::from(rate.1.max(1));
    let (width, height, _) = settings.video_picture().map_err(|e| e.message)?;
    let sample_rate = settings.audio_sample_rate().map_err(|e| e.message)?;
    let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
    let (folder, base) = settings.voice_location(&project_dir).map_err(|e| e.message)?;
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;

    let span = qnc_frame_interp::span(join.out_last, join.in_first, join.n);
    let work = std::env::temp_dir().join(format!("qnc-smooth-{}-{}", std::process::id(), join.clip_id));
    let _ = std::fs::remove_dir_all(&work);
    let result = (|| {
        let extractor = qnc_decoder_catalog::installed_filmstrip_extractor().map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("smooth.no_decoder").to_string())?;
        let from = picture(&extractor, &before, span.from_picture, [width, height], &work.join("from"))?;
        let to = picture(&extractor, &after, span.to_picture, [width, height], &work.join("to"))?;
        let engine = qnc_ai_catalog::installed_frame_interpolator()?;
        let interpolator = qnc_frame_interp::Interpolator { program: qnc_plugin_process::beside_application(&engine.program)?, model: engine.model };
        let pictures = work.join("pictures");
        let count = join.n + join.pause;
        interpolator.between(&from, &to, count, &pictures)?;
        // The clip has one silent channel, as every clip the player reads has a sound; the cut's
        // own sound stays that of its parts.
        let silence = work.join("silence.wav");
        let seconds = count as f64 / fps;
        qnc_transcribe_jobs::write_wav(&silence, &vec![0.0; (seconds * 16_000.0).ceil() as usize + 1])?;
        let sounds = [silence];
        let out = folder.join(format!("{}.mxf", join.clip_id));
        let pattern = pictures.join("inbetween_%02d.png");
        let clip = qnc_voice_clip::VoiceClip { sounds: &sounds, seconds, text: "", heading: "", sequence: Some(&pattern), width, height, fps, fields: None, sample_rate, timecode: "00:00:00:00", out: &out, project_dir: &project_dir };
        qnc_voice_clip::make(&clip)?;
        let uri = qnc_voice_take::canonical(&format!("{base}/{}.mxf", join.clip_id)).map_err(|e| e.to_string())?;
        let record = qnc_voice_clip::describe(&clip, &join.clip_id, &uri)?;
        qnc_voice_catalog::publish_record(locator.reader(), settings, &join.clip_id, record)
    })();
    let _ = std::fs::remove_dir_all(&work);
    result?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(locator.reader(), settings)?;
    qnc_script_db::set_smooth_clip(&target, join.block_id, join.index, join.clip_id.clone()).map_err(|e| e.to_string())
}

fn log(line: &str) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_script_db::Piece;

    fn piece(clip: &str, from: u64, to: u64) -> Piece {
        let block = Block::written("sot", "");
        Piece { clip_id: clip.into(), from_frame: from, to_frame: to, ..Piece::of(&block) }
    }

    fn block(pieces: Vec<Piece>) -> Block {
        let mut block = Block::written("sot", "");
        block.pieces = pieces;
        block
    }

    #[test]
    fn a_join_is_wanted_until_its_clip_is_the_one_of_the_join_as_it_is() {
        let mut after = piece("b", 500, 600);
        after.smooth_frames = 8;
        let before = piece("a", 100, 200);
        let joins = wanted(&[block(vec![before.clone(), after.clone()])]);
        assert_eq!((joins.len(), joins[0].index, joins[0].out_last, joins[0].in_first, joins[0].n), (1, 1, 199, 500, 8));
        after.smooth_clip = joins[0].clip_id.clone();
        assert!(wanted(&[block(vec![before.clone(), after.clone()])]).is_empty());
        // The part before trimmed: another cut, another clip.
        assert_eq!(wanted(&[block(vec![piece("a", 100, 190), after])]).len(), 1);
    }

    #[test]
    fn a_pause_at_the_join_is_more_in_between_pictures_and_a_part_too_short_has_none() {
        let before = piece("a", 100, 200);
        let mut paused = piece("b", 500, 600);
        (paused.smooth_frames, paused.gap_before) = (8, 5);
        let joins = wanted(&[block(vec![before.clone(), paused.clone()])]);
        assert_eq!((joins[0].n, joins[0].pause), (8, 5));
        assert_eq!(qnc_script_draft::smooth_of(&before, &paused).unwrap().1, 13, "one for every frame of the pause");
        let mut longer = paused.clone();
        longer.gap_before = 6;
        assert_ne!(wanted(&[block(vec![before.clone(), longer])])[0].clip_id, joins[0].clip_id, "another pause, another clip");
        let mut short = piece("b", 500, 503);
        short.smooth_frames = 8;
        assert!(wanted(&[block(vec![before, short])]).is_empty());
    }
}
