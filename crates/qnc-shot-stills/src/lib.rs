//! The stills of a saved virtual shot (user rules 2026-09-22 and 2026-10-01). The shot
//! is in the database first; its pictures captured from the confirmed player frames are
//! then written under the project product folder the project settings name, and their
//! URIs are published through the virtual shots table module. A short gets its IN and
//! OUT still, a B-roll shot of a cover its poster (the IN still), so the B-roll tab shows
//! a picture instead of a bare name. Nothing is kept here.

use std::path::Path;

use qnc_db_broker::ProjectDbTarget;
use qnc_virtual_short_stills::VirtualShortStillCache;
use qnc_work_settings::{ProductArea, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.shot-stills";

/// Where the project keeps the stills of one kind of shot.
pub struct Place<'a> {
    pub target: &'a ProjectDbTarget,
    pub settings: &'a WorkSettings,
    pub project_dir: Option<&'a Path>,
}

impl Place<'_> {
    fn folder(&self, area: ProductArea) -> Result<(std::path::PathBuf, String), String> {
        let dir = self
            .project_dir
            .ok_or_else(|| "Lokalni direktorij projekta nije dostupan.".to_string())?;
        Ok((self.settings.product_local_dir(dir, area), self.settings.product_uri(area)))
    }

    /// The IN/OUT stills of a saved short; a missing picture is published as the reason.
    pub fn short(&self, cache: &VirtualShortStillCache, shot: &str, clip: &str, marks: (u64, u64)) -> Result<(), String> {
        let stills = self.folder(ProductArea::VirtualShorts).and_then(|(dir, uri)| {
            cache
                .store_for_short(&dir, &uri, shot, clip, marks.0, marks.1)
                .map(|stills| (stills.in_uri, stills.out_uri))
        });
        qnc_virtual_shots::publish_stills_now(self.target, shot, stills)
    }

    /// The posters of B-roll shots saved for covers: (shot, clip, IN frame).
    pub fn covers(&self, cache: &VirtualShortStillCache, shots: Vec<(String, String, u64)>) -> Result<(), String> {
        for (shot, clip, in_frame) in shots {
            let poster = self
                .folder(ProductArea::BRollVirtualClips)
                .and_then(|(dir, uri)| cache.store_poster(&dir, &uri, &shot, &clip, in_frame));
            qnc_virtual_shots::publish_poster_now(self.target, &shot, poster)?;
        }
        Ok(())
    }
}
