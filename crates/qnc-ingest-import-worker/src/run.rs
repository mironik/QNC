use crate::*;
use std::{path::Path, sync::atomic::{AtomicBool, Ordering}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub clip_id: String,
    pub result: Result<String, String>,
    /// The poster copied into the project together with the media, if any.
    pub thumbnail_uri: Option<String>,
}

/// Claims the next queued clip, imports it and records the outcome. `None` when
/// the queue is empty. A failed import is an outcome (recorded as failed), not an
/// error of the executor.
pub fn run_next(
    queue: &mut dyn ImportQueue,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<Option<Outcome>, String> {
    let Some(clip) = queue.claim_next()? else {
        return Ok(None);
    };
    let clip_id = clip.clip.id().to_string();
    let imported = import_media(&clip, plan, project_dir, opener, cancel, &mut || {
        let _ = queue.heartbeat(&clip_id);
    });
    let mut thumbnail_uri = None;
    let result = match imported {
        Ok(imported) => {
            thumbnail_uri = import_poster(&clip, plan, project_dir, opener, cancel);
            queue.finish_import(
                clip_id.clone(),
                Some(imported.media_uri.clone()),
                thumbnail_uri.clone(),
                imported.copy_of,
                imported.optimized,
                None,
            )?;
            Ok(imported.media_uri)
        }
        Err(error) => {
            let message: String = error.chars().take(4000).collect();
            queue.finish_import(clip_id.clone(), None, None, None, None, Some(message))?;
            Err(error)
        }
    };
    Ok(Some(Outcome {
        clip_id,
        result,
        thumbnail_uri,
    }))
}

/// Runs until the queue is empty or the import is cancelled.
pub fn drain(
    queue: &mut dyn ImportQueue,
    plan: &IngestWorkPlan,
    project_dir: &Path,
    opener: &dyn MediaOpener,
    cancel: &AtomicBool,
) -> Result<Vec<Outcome>, String> {
    let mut outcomes = Vec::new();
    while !cancel.load(Ordering::Relaxed) {
        match run_next(queue, plan, project_dir, opener, cancel)? {
            Some(outcome) => outcomes.push(outcome),
            None => break,
        }
    }
    Ok(outcomes)
}
