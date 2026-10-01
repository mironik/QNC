use super::*;

pub fn timeline_intent_to_ingest_intent(intent: TimelineIntent) -> Option<IngestIntent> {
    match intent {
        TimelineIntent::CueFrame(frame) => Some(IngestIntent::new(
            action_ids::INGEST_CUE_FRAME,
            IngestPayload::Frame(frame.min(i64::MAX as u64) as i64),
        )),
        // Ingest gives no A1 channel choice, so its timeline never asks for one.
        TimelineIntent::ToggleAudioExpand(_) | TimelineIntent::ChooseA1Channel(_) => None,
        TimelineIntent::SelectVirtual { .. }
        | TimelineIntent::SelectCover { .. }
        | TimelineIntent::SelectMarkerSlot { .. }
        | TimelineIntent::SelectMarker { .. } => None,
        TimelineIntent::None => None,
    }
}
