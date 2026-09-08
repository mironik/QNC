use super::{selection_config::SelectionConfig, ClipView, IngestWorkPlan, ThumbStatus};
use qnc_ingest_store::content::{Access, ContentTarget, ImportStatus, StoredClip};
use qnc_work_settings::SettingsReader;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct Loaded {
    pub plan: IngestWorkPlan,
    pub target: ContentTarget,
    pub clips: Option<Vec<ClipView>>,
    pub source: Option<(String, String, String, String)>,
}

pub(super) fn load(
    reader: &SettingsReader,
    config: Option<&SelectionConfig>,
    retained_workspace: Option<&str>,
) -> Result<Loaded, String> {
    let plan = reader
        .read()
        .map_err(|e| e.to_string())
        .and_then(IngestWorkPlan::from_settings)?;
    let target = ContentTarget::for_project(reader, &plan.settings)?;
    if retained_workspace == Some(plan.settings.workspace_db_uri.as_str()) {
        return Ok(Loaded {
            plan,
            target,
            clips: None,
            source: None,
        });
    }
    let mut db = target.open(Access::ReadWrite)?;
    let mut clips = Vec::new();
    let mut after = None;
    let mut sources = std::collections::BTreeMap::new();
    let readers: std::collections::BTreeMap<_, _> = config
        .into_iter()
        .flat_map(|c| &c.sources)
        .filter_map(|s| {
            s.reader()
                .ok()
                .map(|reader| (s.location.uri.clone(), reader))
        })
        .collect();
    loop {
        let page = db.list(after.clone())?;
        if page.is_empty() {
            break;
        }
        let next = page.last().unwrap().clip.id().to_string();
        if after.as_ref().is_some_and(|last| last >= &next) {
            return Err("Neispravno stranicenje Ingest baze.".into());
        }
        after = Some(next);
        for stored in page {
            sources.insert(
                stored.clip.source_uri.clone(),
                (
                    stored.clip.source_name.clone(),
                    stored.clip.serial_number.clone(),
                    stored.clip.volume_name.clone(),
                ),
            );
            let mut clip = view(&stored);
            if let Some(uri) = &stored.clip.thumbnail_uri {
                // Offline media must not hide already committed catalog/selection data.
                if let Ok(image) = thumbnail(&readers, uri) {
                    clip.thumb_status = ThumbStatus::Ready;
                    clip.thumb_image = Some(image);
                }
            }
            clips.push(clip);
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
        clips: Some(clips),
        source,
    })
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
