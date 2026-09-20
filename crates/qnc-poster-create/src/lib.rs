//! A poster for a clip that has none.
//!
//! The card gives a poster when it has one. When it does not, the background application
//! makes one from a key frame of the clip itself, with the same extractor and the same way
//! the filmstrip frames are made, only at poster resolution. It reads the media, writes
//! one picture file and touches nothing else. The frame is chosen from what the project
//! database already knows (the length of the clip), never by probing the media.

use qnc_decoder_catalog::{FilmstripExtractFrame, FilmstripExtractMode};
use qnc_frame_timebase::FrameTimebase;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub const MODULE_ID: &str = "qnc.module.poster-create";
pub const VERSION: &str = "0.1.0";

/// The resolution of a poster, the same as the pictures a camera card keeps.
pub const POSTER_SIZE: [u32; 2] = [512, 288];

/// Creates the poster at `output` from the key frame near `seek_sec` of the local file
/// `source`. `Ok(false)` when this machine has no local extractor (nothing is created).
pub fn create_poster(
    source: &Path,
    seek_sec: f64,
    timebase: FrameTimebase,
    output: &Path,
    cancel: &AtomicBool,
) -> Result<bool, String> {
    let Some(extractor) =
        qnc_decoder_catalog::installed_filmstrip_extractor().map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };
    let scratch = scratch_dir();
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let result = (|| {
        // The extractor makes filmstrips, which have at least two frames; the first one
        // is the poster.
        let frames = [
            FilmstripExtractFrame { seek_sec },
            FilmstripExtractFrame { seek_sec },
        ];
        extractor.extract_frames_with_cancel(
            source,
            &frames,
            timebase,
            FilmstripExtractMode::KeyframeSeek,
            POSTER_SIZE,
            &scratch,
            cancel,
        )?;
        let made = scratch.join("000.jpg");
        if !made.is_file() {
            return Err("poster frame was not made".to_string());
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(&made, output).map_err(|e| e.to_string())?;
        Ok(true)
    })();
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn scratch_dir() -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("qnc-poster-{}-{unique}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poster_has_the_resolution_of_a_card_poster() {
        assert_eq!(POSTER_SIZE, [512, 288]);
    }

    #[test]
    fn a_missing_source_makes_no_poster_and_leaves_nothing_behind() {
        let dir = std::env::temp_dir().join(format!("qnc-poster-test-{}", std::process::id()));
        let output = dir.join("p").join("poster.jpg");
        let outcome = create_poster(
            &dir.join("does-not-exist.mxf"),
            1.0,
            FrameTimebase::new(50, 1).unwrap(),
            &output,
            &AtomicBool::new(false),
        );
        assert!(!matches!(outcome, Ok(true)));
        assert!(!output.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
