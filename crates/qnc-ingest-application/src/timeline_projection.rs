use super::*;

pub fn timeline_intent_to_ingest_intent(intent: TimelineIntent) -> Option<IngestIntent> {
    match intent {
        TimelineIntent::CueFrame(frame) => Some(IngestIntent::new(
            action_ids::INGEST_CUE_FRAME,
            IngestPayload::Frame(frame.min(i64::MAX as u64) as i64),
        )),
        TimelineIntent::ToggleAudioExpand(_) => None,
        TimelineIntent::SelectVirtual { .. }
        | TimelineIntent::SelectCover { .. }
        | TimelineIntent::SelectMarkerSlot { .. }
        | TimelineIntent::SelectMarker { .. } => None,
        TimelineIntent::None => None,
    }
}

pub(crate) fn playback_timeline_projection(playback: &qnc_player_client::View) -> TimelineProjection {
    qnc_player_timeline::projection_from_player_reply(playback.reply.as_ref())
}
