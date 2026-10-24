//! The clip records of the active project, read only, as the player input reads them: the
//! saved clip with its media record and what its import made (`qnc-content-store`). One reader
//! for every program that prepares a clip or a story (preview, prerender, export); nothing is
//! read from a host database, so a copied project plays anywhere. It writes nothing.

use qnc_content_store::{Access, ContentTarget};
use qnc_player_input::{PlayerClipRecord, PlayerContentRead};

pub const MODULE_ID: &str = "qnc.module.player-clip-content";

#[derive(Clone)]
pub struct PlayerClipContent {
    /// The content database of the active project; its error is given on the first read.
    pub target: Result<ContentTarget, String>,
}

impl PlayerContentRead for PlayerClipContent {
    fn read_clip(&self, clip_id: &str) -> Result<Option<PlayerClipRecord>, String> {
        let target = self.target.as_ref().map_err(Clone::clone)?;
        let stored = target.open(Access::ReadOnly)?.read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
            imported_copy_of: stored.imported_copy_of.map(|copy| match copy {
                qnc_content_store::ImportedCopy::Original => qnc_player_input::Representation::Original,
                qnc_content_store::ImportedCopy::Proxy => qnc_player_input::Representation::Proxy,
            }),
            imported_optimized: stored.imported_optimized,
        }))
    }
}
