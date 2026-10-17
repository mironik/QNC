use super::*;

pub fn timeline_intent_to_ingest_intent(intent: TimelineIntent) -> Option<IngestIntent> {
    match intent {
        cue @ (TimelineIntent::CueFrame(_) | TimelineIntent::GoHome) => Some(IngestIntent::new(
            action_ids::INGEST_CUE_FRAME, // V is Home: frame 0
            IngestPayload::Frame(if let TimelineIntent::CueFrame(frame) = cue { frame.min(i64::MAX as u64) as i64 } else { 0 }),
        )),
        // The wheel and a click on A1/A2 or a channel are the source timeline's own actions.
        other => qnc_source_timeline::SourceTimeline::action_of(&other)
            .map(|(action, value)| IngestIntent::new(action, value.map_or(IngestPayload::None, IngestPayload::Frame))),
    }
}
