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

/// The timecode of the clip shown now, read once per clip. The database stays the
/// truth: another clip, or another project, reads it again.
#[derive(Debug, Default)]
pub struct SourceTimecodes {
    current: Option<(String, String, Option<SourceTimecode>)>,
}

impl SourceTimecodes {
    pub fn new() -> Self {
        Self::default()
    }

    /// The timecode of `clip_id` in the project behind `target`.
    pub fn for_clip(
        &mut self,
        target: Option<&ContentTarget>,
        clip_id: Option<&str>,
    ) -> Option<SourceTimecode> {
        let (target, clip_id) = (target?, clip_id?);
        let same = self
            .current
            .as_ref()
            .is_some_and(|(uri, id, _)| uri == target.uri() && id == clip_id);
        if !same {
            let timecode = read_clip(target, clip_id).ok().flatten();
            self.current = Some((target.uri().to_string(), clip_id.to_string(), timecode));
        }
        self.current.as_ref().and_then(|(_, _, timecode)| *timecode)
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
