//! Opt-in read-only diagnostic using existing public project/input DB contracts.
use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder, DecoderConfig, VERSION};
use qnc_media_metadata::{MediaRepresentation, Rational, StreamDetails};
use qnc_media_stream::{LocalSource, MediaStream, SourceReference};
use qnc_player_input::InputReader;
use qnc_work_settings::SettingsReader;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    io::{Read, Seek},
    path::{Path, PathBuf},
    time::Instant,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn decode(
    source: &LocalSource,
    media: &MediaRepresentation,
    index: u32,
    start: Option<Rational>,
    limit: Option<u64>,
    midpoint: Option<u64>,
) -> Result<serde_json::Value> {
    let stream = MediaStream::local(source, &media.media_uri)?;
    let request = DecodeRequest {
        version: VERSION.into(),
        media: media.clone(),
        stream_index: index,
        start,
    };
    let mut decoder = Decoder::open(
        request,
        stream,
        DecoderConfig::new(qnc_ffmpeg_decode::FfmpegAdapter::new("ffmpeg")),
    )?;
    let pid = decoder.process_id();
    let begun = Instant::now();
    let mut packets = 0;
    let mut bytes = 0;
    let mut peak = 0_f32;
    let mut first = None;
    let mut last = None;
    let mut middle = None;
    let mut distinct = 0;
    let mut previous = None;
    let mut format = None;
    let mut first_hash = None;
    let mut first_packet_ms = None;
    let mut last_packet_ms = 0;
    while let Some(packet) = decoder.next_packet()? {
        let arrived_ms = begun.elapsed().as_millis();
        first_packet_ms.get_or_insert(arrived_ms);
        last_packet_ms = arrived_ms;
        if decoder.process_id() != pid {
            return Err("process changed during contiguous decode".into());
        }
        let hash = format!("{:x}", Sha256::digest(&packet.bytes));
        if previous.as_ref().is_some_and(|p| p != &hash) {
            distinct += 1;
        }
        previous = Some(hash.clone());
        if first_hash.is_none() {
            first_hash = Some(hash.clone());
        }
        if midpoint == Some(packet.ordinal) {
            middle = Some(hash);
        }
        if matches!(packet.format, DecodedFormat::Audio { .. }) {
            for chunk in packet.bytes.chunks_exact(4) {
                peak = peak.max(f32::from_le_bytes(chunk.try_into()?).abs());
            }
        }
        let timestamp = serde_json::json!({"pts":packet.pts,"time_base":packet.time_base});
        if first.is_none() {
            first = Some(timestamp.clone());
        }
        last = Some(timestamp);
        format = Some(packet.format);
        packets += 1;
        bytes += packet.bytes.len();
        if limit.is_some_and(|n| packets >= n) {
            decoder.cancel();
            break;
        }
    }
    if decoder.process_id().is_some() {
        return Err("decoder process not reaped".into());
    }
    Ok(
        serde_json::json!({"stream":index,"packets":packets,"bytes":bytes,"format":format,"first":first,"last":last,"first_hash":first_hash,"midpoint_hash":middle,"distinct_adjacent":distinct,"peak":peak,"first_packet_ms":first_packet_ms,"last_packet_ms":last_packet_ms,"elapsed_ms":begun.elapsed().as_millis(),"one_process":true,"process_reaped":true}),
    )
}
fn hash(stream: &mut MediaStream) -> Result<String> {
    stream.rewind()?;
    let mut sum = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        sum.update(&buffer[..n]);
    }
    Ok(format!("{:x}", sum.finalize()))
}
fn check_media(
    root: &Path,
    media: &MediaRepresentation,
    limit: Option<u64>,
    midpoint: Option<u64>,
) -> Result<(Vec<serde_json::Value>, LocalSource)> {
    let reference = SourceReference::from_uri(&media.media_uri)?;
    let source = LocalSource::new(reference.source_uri(), root)?;
    let mut bytes = MediaStream::local(&source, &media.media_uri)?;
    let before = hash(&mut bytes)?;
    let mut reports = Vec::new();
    for stream in &media.streams {
        if matches!(stream.details, StreamDetails::Other { .. }) {
            continue;
        }
        reports.push(decode(
            &source,
            media,
            stream.index.as_ref().ok_or("missing index")?.value,
            None,
            limit,
            midpoint,
        )?);
    }
    if hash(&mut bytes)? != before {
        return Err("source changed".into());
    }
    Ok((reports, source))
}
fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().ok_or("ROOT required")?);
    let owner = PathBuf::from(args.next().ok_or("OWNER_SOURCE_DIRECTORY required")?);
    let id = args
        .next()
        .ok_or("CLIP_ID required")?
        .into_string()
        .map_err(|_| "invalid clip id")?;
    let original = match args.next() {
        Some(v) if v == "--verify-original" => true,
        None => false,
        _ => return Err("unknown option".into()),
    };
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let settings_reader = SettingsReader::from_root(&root)?;
    let settings = settings_reader.read()?;
    let input = InputReader::new(settings_reader.clone()).load(&settings.workspace_db_uri, &id)?;
    let media = input.media()?;
    let video = input
        .layout
        .video
        .as_ref()
        .ok_or("video required for diagnostic seek")?;
    let midpoint = video.duration_frames / 2;
    let (selected, source) = check_media(&owner, media, None, Some(midpoint))?;
    let timestamp = Rational {
        numerator: i64::try_from(midpoint)?
            .checked_mul(video.timebase.fps_den)
            .ok_or("timestamp overflow")?,
        denominator: video.timebase.fps_num,
    };
    let seek = decode(
        &source,
        media,
        video.stream_index,
        Some(timestamp),
        Some(3),
        None,
    )?;
    let sequential = selected
        .iter()
        .find(|v| v["stream"] == video.stream_index)
        .ok_or("missing sequential video")?;
    let seek_matches = sequential["midpoint_hash"] == seek["first_hash"];
    if !seek_matches {
        return Err(
            "timestamp seek differs from corresponding sequential frame in this diagnostic clip"
                .into(),
        );
    }
    let original_report = if original {
        Some(check_media(&owner, &input.snapshot.metadata.original, Some(8), None)?.0)
    } else {
        None
    };
    if settings_reader.read()? != settings {
        return Err("project settings changed".into());
    }
    println!(
        "{}",
        serde_json::json!({"project":settings.project_name,"clip":id,"representation":input.representation,"selected":selected,"seek":seek,"seek_matches_sequential":seek_matches,"original_verification":original_report,"source_unchanged":true,"database_writes":0,"audio_device_output":false,"player_runtime":false})
    );
    Ok(())
}
