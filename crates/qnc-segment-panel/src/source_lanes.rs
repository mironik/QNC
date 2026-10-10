//! What the source timeline shows of the A1/A2 lanes of the story model: the channel
//! picker of each lane and the wave over the video row. The model keeps the choice;
//! this piece only turns it into the timeline's input, so no form translates it.

use qnc_program_segments::{EditStill, SegmentsView};
use qnc_timeline::{AudioLane, ChannelChoice, TimelineEditStill, TimelineStillImage};

/// The channel picker of A1 (lane 0) or A2: a lane taken with the keyboard shows its
/// draft with the picker open.
pub fn source_lane_choice(segments: &SegmentsView, lane: u8) -> Option<ChannelChoice> {
    let (selected, count) = if lane == 0 { segments.a1_choice } else { segments.a2_choice }?;
    let draft = segments.lane_taken.filter(|(taken, _)| *taken == lane).map(|(_, draft)| draft);
    Some(ChannelChoice { selected: draft.unwrap_or(selected), count, open: draft.is_some() })
}

/// The wave shown over the video row: the taken lane's, else A1's, else A2's.
pub fn source_wave_over_video(segments: &SegmentsView) -> Option<(AudioLane, u8)> {
    let lanes = [AudioLane::A1, AudioLane::A2];
    let taken = segments.lane_taken.map(|(lane, _)| usize::from(lane));
    taken
        .into_iter()
        .chain([0, 1])
        .find(|lane| segments.wave_zoom[*lane] > 0)
        .map(|lane| (lanes[lane], segments.wave_zoom[lane]))
}

/// The edit pictures of a segment edge taken with Ctrl+I/O, for the source timeline
/// (their frames are source frames of the clip it shows).
pub fn source_edit_stills(segments: &SegmentsView) -> Vec<TimelineEditStill<'_>> {
    segments.source_edit_stills.iter().map(|still| timeline_still(still, still.marker_frame)).collect()
}

/// One edit picture as the timeline draws it, at `frame` of that timeline.
pub(crate) fn timeline_still(still: &EditStill, frame: u64) -> TimelineEditStill<'_> {
    TimelineEditStill {
        frame,
        after: still.after,
        black: still.picture.is_none(),
        image: still.image.as_deref().map(|image| TimelineStillImage {
            id: &still.id,
            content_key: image.content_key,
            size: image.size,
            pixels: &image.pixels,
        }),
    }
}

/// The story's part of the source timeline: the A1/A2 channel pickers, the wave over
/// the video row and the pictures of a trimmed segment edge, in one call of the form.
pub trait StorySourceTimeline<'a> {
    fn story(self, segments: &'a SegmentsView) -> Self;
}

impl<'a> StorySourceTimeline<'a> for qnc_source_dock::SourceTimeline<'a> {
    fn story(self, segments: &'a SegmentsView) -> Self {
        self.with_a1_channel(source_lane_choice(segments, 0))
            .with_a2_channel(source_lane_choice(segments, 1))
            .with_wave_over_video(source_wave_over_video(segments))
            .with_edit_stills(source_edit_stills(segments))
    }
}
