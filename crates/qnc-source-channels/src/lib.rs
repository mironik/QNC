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

/// The channel count of the clip shown now, read from the database in the background and
/// read again every second while it is shown (piece `qnc-clip-follow`): a record completed
/// later, by the worker, shows its channels.
#[derive(Debug)]
pub struct SourceChannels {
    follow: qnc_clip_follow::ClipFollow<Option<u16>>,
}

impl Default for SourceChannels {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceChannels {
    pub fn new() -> Self {
        Self { follow: qnc_clip_follow::ClipFollow::new("source-channels", qnc_clip_follow::EVERY) }
    }

    /// The channel count of `clip_id` in the project behind `target`; `None` until it is read.
    pub fn for_clip(&mut self, target: Option<&ContentTarget>, clip_id: Option<&str>) -> Option<u16> {
        let (target, clip_id) = (target?, clip_id?);
        self.follow
            .get(target.uri(), clip_id, true, || {
                let (owned, clip) = (target.clone(), clip_id.to_string());
                move || read_clip(&owned, &clip)
            })
            .flatten()
    }
}
