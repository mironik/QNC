use super::{selection_config::SelectionConfig, ClipView, IngestWorkPlan, ThumbStatus};
use qnc_ingest_store::content::{
    Access, CatalogStats, ContentTarget, ImportStatus, StoredClip, StoredClipSummary,
};
use qnc_work_settings::SettingsReader;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::SyncSender,
    Arc,
};

#[derive(Debug)]
pub(super) struct Loaded {
    pub plan: IngestWorkPlan,
    pub target: ContentTarget,
    pub stats: CatalogStats,
    pub clips: Option<Vec<ClipView>>,
    pub source: Option<(String, String, String, String)>,
}

#[derive(Debug)]
pub(super) enum ThumbnailEvent {
    Ready {
        clip_id: String,
        uri: String,
        image: Arc<qnc_image_assets::RgbaImage>,
    },
    Finished,
}

pub(super) fn load(
    reader: &SettingsReader,
    _config: Option<&SelectionConfig>,
    retained_workspace: Option<&str>,
    retained_stats: Option<&CatalogStats>,
) -> Result<Loaded, String> {
    let plan = reader
        .read()
        .map_err(|e| e.to_string())
        .and_then(IngestWorkPlan::from_settings)?;
    let target = ContentTarget::for_project(reader, &plan.settings)?;
    let mut db = target.open(Access::ReadOnly)?;
    let stats = db.stats()?;
    if retained_workspace == Some(plan.settings.workspace_db_uri.as_str())
        && retained_stats == Some(&stats)
    {
        return Ok(Loaded {
            plan,
            target,
            stats,
            clips: None,
            source: None,
        });
    }
    let mut clips = Vec::new();
    let mut after = None;
    let mut sources = std::collections::BTreeMap::new();
    loop {
        let page = db.list_summary(after.clone())?;
        if page.is_empty() {
            break;
        }
        let next = page.last().unwrap().clip_id.clone();
        if after.as_ref().is_some_and(|last| last >= &next) {
            return Err("Neispravno stranicenje Ingest baze.".into());
        }
        after = Some(next);
        for stored in page {
            sources.insert(
                stored.source_uri.clone(),
                (
                    stored.source_name.clone(),
                    stored.serial_number.clone(),
                    stored.volume_name.clone(),
                ),
            );
            clips.push(view_summary(&stored));
        }
    }
    clips.sort_by(|a, b| a.name.cmp(&b.name));
    let source = if sources.len() == 1 {
        sources
            .into_iter()
            .next()
            .map(|(uri, (name, serial, volume))| (uri, name, serial, volume))
    } else {
        None
    };
    Ok(Loaded {
        plan,
        target,
        stats,
        clips: Some(clips),
        source,
    })
}

pub(super) fn load_thumbnails(
    config: SelectionConfig,
    clips: Vec<(String, String)>,
    send: SyncSender<ThumbnailEvent>,
    cancel: Arc<AtomicBool>,
) {
    let readers: std::collections::BTreeMap<_, _> = config
        .sources
        .iter()
        .filter_map(|s| {
            s.reader()
                .ok()
                .map(|reader| (s.location.uri.clone(), reader))
        })
        .collect();
    for (clip_id, uri) in clips {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(image) = thumbnail(&readers, &uri) else {
            continue;
        };
        if send
            .send(ThumbnailEvent::Ready {
                clip_id,
                uri,
                image,
            })
            .is_err()
        {
            return;
        }
    }
    let _ = send.send(ThumbnailEvent::Finished);
}

pub(super) fn view(stored: &StoredClip) -> ClipView {
    let duration = stored
        .clip
        .snapshot
        .metadata
        .original
        .duration_seconds
        .as_ref()
        .map(|d| d.value.numerator as f64 / d.value.denominator as f64)
        .unwrap_or(0.0);
    ClipView {
        clip_id: stored.clip.id().into(),
        name: stored.clip.name.clone(),
        duration_seconds: duration,
        selected: stored.selected,
        imported: stored.import_status == ImportStatus::Imported,
        previously_seen: true,
        metadata_revision: stored.clip.snapshot.revision,
        thumb_uri: stored.clip.thumbnail_uri.clone(),
        thumb_status: if stored.clip.thumbnail_uri.is_some() {
            ThumbStatus::Pending
        } else {
            ThumbStatus::Missing
        },
        ..Default::default()
    }
}

pub(super) fn view_summary(stored: &StoredClipSummary) -> ClipView {
    ClipView {
        clip_id: stored.clip_id.clone(),
        name: stored.name.clone(),
        duration_seconds: stored.duration_seconds,
        selected: stored.selected,
        imported: stored.import_status == ImportStatus::Imported,
        previously_seen: true,
        metadata_revision: stored.revision,
        thumb_uri: stored.thumbnail_uri.clone(),
        thumb_status: if stored.thumbnail_uri.is_some() {
            ThumbStatus::Pending
        } else {
            ThumbStatus::Missing
        },
        ..Default::default()
    }
}

fn thumbnail(
    readers: &std::collections::BTreeMap<String, qnc_source_reader::SourceReader>,
    uri: &str,
) -> Result<Arc<qnc_image_assets::RgbaImage>, String> {
    let reference = qnc_source_reader::SourceReference::from_uri(uri).map_err(|e| e.to_string())?;
    let source = readers
        .get(reference.source_uri())
        .ok_or("Izvor postera nije dostupan.")?;
    let data = source
        .read_bytes(&reference, qnc_image_assets::MAX_BYTES as u64)
        .map_err(|e| e.to_string())?;
    qnc_image_assets::decode_thumbnail(&data.bytes)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}
