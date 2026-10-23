//! The selection written once through the content write transport, on the writer's thread.

use crate::{conflicts::in_the_way, Applied, Request, Stop};
use qnc_ingest_store::content::ContentWriteTransport;
use std::time::Duration;

pub(crate) fn write(mut request: Request) -> Result<Applied, Stop> {
    let found = in_the_way(&request).map_err(Stop::Failed)?;
    if !found.is_empty() {
        if !request.skip_conflicts {
            return Err(Stop::Conflicts(found));
        }
        request.selected.retain(|id| !found.iter().any(|(clip, _, _)| clip == id));
        request.unselected.extend(found.into_iter().map(|(id, _, _)| id));
    }
    let Request { target, selected, unselected, needs, .. } = request;
    let mut transport = ContentWriteTransport::start(target).map_err(|e| Stop::Failed(e.to_string()))?;
    if !unselected.is_empty() {
        run(&mut transport, "unselect", |t, key| t.select(key, unselected.clone(), false)).map_err(Stop::Failed)?;
    }
    if !selected.is_empty() {
        run(&mut transport, "select", |t, key| t.select(key, selected.clone(), true)).map_err(Stop::Failed)?;
        // The selected clips wait in the queue for the background application.
        run(&mut transport, "queue", |t, key| t.queue_selected(key, needs)).map_err(Stop::Failed)?;
    }
    Ok(Applied { selected, unselected })
}

fn run<E: ToString>(
    transport: &mut ContentWriteTransport,
    name: &str,
    send: impl FnOnce(&mut ContentWriteTransport, String) -> Result<(), E>,
) -> Result<(), String> {
    let key = format!("selection-{name}");
    send(transport, key.clone()).map_err(|e| e.to_string())?;
    loop {
        for completion in transport.poll() {
            if completion.key == key {
                completion.result.map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
        if !transport.has_pending() {
            return Err(qnc_ui_text::text("db.content_writer_returned_result").into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
