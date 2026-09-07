use crate::{xml, *};
use qnc_media_metadata::*;
use std::collections::BTreeMap;

pub fn read_metadata(
    index: &CameraIndex,
    material_index: usize,
    binding: &ClipBinding,
    sidecar: Option<&SidecarDocument>,
) -> Result<MetadataRead, String> {
    validate_uri(&index.document_uri)?;
    if binding.clip_id.trim().is_empty() {
        return Err("clip_id is required".into());
    }
    let material = index
        .materials
        .get(material_index)
        .ok_or("material index out of bounds")?;
    bind(&material.original, &binding.original)?;
    if material.proxies.len() > 1 {
        return Err("multiple proxies require an explicit representation contract".into());
    }
    let proxy_pair = match (material.proxies.first(), &binding.proxy) {
        (Some(media), Some(bound)) => {
            bind(media, bound)?;
            Some((media, bound))
        }
        (None, None) => None,
        _ => return Err(
            "proxy binding must match the indexed proxy; unresolved references cannot be dropped"
                .into(),
        ),
    };
    if binding
        .proxy
        .as_ref()
        .is_some_and(|p| p.media_uri == binding.original.media_uri)
    {
        return Err("original and proxy cannot share a media identity".into());
    }
    let mut notices = Vec::new();
    let mut evidence = vec![make_evidence(
        "index-original",
        &index.document_uri,
        &binding.original.media_uri,
    )];
    let mut original = representation(
        &material.original,
        &binding.original,
        "index-original",
        "original",
        &mut notices,
    );
    for (locator, value) in &index.properties {
        original.tags.insert(
            format!("sony.index:{locator}"),
            fact("index-original", locator, value.clone()),
        );
    }
    let mut proxy = proxy_pair.map(|(media, bound)| {
        evidence.push(make_evidence(
            "index-proxy",
            &index.document_uri,
            &bound.media_uri,
        ));
        representation(media, bound, "index-proxy", "proxy", &mut notices)
    });
    let mut normal_progressive = false;
    if let Some(sidecar) = sidecar {
        let path = relative_reference(&sidecar.relative_path)?;
        if !material
            .related
            .iter()
            .any(|r| r.kind == "XML" && r.relative_path == path)
        {
            return Err("sidecar is not explicitly related to this Material".into());
        }
        validate_uri(&sidecar.document.document_uri)?;
        let root = xml::parse(&sidecar.document.text, "NonRealTimeMeta", SIDECAR_NAMESPACE)?;
        let target = root
            .one("TargetMaterial")?
            .ok_or("missing TargetMaterial")?
            .required("umidRef")?;
        let original_umid = material
            .original
            .attributes
            .get("umid")
            .ok_or("missing original UMID")?;
        if !target.eq_ignore_ascii_case(original_umid) {
            return Err("sidecar UMID does not match the original".into());
        }
        evidence.push(make_evidence(
            "sidecar-original",
            &sidecar.document.document_uri,
            &binding.original.media_uri,
        ));
        normal_progressive = apply_sidecar(&root, &material.original, &mut original, &mut notices)?;
    }
    if normal_progressive {
        if let Some((camera, _)) = proxy_pair {
            if let Some(proxy) = &mut proxy {
                if video_ref(proxy)
                    .and_then(|v| v.frame_rate.as_ref())
                    .is_some()
                {
                    if let Some(count) = camera.attributes.get("dur") {
                        if let Some(count) = positive(count, "proxy.frame_count", &mut notices) {
                            promote_video_count(
                                proxy,
                                "index-proxy",
                                &format!("{}/@dur", camera.locator),
                                count,
                            );
                        }
                    }
                }
            }
        }
    } else {
        notice(&mut notices, "unresolved", "duration", "exact duration requires a supported normal progressive sidecar and consistent index facts");
    }
    notice(&mut notices, "incomplete", "streams", "XML channel descriptions do not prove container stream indices, timing or complete audio formats");
    Ok(MetadataRead {
        metadata: ClipMetadata {
            contract_id: CONTRACT_ID.into(),
            contract_version: CONTRACT_VERSION.into(),
            clip_id: binding.clip_id.clone(),
            evidence,
            original,
            proxy,
        },
        notices,
    })
}

fn bind(media: &IndexedMedia, bound: &BoundMedia) -> Result<(), String> {
    validate_uri(&bound.media_uri)?;
    if relative_reference(&bound.relative_path)? != media.relative_path {
        return Err("binding does not match indexed reference".into());
    }
    Ok(())
}

fn make_evidence(id: &str, document_uri: &str, media_uri: &str) -> Evidence {
    Evidence {
        id: id.into(),
        kind: EvidenceKind::CameraMetadata,
        document_uri: document_uri.into(),
        media_uri: media_uri.into(),
    }
}

fn fact<T>(id: &str, locator: &str, value: T) -> Fact<T> {
    Fact {
        value,
        evidence_id: id.into(),
        locator: locator.into(),
    }
}

fn notice(notices: &mut Vec<ReadNotice>, code: &str, field: &str, message: &str) {
    notices.push(ReadNotice {
        code: code.into(),
        field: field.into(),
        message: message.into(),
    });
}

fn representation(
    camera: &IndexedMedia,
    bound: &BoundMedia,
    id: &str,
    prefix: &str,
    notices: &mut Vec<ReadNotice>,
) -> MediaRepresentation {
    let mut media = MediaRepresentation {
        media_uri: bound.media_uri.clone(),
        container: None,
        duration_seconds: None,
        streams_complete: None,
        streams: Vec::new(),
        tags: BTreeMap::new(),
    };
    for (key, value) in &camera.attributes {
        let locator = format!("{}/@{key}", camera.locator);
        media.tags.insert(
            format!("sony.index:{locator}"),
            fact(id, &locator, value.clone()),
        );
    }
    if let Some(kind) = camera.attributes.get("type") {
        match kind.as_str() {
            "MXF" | "MP4" => {
                media.container = Some(fact(
                    id,
                    &format!("{}/@type", camera.locator),
                    kind.to_ascii_lowercase(),
                ))
            }
            _ => notice(
                notices,
                "unsupported",
                &format!("{prefix}.container"),
                "unmapped camera container label retained as raw fact",
            ),
        }
    }
    if let Some(codec) = camera.attributes.get("videoType") {
        ensure_video(&mut media);
        media.streams[0].codec = codec_fact(
            codec,
            id,
            &format!("{}/@videoType", camera.locator),
            &format!("{prefix}.codec"),
            notices,
        );
    }
    if let Some(fps) = camera.attributes.get("fps") {
        let video = ensure_video(&mut media);
        video.frame_rate = fps_fact(
            fps,
            id,
            &format!("{}/@fps", camera.locator),
            &format!("{prefix}.frame_rate"),
            notices,
        );
        if video.frame_rate.is_some() {
            video.scan_mode = Some(fact(
                id,
                &format!("{}/@fps", camera.locator),
                ScanMode::Progressive,
            ));
        }
    }
    media
}

fn codec_fact(
    value: &str,
    id: &str,
    locator: &str,
    field: &str,
    notices: &mut Vec<ReadNotice>,
) -> Option<Fact<Signal<String>>> {
    if value.starts_with("AVC50_") || value.starts_with("AVC_Proxy_") {
        Some(fact(id, locator, Signal::Known("h264".into())))
    } else {
        notice(
            notices,
            "unsupported",
            field,
            "unmapped video codec label retained as raw fact",
        );
        None
    }
}

fn progressive_rate(value: &str) -> Option<FrameTimebase> {
    let value = value.strip_suffix('p')?;
    let (integer, fraction) = value.split_once('.').unwrap_or((value, "0"));
    if integer.is_empty()
        || !integer.bytes().all(|c| c.is_ascii_digit())
        || fraction.is_empty()
        || !fraction.bytes().all(|c| c == b'0')
    {
        return None;
    }
    FrameTimebase::new(integer.parse().ok()?, 1).ok()
}

fn fps_fact(
    value: &str,
    id: &str,
    locator: &str,
    field: &str,
    notices: &mut Vec<ReadNotice>,
) -> Option<Fact<FrameTimebase>> {
    match progressive_rate(value) {
        Some(rate) => Some(fact(id, locator, rate)),
        None => {
            notice(
                notices,
                "unsupported",
                field,
                "FPS semantics outside verified integer-progressive mapping; raw value retained",
            );
            None
        }
    }
}

fn positive(value: &str, field: &str, notices: &mut Vec<ReadNotice>) -> Option<i64> {
    match value.parse::<i64>() {
        Ok(value) if value > 0 => Some(value),
        _ => {
            notice(
                notices,
                "invalid",
                field,
                "expected positive integer within signed 64-bit range",
            );
            None
        }
    }
}

fn merge<T: PartialEq>(
    target: &mut Option<Fact<T>>,
    incoming: Option<Fact<T>>,
    field: &str,
    notices: &mut Vec<ReadNotice>,
) {
    match (&target, &incoming) {
        (Some(a), Some(b)) if a.value != b.value => {
            *target = None;
            notice(
                notices,
                "conflict",
                field,
                "camera index and sidecar disagree; neither value is silently preferred",
            );
        }
        _ => *target = incoming,
    }
}

fn apply_sidecar(
    root: &xml::Node,
    camera: &IndexedMedia,
    media: &mut MediaRepresentation,
    notices: &mut Vec<ReadNotice>,
) -> Result<bool, String> {
    let id = "sidecar-original";
    let mut attributes = BTreeMap::new();
    raw_attributes(root, &mut attributes);
    for (locator, value) in attributes {
        media
            .tags
            .insert(format!("sony.sidecar:{locator}"), fact(id, &locator, value));
    }
    if let Some(date) = root
        .one("CreationDate")?
        .and_then(|n| n.attributes.get("value").map(|v| (n, v)))
    {
        media.tags.insert(
            "creation_time".into(),
            fact(id, &format!("{}/@value", date.0.locator), date.1.clone()),
        );
    }
    let frame = root.at(&["VideoFormat", "VideoFrame"])?;
    if let Some(frame) = frame {
        if let Some(codec) = frame.attributes.get("videoCodec") {
            ensure_video(media);
            let incoming = codec_fact(
                codec,
                id,
                &format!("{}/@videoCodec", frame.locator),
                "original.codec",
                notices,
            );
            merge(
                &mut media.streams[0].codec,
                incoming,
                "original.codec",
                notices,
            );
            if camera
                .attributes
                .get("videoType")
                .is_some_and(|c| c != codec)
            {
                media.streams[0].codec = None;
                notice(
                    notices,
                    "conflict",
                    "original.codec",
                    "raw camera video codec declarations disagree",
                );
            }
        }
        if let Some(fps) = frame.attributes.get("formatFps") {
            let incoming = fps_fact(
                fps,
                id,
                &format!("{}/@formatFps", frame.locator),
                "original.frame_rate",
                notices,
            );
            let video = ensure_video(media);
            merge(
                &mut video.frame_rate,
                incoming,
                "original.frame_rate",
                notices,
            );
            video.scan_mode = video.frame_rate.as_ref().map(|_| {
                fact(
                    id,
                    &format!("{}/@formatFps", frame.locator),
                    ScanMode::Progressive,
                )
            });
        }
    }
    if let Some(layout) = root.at(&["VideoFormat", "VideoLayout"])? {
        let video = ensure_video(media);
        for (attribute, target) in [
            ("pixel", &mut video.width),
            ("numOfVerticalLine", &mut video.height),
        ] {
            if let Some(value) = layout.attributes.get(attribute) {
                let value = positive(value, attribute, notices).and_then(|v| u32::try_from(v).ok());
                *target =
                    value.map(|value| fact(id, &format!("{}/@{attribute}", layout.locator), value));
            }
        }
    }
    let normal = root
        .one("RecordingMode")?
        .and_then(|n| n.attributes.get("type"))
        .is_some_and(|t| t == "normal");
    let format_rate = frame
        .and_then(|n| n.attributes.get("formatFps"))
        .and_then(|s| progressive_rate(s));
    let capture_rate = frame
        .and_then(|n| n.attributes.get("captureFps"))
        .and_then(|s| progressive_rate(s));
    let index_rate = camera
        .attributes
        .get("fps")
        .and_then(|s| progressive_rate(s));
    let supported = normal
        && format_rate.is_some()
        && format_rate == capture_rate
        && format_rate == index_rate
        && camera.attributes.get("offset").is_some_and(|o| o == "0");
    if !supported {
        return Ok(false);
    }
    let Some(duration) = root.one("Duration")? else {
        return Ok(false);
    };
    let Some(side_duration) = duration.attributes.get("value") else {
        return Ok(false);
    };
    let Some(index_duration) = camera.attributes.get("dur") else {
        return Ok(false);
    };
    let a = positive(side_duration, "original.frame_count", notices);
    let b = positive(index_duration, "original.frame_count", notices);
    let Some((a, b)) = a.zip(b) else {
        return Ok(false);
    };
    if a != b {
        notice(
            notices,
            "conflict",
            "original.frame_count",
            "Duration and index dur disagree",
        );
        return Ok(false);
    }
    promote_video_count(media, id, &format!("{}/@value", duration.locator), a);
    Ok(true)
}

fn promote_video_count(media: &mut MediaRepresentation, id: &str, locator: &str, count: i64) {
    // This adapter promotes only denominator=1 progressive rates.
    // XML dur/Duration counts video edit units, not the duration of all container streams.
    // Keep the exact video count and rate; audio padding can extend the proxy container.
    let video = ensure_video(media);
    video.frame_count = Some(fact(id, locator, FrameCount::Exact(count as u64)));
    video.frame_rate_mode = Some(fact(id, locator, FrameRateMode::Constant));
}

fn video_ref(media: &MediaRepresentation) -> Option<&VideoMetadata> {
    media.streams.iter().find_map(|s| match &s.details {
        StreamDetails::Video(v) => Some(v.as_ref()),
        _ => None,
    })
}

fn ensure_video(media: &mut MediaRepresentation) -> &mut VideoMetadata {
    if media.streams.is_empty() {
        media.streams.push(MediaStream {
            index: None,
            codec: None,
            profile: None,
            time_base: None,
            start_pts: None,
            duration_ts: None,
            details: StreamDetails::Video(Box::new(VideoMetadata {
                width: None,
                height: None,
                frame_rate: None,
                frame_rate_mode: None,
                frame_count: None,
                scan_mode: None,
                pixel_format: None,
                sample_aspect_ratio: None,
                rotation_degrees: None,
                color: ColorMetadata {
                    primaries: None,
                    transfer: None,
                    matrix: None,
                    range: None,
                },
            })),
        });
    }
    match &mut media.streams[0].details {
        StreamDetails::Video(v) => v,
        _ => unreachable!("only partial video descriptions are constructed here"),
    }
}
