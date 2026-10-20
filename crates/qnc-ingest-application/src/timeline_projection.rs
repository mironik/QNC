use super::*;

/// A pointer on the source timeline is the mouse map's action (piece `qnc-mouse-map`), the
/// same as in every form (user rule 2026-10-20).
pub fn timeline_intent_to_ingest_intent(intent: TimelineIntent) -> Option<IngestIntent> {
    qnc_source_timeline::SourceTimeline::event_of(&intent)
        .map(|event| IngestIntent::new(event.action_id, event.value.map_or(IngestPayload::None, IngestPayload::Frame)))
}
