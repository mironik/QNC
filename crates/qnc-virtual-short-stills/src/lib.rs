//! Captures and stores the IN/OUT stills of a virtual short from confirmed
//! Broadcast Player preview frames.
//!
//! This module is a local helper, not a source of truth: it does not create a
//! virtual short and it does not write the project database. The caller first
//! stores the short in the DB through `qnc-virtual-shots`; this module then
//! stores the image artifacts under the project directory and returns their
//! project URIs so the DB owner can publish them.

use std::{fs, io::BufWriter, path::Path, sync::Arc};

use qnc_source_preview::{MonitorFrame, PreviewView};

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
        project_dir: &Path,
        output_root_uri: &str,
        shot_id: &str,
        clip_id: &str,
        in_frame: u64,
        out_frame: u64,
    ) -> Result<StoredShortStills, String> {
        if !self.has_pair_for(clip_id, in_frame, out_frame) {
            return Err("IN/OUT slike nisu uhvacene za spremljeni virtualni kadar.".into());
        }
        if shot_id.is_empty() || shot_id.contains(['/', '\\']) {
            return Err("Neispravan identitet virtualnog kadra.".into());
        }
        let Some(input) = &self.in_still else {
            return Err("IN slika nije dostupna.".into());
        };
        let Some(output) = &self.out_still else {
            return Err("OUT slika nije dostupna.".into());
        };
        let dir = project_dir.join("virtual_shorts").join(shot_id);
        fs::create_dir_all(&dir).map_err(|error| format!("virtual short directory: {error}"))?;
        write_still(input, &dir.join("in.jpg"))?;
        write_still(output, &dir.join("out.jpg"))?;
        let root = output_root_uri.trim_end_matches('/');
        Ok(StoredShortStills {
            in_uri: format!("{root}/virtual_shorts/{shot_id}/in.jpg"),
            out_uri: format!("{root}/virtual_shorts/{shot_id}/out.jpg"),
        })
    }
}

fn candidate_from_preview(preview: &PreviewView) -> Result<CandidateStill, String> {
    let clip_id = preview
        .clip_id
        .clone()
        .ok_or_else(|| "Klip nije otvoren u Broadcast Playeru.".to_string())?;
    let frame = preview
        .timeline
        .playhead_frame
        .ok_or_else(|| "Broadcast Player nije potvrdio poziciju.".to_string())?;
    let picture = preview
        .monitor_frame
        .as_ref()
        .ok_or_else(|| "Broadcast Player nije objavio sliku za marker.".to_string())?;
    validate_picture(picture)?;
    Ok(CandidateStill {
        clip_id,
        frame,
        width: picture.width,
        height: picture.height,
        rgba: picture.rgba.clone(),
    })
}

fn validate_picture(picture: &MonitorFrame) -> Result<(), String> {
    if picture.width == 0 || picture.height == 0 {
        return Err("Neispravna velicina slike.".into());
    }
    let expected = picture
        .width
        .checked_mul(picture.height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Slika je prevelika.".to_string())?;
    if picture.rgba.len() != expected {
        return Err("RGBA slika nema ocekivanu velicinu.".into());
    }
    Ok(())
}

fn write_still(still: &CandidateStill, path: &Path) -> Result<(), String> {
    let source =
        image::RgbaImage::from_raw(still.width as u32, still.height as u32, still.rgba.to_vec())
            .ok_or_else(|| "RGBA slika nema ocekivanu velicinu.".to_string())?;
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
    fn writes_two_poster_sized_jpegs_under_virtual_shorts() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = VirtualShortStillCache::default();
        cache
            .capture_in(&preview("clip-a", 10, [255, 0, 0, 255]))
            .unwrap();
        cache
            .capture_out(&preview("clip-a", 20, [0, 255, 0, 255]))
            .unwrap();
        let stored = cache
            .store_for_short(
                dir.path(),
                "qnc://local/project/p1",
                "clip-a_shot_001",
                "clip-a",
                10,
                20,
            )
            .unwrap();
        assert_eq!(
            stored.in_uri,
            "qnc://local/project/p1/virtual_shorts/clip-a_shot_001/in.jpg"
        );
        assert!(dir
            .path()
            .join("virtual_shorts")
            .join("clip-a_shot_001")
            .join("in.jpg")
            .is_file());
        assert!(dir
            .path()
            .join("virtual_shorts")
            .join("clip-a_shot_001")
            .join("out.jpg")
            .is_file());
    }
}
