//! XML-only verification harness. Stdin supplies snapshots; no files are opened.
use qnc_media_metadata::{inspect, StreamDetails};
use qnc_sony_metadata::*;
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, time::Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    index: XmlDocument,
    sidecars: Vec<SidecarDocument>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut text = String::new();
    std::io::stdin()
        .take(64 * 1024 * 1024 + 1)
        .read_to_string(&mut text)?;
    if text.len() > 64 * 1024 * 1024 {
        return Err("batch too large".into());
    }
    let input: Input = serde_json::from_str(&text)?;
    let start = Instant::now();
    let index = read_index(&input.index)?;
    let sidecars: BTreeMap<_, _> = input
        .sidecars
        .iter()
        .map(|s| (s.relative_path.as_str(), s))
        .collect();
    if sidecars.len() != input.sidecars.len() {
        return Err("duplicate sidecar input".into());
    }
    let mut proxy_count = 0;
    let mut original_frames = 0;
    let mut proxy_frames = 0;
    let mut creation_dates = 0;
    let mut complete = 0;
    let mut conflicts = 0;
    let mut notices = BTreeMap::<String, usize>::new();
    for (i, material) in index.materials.iter().enumerate() {
        // Synthetic QNC identities for XML-only validation, not live media bindings.
        let binding = ClipBinding {
            clip_id: format!("validation-{i}"),
            original: BoundMedia {
                relative_path: material.original.relative_path.clone(),
                media_uri: format!("qnc://local/media/validation-original-{i}"),
            },
            proxy: material.proxies.first().map(|p| BoundMedia {
                relative_path: p.relative_path.clone(),
                media_uri: format!("qnc://local/media/validation-proxy-{i}"),
            }),
        };
        let sidecar = material
            .related
            .iter()
            .filter(|r| r.kind == "XML")
            .find_map(|r| sidecars.get(r.relative_path.as_str()).copied())
            .ok_or("referenced sidecar missing from verification input")?;
        let result = read_metadata(&index, i, &binding, Some(sidecar))?;
        let count = |media: &qnc_media_metadata::MediaRepresentation| {
            media.streams.iter().any(|s| match &s.details {
                StreamDetails::Video(v) => v.exact_frame_count().is_some(),
                _ => false,
            })
        };
        original_frames += usize::from(count(&result.metadata.original));
        creation_dates += usize::from(result.metadata.original.tags.contains_key("creation_time"));
        if let Some(proxy) = &result.metadata.proxy {
            proxy_count += 1;
            proxy_frames += usize::from(count(proxy));
        }
        for notice in &result.notices {
            *notices.entry(notice.code.clone()).or_default() += 1;
        }
        conflicts += result
            .notices
            .iter()
            .filter(|n| n.code == "conflict")
            .count();
        complete += usize::from(inspect(&result.metadata).is_complete());
    }
    println!(
        "{}",
        serde_json::json!({
            "materials": index.materials.len(), "proxies": proxy_count,
            "originals_with_exact_frames": original_frames, "proxies_with_exact_frames": proxy_frames,
            "creation_dates": creation_dates, "complete_metadata_records": complete,
            "conflicts": conflicts, "notices": notices, "parser_elapsed_ms": start.elapsed().as_millis()
        })
    );
    Ok(())
}
