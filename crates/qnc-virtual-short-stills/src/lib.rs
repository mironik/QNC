//! Captures and stores the IN/OUT stills of a virtual short from confirmed
//! Broadcast Player preview frames.
//!
//! This module is a local helper, not a source of truth: it does not create a
//! virtual short and it does not write the project database. The caller first
//! stores the short in the DB through `qnc-virtual-shots`; this module then
//! stores the image artifacts under the project directory and returns their
//! project URIs so the DB owner can publish them.

use std::{fs, io::BufWriter, path::Path, sync::Arc};

use qnc_source_preview::{MonitorFrame, PreviewView, ShownPicture};

pub const MODULE_ID: &str = "qnc.module.virtual-short-stills";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const STILL_WIDTH: u32 = 512;
pub const STILL_HEIGHT: u32 = 288;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredShortStills {
    pub in_uri: String,
    pub out_uri: String,
}

#[derive(Debug, Default, Clone)]
pub struct VirtualShortStillCache {
    clip_id: Option<String>,
    in_still: Option<CandidateStill>,
    out_still: Option<CandidateStill>,
}

#[derive(Debug, Clone)]
struct CandidateStill {
    clip_id: String,
    frame: u64,
    /// Which picture of the player it is, when it came from the monitor's own frame.
    sequence: Option<(u64, u64)>,
    width: usize,
    height: usize,
    rgba: Arc<[u8]>,
}

impl VirtualShortStillCache {
    pub fn clear(&mut self) {
        self.clip_id = None;
        self.in_still = None;
        self.out_still = None;
    }

    pub fn clear_if_clip_changed(&mut self, clip_id: Option<&str>) {
        if self.clip_id.as_deref() != clip_id {
            self.clear();
            self.clip_id = clip_id.map(str::to_string);
        }
    }

    pub fn capture_in(&mut self, preview: &PreviewView) -> Result<(), String> {
        let still = candidate_from_preview(preview)?;
        self.clear_if_clip_changed(Some(&still.clip_id));
        self.in_still = Some(still);
        Ok(())
    }

    pub fn capture_out(&mut self, preview: &PreviewView) -> Result<(), String> {
        let still = candidate_from_preview(preview)?;
        self.clear_if_clip_changed(Some(&still.clip_id));
        self.out_still = Some(still);
        Ok(())
    }

    /// Follows the monitor: whenever it shows the picture at the IN or the OUT of the clip,
    /// that picture is the still of the mark, however the mark got there (a mark key, taking
    /// and arrows, fit to a slot, a drag). A newer picture of the same frame replaces it.
    pub fn follow(&mut self, shown: Option<(String, ShownPicture)>, marks: (Option<u64>, Option<u64>)) {
        let Some((clip_id, picture)) = shown else { return };
        if picture.rgba.len() != picture.size[0] * picture.size[1] * 4 || picture.size.contains(&0) {
            return;
        }
        let held = |still: &Option<CandidateStill>| {
            still.as_ref().is_some_and(|s| s.clip_id == clip_id && s.frame == picture.frame && s.sequence == Some(picture.sequence))
        };
        let (at_in, at_out) = (marks.0 == Some(picture.frame) && !held(&self.in_still), marks.1 == Some(picture.frame) && !held(&self.out_still));
        if !at_in && !at_out {
            return;
        }
        self.clear_if_clip_changed(Some(&clip_id));
        let still = CandidateStill {
            clip_id,
            frame: picture.frame,
            sequence: Some(picture.sequence),
            width: picture.size[0],
            height: picture.size[1],
            rgba: picture.rgba,
        };
        if at_in {
            self.in_still = Some(still.clone());
        }
        if at_out {
            self.out_still = Some(still);
        }
    }

    pub fn has_pair_for(&self, clip_id: &str, in_frame: u64, out_frame: u64) -> bool {
        matches!(
            (&self.in_still, &self.out_still),
            (Some(input), Some(output))
                if input.clip_id == clip_id
                    && output.clip_id == clip_id
                    && input.frame == in_frame
                    && output.frame == out_frame
        )
    }

    pub fn store_for_short(
        &self,
        stills_dir: &Path,
        stills_root_uri: &str,
        shot_id: &str,
        clip_id: &str,
        in_frame: u64,
        out_frame: u64,
    ) -> Result<StoredShortStills, String> {
        if !self.has_pair_for(clip_id, in_frame, out_frame) {
            return Err(qnc_ui_text::text("stills.out_pictures_were_taken_saved_subclip").into());
        }
        if shot_id.is_empty() || shot_id.contains(['/', '\\']) {
            return Err(qnc_ui_text::text("stills.invalid_subclip_identity").into());
        }
        let Some(input) = &self.in_still else {
            return Err(qnc_ui_text::text("stills.picture_available").into());
        };
        let Some(output) = &self.out_still else {
            return Err(qnc_ui_text::text("stills.out_picture_available").into());
        };
        let dir = stills_dir.join(shot_id);
        fs::create_dir_all(&dir).map_err(|error| format!("virtual short directory: {error}"))?;
        write_still(input, &dir.join("in.jpg"))?;
        write_still(output, &dir.join("out.jpg"))?;
        let root = stills_root_uri.trim_end_matches('/');
        Ok(StoredShortStills {
            in_uri: format!("{root}/{shot_id}/in.jpg"),
            out_uri: format!("{root}/{shot_id}/out.jpg"),
        })
    }
}

impl VirtualShortStillCache {
    /// The poster of a B-roll shot made for a cover (user rule 2026-10-01): the IN still
    /// captured when its IN was set, written as `<shot_id>/in.jpg`; returns its URI.
    pub fn store_poster(
        &self,
        stills_dir: &Path,
        stills_root_uri: &str,
        shot_id: &str,
        clip_id: &str,
        in_frame: u64,
    ) -> Result<String, String> {
        let input = self
            .in_still
            .as_ref()
            .filter(|still| still.clip_id == clip_id && still.frame == in_frame)
            .ok_or_else(|| qnc_ui_text::text("stills.picture_b_roll_taken").to_string())?;
        if shot_id.is_empty() || shot_id.contains(['/', '\\']) {
            return Err(qnc_ui_text::text("stills.invalid_subclip_identity").into());
        }
        let dir = stills_dir.join(shot_id);
        fs::create_dir_all(&dir).map_err(|error| format!("B-roll poster directory: {error}"))?;
        write_still(input, &dir.join("in.jpg"))?;
        Ok(format!("{}/{shot_id}/in.jpg", stills_root_uri.trim_end_matches('/')))
    }
}

fn candidate_from_preview(preview: &PreviewView) -> Result<CandidateStill, String> {
    let clip_id = preview
        .clip_id
        .clone()
        .ok_or_else(|| qnc_ui_text::text("stills.clip_open_broadcast_player").to_string())?;
    let frame = preview
        .timeline
        .playhead_frame
        .ok_or_else(|| qnc_ui_text::text("stills.broadcast_player_confirmed_position").to_string())?;
    let picture = preview
        .monitor_frame
        .as_ref()
        .ok_or_else(|| qnc_ui_text::text("stills.broadcast_player_given_picture_mark").to_string())?;
    validate_picture(picture)?;
    Ok(CandidateStill {
        clip_id,
        frame,
        sequence: None,
        width: picture.width,
        height: picture.height,
        rgba: picture.rgba.clone(),
    })
}

fn validate_picture(picture: &MonitorFrame) -> Result<(), String> {
    if picture.width == 0 || picture.height == 0 {
        return Err(qnc_ui_text::text("stills.invalid_picture_size").into());
    }
    let expected = picture
        .width
        .checked_mul(picture.height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| qnc_ui_text::text("stills.picture_too_large").to_string())?;
    if picture.rgba.len() != expected {
        return Err(qnc_ui_text::text("stills.rgba_picture_does_have_expected_size").into());
    }
    Ok(())
}

fn write_still(still: &CandidateStill, path: &Path) -> Result<(), String> {
    let source =
        image::RgbaImage::from_raw(still.width as u32, still.height as u32, still.rgba.to_vec())
            .ok_or_else(|| qnc_ui_text::text("stills.rgba_picture_does_have_expected_size").to_string())?;
    let fitted = fit_to_canvas(&source);
    let file = fs::File::create(path).map_err(|error| format!("virtual short still: {error}"))?;
    let mut writer = BufWriter::new(file);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 90)
        .encode_image(&fitted)
        .map_err(|error| format!("virtual short jpeg: {error}"))
}

fn fit_to_canvas(source: &image::RgbaImage) -> image::RgbaImage {
    let width_scale = STILL_WIDTH as f32 / source.width().max(1) as f32;
    let height_scale = STILL_HEIGHT as f32 / source.height().max(1) as f32;
    let scale = width_scale.min(height_scale);
    let width = ((source.width() as f32 * scale).round() as u32).clamp(1, STILL_WIDTH);
    let height = ((source.height() as f32 * scale).round() as u32).clamp(1, STILL_HEIGHT);
    let resized =
        image::imageops::resize(source, width, height, image::imageops::FilterType::Triangle);
    let mut canvas =
        image::RgbaImage::from_pixel(STILL_WIDTH, STILL_HEIGHT, image::Rgba([0, 0, 0, 255]));
    let x = (STILL_WIDTH - width) / 2;
    let y = (STILL_HEIGHT - height) / 2;
    image::imageops::overlay(&mut canvas, &resized, x.into(), y.into());
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_source_preview::{MonitorFrame, PreviewView};

    fn preview(clip_id: &str, frame: u64, rgba: [u8; 4]) -> PreviewView {
        let mut view = PreviewView::default();
        view.clip_id = Some(clip_id.into());
        view.timeline.duration_frames = 100;
        view.timeline.playhead_frame = Some(frame);
        view.monitor_frame = Some(MonitorFrame {
            session_id: "s".into(),
            generation: 1,
            sequence: frame,
            width: 2,
            height: 1,
            rgba: Arc::from([rgba, rgba].concat()),
            frame_map: None,
        });
        view
    }

    #[test]
    fn captures_in_and_out_for_the_same_confirmed_clip() {
        let mut cache = VirtualShortStillCache::default();
        cache
            .capture_in(&preview("clip-a", 10, [255, 0, 0, 255]))
            .unwrap();
        cache
            .capture_out(&preview("clip-a", 20, [0, 255, 0, 255]))
            .unwrap();
        assert!(cache.has_pair_for("clip-a", 10, 20));
        assert!(!cache.has_pair_for("clip-a", 10, 21));
    }

    #[test]
    fn changing_clip_clears_the_cached_pair() {
        let mut cache = VirtualShortStillCache::default();
        cache
            .capture_in(&preview("clip-a", 10, [255, 0, 0, 255]))
            .unwrap();
        cache.clear_if_clip_changed(Some("clip-b"));
        cache
            .capture_out(&preview("clip-b", 20, [0, 255, 0, 255]))
            .unwrap();
        assert!(!cache.has_pair_for("clip-a", 10, 20));
    }

    #[test]
    fn a_cover_poster_is_its_in_still_under_b_roll() {
        let dir = tempfile::tempdir().unwrap();
        let stills_dir = dir.path().join("products").join("b_roll_virtual_clips");
        let mut cache = VirtualShortStillCache::default();
        cache.capture_in(&preview("clip-a", 10, [255, 0, 0, 255])).unwrap();
        let root = "qnc://local/project/p1/products/b_roll_virtual_clips";
        let uri = cache.store_poster(&stills_dir, root, "clip-a_broll_001", "clip-a", 10).unwrap();
        assert_eq!(uri, format!("{root}/clip-a_broll_001/in.jpg"));
        assert!(stills_dir.join("clip-a_broll_001").join("in.jpg").is_file());
        assert!(cache.store_poster(&stills_dir, root, "x", "clip-a", 11).is_err(), "another IN has no picture");
    }

    #[test]
    fn writes_two_poster_sized_jpegs_under_virtual_shorts() {
        let dir = tempfile::tempdir().unwrap();
        let stills_dir = dir.path().join("products").join("virtual_shorts");
        let mut cache = VirtualShortStillCache::default();
        cache
            .capture_in(&preview("clip-a", 10, [255, 0, 0, 255]))
            .unwrap();
        cache
            .capture_out(&preview("clip-a", 20, [0, 255, 0, 255]))
            .unwrap();
        let stored = cache
            .store_for_short(
                &stills_dir,
                "qnc://local/project/p1/products/virtual_shorts",
                "clip-a_shot_001",
                "clip-a",
                10,
                20,
            )
            .unwrap();
        assert_eq!(
            stored.in_uri,
            "qnc://local/project/p1/products/virtual_shorts/clip-a_shot_001/in.jpg"
        );
        assert!(stills_dir.join("clip-a_shot_001").join("in.jpg").is_file());
        assert!(stills_dir.join("clip-a_shot_001").join("out.jpg").is_file());
    }

    #[test]
    fn a_mark_moved_without_i_or_o_takes_the_picture_the_monitor_shows_there() {
        let shown = |frame, sequence| {
            let picture = ShownPicture { frame, sequence: (1, sequence), size: [2, 2], rgba: Arc::from(vec![9u8; 16]) };
            Some(("clip-a".to_string(), picture))
        };
        let mut cache = VirtualShortStillCache::default();
        cache.follow(shown(12, 1), (Some(12), Some(40))); // IN moved to 12 by taking it and stepping
        cache.follow(shown(20, 2), (Some(12), Some(40))); // a picture away from the marks is not taken
        assert!(!cache.has_pair_for("clip-a", 12, 40));
        cache.follow(shown(40, 3), (Some(12), Some(40)));
        assert!(cache.has_pair_for("clip-a", 12, 40));
        cache.follow(Some(("clip-b".to_string(), shown(12, 4).unwrap().1)), (Some(12), Some(40)));
        assert!(!cache.has_pair_for("clip-a", 12, 40), "another clip starts over");
    }

}
