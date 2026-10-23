use super::*;

/// A playlist for an exact program-frame window (export of a part). Record
/// frames are rebased to zero; linked source ranges stay absolute.
pub fn build_program_frame_window(
    playlist: &FlatProgramPlaylist,
    record_range: ProgramFrameRange,
) -> Result<FlatProgramPlaylist, String> {
    playlist.validate().map_err(|error| error.message)?;
    let record_range = ProgramFrameRange::new(record_range.in_frame, record_range.out_frame)
        .map_err(|error| error.message)?;
    if record_range.out_frame > playlist.duration_frames {
        return Err(format!(
            "Program window zavrsava izvan programa: {} > {}.",
            record_range.out_frame, playlist.duration_frames
        ));
    }
    let mut items = Vec::new();
    for item in &playlist.items {
        let overlap_in = item.record_range.in_frame.max(record_range.in_frame);
        let overlap_out = item.record_range.out_frame.min(record_range.out_frame);
        if overlap_out <= overlap_in {
            continue;
        }
        let mut window_item = trim_program_item(item, overlap_in, overlap_out)?;
        window_item.record_range = ProgramFrameRange::new(
            overlap_in.saturating_sub(record_range.in_frame),
            overlap_out.saturating_sub(record_range.in_frame),
        )
        .map_err(|error| error.message)?;
        window_item.item_id = item_id(
            window_item.record_range.in_frame,
            window_item.record_range.out_frame,
        );
        items.push(window_item);
    }
    let result = FlatProgramPlaylist {
        playlist_id: format!(
            "{}:window:{}-{}",
            playlist.playlist_id, record_range.in_frame, record_range.out_frame
        ),
        duration_frames: record_range.frame_len(),
        items,
        ..playlist.clone()
    };
    result.validate().map_err(|error| error.message)?;
    Ok(result)
}

pub(crate) fn trim_program_item(
    item: &FlatProgramItem,
    record_in: i64,
    record_out: i64,
) -> Result<FlatProgramItem, String> {
    Ok(FlatProgramItem {
        item_id: item_id(record_in, record_out),
        record_range: ProgramFrameRange::new(record_in, record_out)
            .map_err(|error| error.message)?,
        sources: item
            .sources
            .iter()
            .map(|source| trim_flat_source(source, item.record_range, record_in, record_out))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(crate) fn retained_audio_source(
    source: &FlatProgramSource,
    item_range: ProgramFrameRange,
    record_in: i64,
    record_out: i64,
    replaced_audio_outputs: &[u16],
) -> Option<Result<FlatProgramSource, String>> {
    let audio_routes = source
        .audio_routes
        .iter()
        .copied()
        .filter(|route| !replaced_audio_outputs.contains(&route.output_channel))
        .collect::<Vec<_>>();
    if audio_routes.is_empty() {
        return None;
    }
    Some(
        trim_flat_source(source, item_range, record_in, record_out).map(|mut source| {
            source.video_layer = None;
            source.source_video_format = None;
            source.audio_routes = audio_routes;
            source
        }),
    )
}

pub(crate) fn trim_flat_source(
    source: &FlatProgramSource,
    origin_record_range: ProgramFrameRange,
    record_in: i64,
    record_out: i64,
) -> Result<FlatProgramSource, String> {
    let offset = record_in.saturating_sub(origin_record_range.in_frame);
    let source_in = source.source_range.source_in.saturating_add(offset);
    let source_out = source_in.saturating_add(record_out.saturating_sub(record_in));
    if source_out > source.source_range.source_out {
        return Err(format!(
            "Source '{}' does not have enough frames for the program range {}..{}.",
            source.source_id, record_in, record_out
        ));
    }
    let mut source = source.clone();
    source.source_range.source_in = source_in;
    source.source_range.source_out = source_out;
    Ok(source)
}
