//! The audio channels of the original of a clip (user rule 2026-09-30: the channel heard
//! on A1 is chosen from the real channels of the clip). Read-only from the stored
//! record, as the player counts them: every channel of every audio stream, in stream
//! order. No probe and no guessed count: a record without channel counts gives none.

use qnc_content_store::Access;
use qnc_media_metadata::StreamDetails;

pub use qnc_content_store::ContentTarget;

/// How many audio channels the stored original of `clip_id` has; `None` when the clip
/// or the channel count of one of its audio streams is not in the database.
pub fn read_clip(target: &ContentTarget, clip_id: &str) -> Result<Option<u16>, String> {
    let Some(stored) = target.open(Access::ReadOnly)?.read(clip_id)? else {
        return Ok(None);
    };
    let mut count: u32 = 0;
    for stream in &stored.clip.snapshot.metadata.original.streams {
        if let StreamDetails::Audio(audio) = &stream.details {
            let Some(channels) = audio.channels.as_ref() else {
                return Ok(None);
            };
            count = count.saturating_add(channels.value);
        }
    }
    Ok(u16::try_from(count).ok())
}

/// The channel count of the clip shown now, read once per clip. The database stays the
/// truth: another clip, or another project, reads it again.
#[derive(Debug, Default)]
pub struct SourceChannels {
    current: Option<(String, String, Option<u16>)>,
}

impl SourceChannels {
    pub fn new() -> Self {
        Self::default()
    }

    /// The channel count of `clip_id` in the project behind `target`.
    pub fn for_clip(&mut self, target: Option<&ContentTarget>, clip_id: Option<&str>) -> Option<u16> {
        let (target, clip_id) = (target?, clip_id?);
        let same = self
            .current
            .as_ref()
            .is_some_and(|(uri, id, _)| uri == target.uri() && id == clip_id);
        if !same {
            let count = read_clip(target, clip_id).ok().flatten();
            self.current = Some((target.uri().to_string(), clip_id.to_string(), count));
        }
        self.current.as_ref().and_then(|(_, _, count)| *count)
    }
}
