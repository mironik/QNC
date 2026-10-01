use super::*;

pub fn timeline_intent_to_ingest_intent(intent: TimelineIntent) -> Option<IngestIntent> {
    match intent {
        cue @ (TimelineIntent::CueFrame(_) | TimelineIntent::GoHome) => Some(IngestIntent::new(
            action_ids::INGEST_CUE_FRAME, // V is Home: frame 0
            IngestPayload::Frame(if let TimelineIntent::CueFrame(frame) = cue { frame.min(i64::MAX as u64) as i64 } else { 0 }),
        )),
        // Ingest gives no A1/A2 channel choice, so its timeline never asks for one.
        TimelineIntent::ToggleAudioExpand(_) | TimelineIntent::ChooseA1Channel(_) | TimelineIntent::ChooseA2Channel(_) | TimelineIntent::TakeAudioLane(_) => None,
        TimelineIntent::SelectVirtual { .. }
        | TimelineIntent::SelectCover { .. }
        | TimelineIntent::SelectMarkerSlot { .. }
        | TimelineIntent::SelectMarker { .. } => None,
        TimelineIntent::None => None,
    }
}
