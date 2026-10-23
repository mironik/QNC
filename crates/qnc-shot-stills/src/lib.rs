//! The stills of a saved virtual shot (user rules 2026-09-22 and 2026-10-01). The shot
//! is in the database first; its pictures are then written under the project product
//! folder the project settings name, and their URIs are published through the virtual
//! shots table module. A short gets its IN and OUT still, a B-roll shot of a cover its
//! poster (the IN still), so the pool shows a picture instead of a bare name.
//!
//! A picture is the one the monitor showed at that frame when it was captured; else the
//! frame is decoded from the media the project plays, through the decoder catalog (user
//! 2026-10-23: a SubClip whose OUT is the end of the clip had no poster, because the
//! monitor never showed that frame). The work runs off the form thread; nothing here is
//! a source of truth, the database says which stills a shot has.

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use qnc_db_broker::ProjectDbTarget;
use qnc_still_locator::StillLocator;
use qnc_virtual_short_stills::{RgbaImage, VirtualShortStillCache, STILL_HEIGHT, STILL_WIDTH};
use qnc_work_settings::{ProductArea, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.shot-stills";

/// Where the project keeps the stills of one kind of shot, and where this machine reads
/// a clip's picture when the monitor did not capture it.
pub struct Place<'a> {
    pub target: &'a ProjectDbTarget,
    pub settings: &'a WorkSettings,
    pub project_dir: Option<&'a Path>,
    pub locator: Option<StillLocator>,
}

impl Place<'_> {
    fn folder(&self, area: ProductArea) -> Result<(PathBuf, String), String> {
        let dir = self
            .project_dir
            .ok_or_else(|| qnc_ui_text::text("story.msg.project_folder_unavailable").to_string())?;
        Ok((self.settings.product_local_dir(dir, area), self.settings.product_uri(area)))
    }
}

/// The pictures captured from the monitor and the stills being written for saved shots.
#[derive(Default)]
pub struct ShotStills {
    cache: VirtualShortStillCache,
    published: Arc<AtomicBool>,
}

/// One shot whose stills are written: the IN still is its poster, a short has an OUT too.
struct Job {
    shot: String,
    clip: String,
    out_frame: Option<u64>,
    folder: Result<(PathBuf, String), String>,
    pictures: Vec<(u64, Option<RgbaImage>)>,
}

impl ShotStills {
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn clear_if_clip_changed(&mut self, clip_id: Option<&str>) {
        self.cache.clear_if_clip_changed(clip_id);
    }

    /// Keeps the picture the monitor shows at the IN or the OUT of the clip.
    pub fn follow(&mut self, shown: Option<(String, qnc_source_preview::ShownPicture)>, marks: (Option<u64>, Option<u64>)) {
        self.cache.follow(shown, marks);
    }

    /// The stills of a saved short (`Some`: shot, clip, IN/OUT) or the posters of the
    /// covers just saved (shot, clip, IN frame); written and published off this thread.
    pub fn store(&mut self, place: Place<'_>, short: Option<(&str, &str, (u64, u64))>, covers: Vec<(String, String, u64)>) {
        let job = |shot: &str, clip: &str, in_frame: u64, out_frame: Option<u64>, area| {
            let pictures = std::iter::once(in_frame)
                .chain(out_frame)
                .map(|frame| (frame, self.cache.captured(clip, frame)))
                .collect();
            Job { shot: shot.into(), clip: clip.into(), out_frame, folder: place.folder(area), pictures }
        };
        let jobs: Vec<Job> = match short {
            Some((shot, clip, (in_frame, out_frame))) => vec![job(shot, clip, in_frame, Some(out_frame), ProductArea::VirtualShorts)],
            None => covers
                .iter()
                .map(|(shot, clip, in_frame)| job(shot, clip, *in_frame, None, ProductArea::BRollVirtualClips))
                .collect(),
        };
        if jobs.is_empty() {
            return;
        }
        let (target, locator, published) = (place.target.clone(), place.locator, self.published.clone());
        let spawned = std::thread::Builder::new().name("qnc-shot-stills".into()).spawn(move || {
            for job in jobs {
                if let Err(error) = write_and_publish(&target, locator.as_ref(), job) {
                    log(&format!("shot-stills publish failed: {error}"));
                }
            }
            published.store(true, Ordering::Release);
        });
        if let Err(error) = spawned {
            log(&format!("shot-stills thread: {error}"));
        }
    }

    /// Whether stills were published since the last call: the pool reads its shots again.
    pub fn take_published(&mut self) -> bool {
        self.published.swap(false, Ordering::AcqRel)
    }
}

fn write_and_publish(target: &ProjectDbTarget, locator: Option<&StillLocator>, job: Job) -> Result<(), String> {
    let written = write(locator, &job);
    if let Err(error) = &written {
        log(&format!("shot-stills {} {}: {error}", job.shot, job.clip));
    }
    match job.out_frame {
        Some(_) => qnc_virtual_shots::publish_stills_now(target, &job.shot, written.map(|uris| (uris[0].clone(), uris[1].clone()))),
        None => qnc_virtual_shots::publish_poster_now(target, &job.shot, written.map(|mut uris| uris.remove(0))),
    }
}

/// The JPEG of every picture of the job, in order (`in.jpg`, `out.jpg`); their URIs.
fn write(locator: Option<&StillLocator>, job: &Job) -> Result<Vec<String>, String> {
    if job.shot.is_empty() || job.shot.contains(['/', '\\']) {
        return Err(qnc_ui_text::text("stills.invalid_subclip_identity").into());
    }
    let (dir, uri) = job.folder.clone()?;
    let dir = dir.join(&job.shot);
    std::fs::create_dir_all(&dir).map_err(|error| format!("virtual shot directory: {error}"))?;
    let mut uris = Vec::new();
    for ((frame, captured), name) in job.pictures.iter().zip(["in.jpg", "out.jpg"]) {
        let picture = match captured {
            Some(picture) => picture.clone(),
            // The OUT of a SubClip without its own OUT is the end of the clip, the bound
            // after its last picture (user 2026-10-23: every clip is virtual, its IN and OUT
            // are its start and end): that still is the last picture.
            None => decode(locator, &job.clip, *frame).or_else(|error| match frame.checked_sub(1) {
                Some(last) if name == "out.jpg" => decode(locator, &job.clip, last),
                _ => Err(error),
            })?,
        };
        qnc_virtual_short_stills::write_still_image(&picture, &dir.join(name))?;
        uris.push(format!("{}/{}/{name}", uri.trim_end_matches('/'), job.shot));
    }
    Ok(uris)
}

/// One exact frame of the clip from the media the project plays, through the decoder
/// catalog; no probe, nothing written but a scratch file of the system that is removed.
fn decode(locator: Option<&StillLocator>, clip: &str, frame: u64) -> Result<RgbaImage, String> {
    let failed = |error: &str| qnc_ui_text::fill("stills.picture_not_decoded", &[("frame", &frame.to_string()), ("error", error)]);
    let locator = locator.ok_or_else(|| failed(&qnc_ui_text::text("stills.clip_open_broadcast_player")))?;
    let place = locator.locate(clip).map_err(|error| failed(&error))?;
    let extractor = qnc_decoder_catalog::installed_filmstrip_extractor()
        .map_err(|error| failed(&error.to_string()))?
        .ok_or_else(|| failed(&qnc_ui_text::text("story.msg.no_still_extractor")))?;
    let (num, den) = place.fps;
    // A quarter of a frame early: the first picture at or after it is the frame itself.
    let seek = ((frame as f64 - 0.25) * f64::from(den) / f64::from(num.max(1))).max(0.0);
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!("qnc-shot-still-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let result = (|| {
        std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
        let cancel = AtomicBool::new(false);
        extractor.extract_run_with_cancel(&place.file, seek, 1, [STILL_WIDTH, STILL_HEIGHT], &scratch, &cancel)?;
        // No picture is written at or past the end of the media.
        let bytes = std::fs::read(scratch.join("000.jpg")).map_err(|_| qnc_ui_text::text("stills.past_clip_end").to_string())?;
        qnc_virtual_short_stills::read_jpeg(&bytes)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result.map_err(|error| failed(&error))
}

fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(dir: &Path, pictures: Vec<(u64, Option<RgbaImage>)>, out_frame: Option<u64>) -> Job {
        Job {
            shot: "short-1".into(),
            clip: "clip-1".into(),
            out_frame,
            folder: Ok((dir.to_path_buf(), "qnc://local/project/p/products/virtual_shorts".into())),
            pictures,
        }
    }

    #[test]
    fn the_pictures_the_monitor_showed_are_written_without_decoding() {
        let dir = tempfile::tempdir().unwrap();
        let picture = RgbaImage::from_pixel(4, 2, qnc_virtual_short_stills::image_pixel([200, 10, 10, 255]));
        let uris = write(None, &job(dir.path(), vec![(10, Some(picture.clone())), (50, Some(picture))], Some(50))).unwrap();
        assert_eq!(uris, ["qnc://local/project/p/products/virtual_shorts/short-1/in.jpg", "qnc://local/project/p/products/virtual_shorts/short-1/out.jpg"]);
        assert!(dir.path().join("short-1/in.jpg").is_file() && dir.path().join("short-1/out.jpg").is_file());
    }

    #[test]
    fn a_picture_the_monitor_never_showed_is_decoded_and_says_why_when_it_cannot_be() {
        let dir = tempfile::tempdir().unwrap();
        let picture = RgbaImage::from_pixel(4, 2, qnc_virtual_short_stills::image_pixel([0, 0, 0, 255]));
        // The OUT at the end of the clip was never on the monitor and no clip is open here.
        let error = write(None, &job(dir.path(), vec![(10, Some(picture)), (500, None)], Some(500))).unwrap_err();
        // The OUT at the clip end falls back to its last picture, which is tried too.
        assert!(error.contains("499"), "{error}");
    }
}
