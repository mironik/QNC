use super::*;

/// Applies a transient cutaway to an already flattened program snapshot.
///
/// The operation preserves the program A1 bus and replaces the visible video
/// and A2 bus only inside `record_range`. It does not change editorial data.
pub fn apply_transient_program_overlay(
    playlist: &FlatProgramPlaylist,
    overlay: &ProgramTransientOverlayInput,
) -> Result<FlatProgramPlaylist, String> {
    playlist.validate().map_err(|error| error.message)?;
    let overlay_range = ProgramFrameRange::new(
        overlay.record_range.in_frame,
        overlay.record_range.out_frame,
    )
    .map_err(|error| error.message)?;
    if overlay_range.out_frame > playlist.duration_frames {
        return Err(format!(
            "The temporary overlay ends outside the program: {} > {}.",
            overlay_range.out_frame, playlist.duration_frames
        ));
    }
    if overlay.source.video_layer != Some(ProgramVideoLayer::Cover) {
        return Err("The temporary overlay must be a B-roll picture source.".into());
    }
    if overlay.source.source_range.frame_len() != overlay_range.frame_len() {
        return Err("The temporary overlay's source and program ranges differ in length.".into());
    }
    let replaced_audio_outputs = overlay
        .source
        .audio_routes
        .iter()
        .map(|route| route.output_channel)
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    for item in &playlist.items {
        let item_start = item.record_range.in_frame;
        let item_end = item.record_range.out_frame;
        let overlap_start = item_start.max(overlay_range.in_frame);
        let overlap_end = item_end.min(overlay_range.out_frame);
        if overlap_end <= overlap_start {
            items.push(item.clone());
            continue;
        }
        if item_start < overlap_start {
            items.push(trim_program_item(item, item_start, overlap_start)?);
        }
        let mut sources = item
            .sources
            .iter()
            .filter_map(|source| {
                retained_audio_source(
                    source,
                    item.record_range,
                    overlap_start,
                    overlap_end,
                    &replaced_audio_outputs,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        sources.push(trim_flat_source(
            &overlay.source,
            overlay_range,
            overlap_start,
            overlap_end,
        )?);
        items.push(FlatProgramItem {
            item_id: item_id(overlap_start, overlap_end),
            record_range: ProgramFrameRange::new(overlap_start, overlap_end)
                .map_err(|error| error.message)?,
            sources,
        });
        if overlap_end < item_end {
            items.push(trim_program_item(item, overlap_end, item_end)?);
        }
    }
    let result = FlatProgramPlaylist {
        items,
        ..playlist.clone()
    };
    result.validate().map_err(|error| error.message)?;
    Ok(result)
}
