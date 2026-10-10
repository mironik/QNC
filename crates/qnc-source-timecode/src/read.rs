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
    /// The read of another clip on its own thread: the form's thread never waits on the
    /// database (a background job writing it, e.g. the filmstrip, made a click wait 1.5 s).
    reading: Option<(String, String, std::sync::mpsc::Receiver<Option<SourceTimecode>>)>,
}

impl SourceTimecodes {
    pub fn new() -> Self {
        Self::default()
    }

    /// The timecode of `clip_id` in the project behind `target`; `None` while it is read
    /// (in the background, once per clip).
    pub fn for_clip(
        &mut self,
        target: Option<&ContentTarget>,
        clip_id: Option<&str>,
    ) -> Option<SourceTimecode> {
        let (target, clip_id) = (target?, clip_id?);
        if let Some((uri, id, done)) = &self.reading {
            if let Ok(timecode) = done.try_recv() {
                self.current = Some((uri.clone(), id.clone(), timecode));
                self.reading = None;
            }
        }
        let is = |uri: &str, id: &str| uri == target.uri() && id == clip_id;
        if let Some((_, _, timecode)) = self.current.as_ref().filter(|(uri, id, _)| is(uri, id)) {
            return *timecode;
        }
        if !self.reading.as_ref().is_some_and(|(uri, id, _)| is(uri, id)) {
            let (send, done) = std::sync::mpsc::channel();
            let (owned, clip) = (target.clone(), clip_id.to_string());
            std::thread::spawn(move || {
                let _ = send.send(read_clip(&owned, &clip).ok().flatten());
            });
            self.reading = Some((target.uri().to_string(), clip_id.to_string(), done));
        }
        None
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
