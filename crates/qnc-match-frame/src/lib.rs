//! Match frame (docs/98 1.3, user 2026-11-04; F in every editor): from a story to the same
//! picture of its source clip, and from a source clip to where the story plays that picture.
//! A piece of a story plays `[source_in, source_out)` of a clip over `[start, end)` of the
//! story (the frame ratio of a story and its clip is that of the two ranges, 2:1 and 1:2 too).
//! Pure: the owner gives the pieces, this block finds the frame.

pub const MODULE_ID: &str = "qnc.module.match-frame";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The catalog action (contracts/qnc-keyboard-shortcuts.json).
pub const MATCH_FRAME: &str = "match_frame";

/// A piece of a story: a segment or a cover, what of which clip it plays where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece<'a> {
    pub clip_id: &'a str,
    pub source_in: u64,
    pub source_out: u64,
    pub start: u64,
    pub end: u64,
}

impl Piece<'_> {
    fn plays(&self, story_frame: u64) -> bool {
        (self.start..self.end).contains(&story_frame)
    }

    fn holds(&self, clip_id: &str, source_frame: u64) -> bool {
        self.clip_id == clip_id && (self.source_in..self.source_out).contains(&source_frame)
    }

    fn source_of(&self, story_frame: u64) -> u64 {
        let (story, source) = (self.end - self.start, self.source_out - self.source_in);
        self.source_in + (story_frame - self.start) * source / story.max(1)
    }

    fn story_of(&self, source_frame: u64) -> u64 {
        let (story, source) = (self.end - self.start, self.source_out - self.source_in);
        self.start + (source_frame - self.source_in) * story / source.max(1)
    }
}

/// The clip and its frame the story shows at `story_frame`: the first piece that plays there
/// (give covers before segments: a cover is over its segment).
pub fn source_at<'a>(pieces: &[Piece<'a>], story_frame: u64) -> Option<(&'a str, u64)> {
    let piece = pieces.iter().find(|piece| piece.plays(story_frame))?;
    Some((piece.clip_id, piece.source_of(story_frame)))
}

/// Where the story plays `source_frame` of `clip_id`: the first such place at or after `from`,
/// else the first of all (F again goes on to the next place the picture is used).
pub fn story_at(pieces: &[Piece<'_>], clip_id: &str, source_frame: u64, from: u64) -> Option<u64> {
    let mut places: Vec<u64> = pieces.iter().filter(|piece| piece.holds(clip_id, source_frame)).map(|piece| piece.story_of(source_frame)).collect();
    places.sort_unstable();
    places.iter().find(|frame| **frame >= from).or(places.first()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces() -> Vec<Piece<'static>> {
        vec![
            Piece { clip_id: "cover", source_in: 500, source_out: 550, start: 20, end: 70 },
            Piece { clip_id: "a", source_in: 100, source_out: 200, start: 0, end: 100 },
            Piece { clip_id: "b", source_in: 10, source_out: 60, start: 100, end: 200 },
        ]
    }

    #[test]
    fn the_story_shows_its_source_frame_and_the_source_finds_its_story_frame() {
        let pieces = pieces();
        assert_eq!(source_at(&pieces, 10), Some(("a", 110)));
        assert_eq!(source_at(&pieces, 30), Some(("cover", 510)), "a cover over its segment");
        assert_eq!(source_at(&pieces, 150), Some(("b", 35)), "a clip of half the rate: 2:1");
        assert_eq!(source_at(&pieces, 300), None);
        assert_eq!(story_at(&pieces, "b", 35, 0), Some(150));
        assert_eq!(story_at(&pieces, "a", 110, 0), Some(10));
        assert_eq!(story_at(&pieces, "a", 999, 0), None, "a picture the story does not use");
    }
}
