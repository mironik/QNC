use super::*;

pub(super) fn timeline_placeholder_height() -> f32 {
    qnc_timeline::source_player_timeline_height()
}

pub(super) fn render_player_timeline(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let intent = qnc_timeline::show_source_player_timeline_with_artifacts(
        ui,
        rect,
        &view.timeline,
        timeline_theme(theme),
        view.timeline_filmstrip_background(),
        view.timeline_a1_peaks(),
        view.timeline_a2_peaks(),
        view.timeline_a3_peaks(),
        view.timeline_a4_peaks(),
    );
    timeline_intent_to_ingest_intent(intent)
}

pub(super) fn timeline_theme(theme: &Theme) -> TimelineTheme {
    TimelineTheme::from_qnc_theme(
        theme.bg,
        theme.surface,
        theme.surface_alt,
        theme.border_soft,
        theme.text,
        theme.text_muted,
        theme.accent,
    )
}
