//! Reads the stored record of a clip through the project content store (read-only).

use qnc_content_store::{Access, ContentTarget};
use qnc_media_metadata::StreamDetails;

use crate::SourceTimecode;

/// The source timecode of one clip from its stored original record; `None` when
/// the clip or its source frame rate is not in the database.
pub fn read_clip(target: &ContentTarget, clip_id: &str) -> Result<Option<SourceTimecode>, String> {
    let Some(stored) = target.open(Access::ReadOnly)?.read(clip_id)? else {
        return Ok(None);
    };
    let original = &stored.clip.snapshot.metadata.original;
    let rate = original.streams.iter().find_map(|stream| match &stream.details {
        StreamDetails::Video(video) => video.frame_rate.as_ref().map(|rate| rate.value),
        _ => None,
    });
    let Some(rate) = rate else {
        return Ok(None);
    };
    let (Ok(num), Ok(den)) = (u64::try_from(rate.fps_num), u64::try_from(rate.fps_den)) else {
        return Ok(None);
    };
    let tags = original
        .tags
        .iter()
        .map(|(key, fact)| (key.as_str(), fact.value.as_str()));
    Ok(SourceTimecode::from_tags(tags, (num, den)))
}

/// The timecode of the clip shown now, read from the database in the background and read
/// again every second while it is shown (piece `qnc-clip-follow`); the form's thread never
/// waits on the database (a background job writing it, e.g. the filmstrip, made a click wait
/// 1.5 s).
#[derive(Debug)]
pub struct SourceTimecodes {
    follow: qnc_clip_follow::ClipFollow<Option<SourceTimecode>>,
}

impl Default for SourceTimecodes {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceTimecodes {
    pub fn new() -> Self {
        Self { follow: qnc_clip_follow::ClipFollow::new("source-timecode", qnc_clip_follow::EVERY) }
    }

    /// The timecode of `clip_id` in the project behind `target`; `None` until it is read.
    pub fn for_clip(&mut self, target: Option<&ContentTarget>, clip_id: Option<&str>) -> Option<SourceTimecode> {
        let (target, clip_id) = (target?, clip_id?);
        self.follow
            .get(target.uri(), clip_id, true, || {
                let (owned, clip) = (target.clone(), clip_id.to_string());
                move || read_clip(&owned, &clip)
            })
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy of a real project database (never the project itself):
    /// QNC_TC_DB=<copy> QNC_TC_URI=<content uri> QNC_TC_CLIP=<clip id>.
    #[test]
    #[ignore = "reads a copy of a real project database"]
    fn reads_the_camera_timecode_of_a_stored_clip() {
        let var = |name| std::env::var(name).unwrap();
        let db = std::path::PathBuf::from(var("QNC_TC_DB"));
        let target =
            ContentTarget::from_owner_binding(&db, &var("QNC_TC_URI")).unwrap();
        let timecode = read_clip(&target, &var("QNC_TC_CLIP")).unwrap().unwrap();
        println!("start {:?} fps {} -> {}", timecode.start_frame, timecode.fps, timecode.label(0));
        assert!(timecode.start_frame.is_some());
    }
}
