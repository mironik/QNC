//! The selected clips that do not serve what the project uses, and the choice offered for them.

use crate::{InTheWay, Request, CANCEL_IMPORT, SKIP_CONFLICTS};
use qnc_choice_box::{Choice, ChoiceOption};
use qnc_import_fit::{Conflict, Needs, Playback};
use qnc_ingest_store::content::Access;


/// The selected clips that do not serve what the project uses.
pub(crate) fn in_the_way(request: &Request) -> Result<Vec<InTheWay>, String> {
    let mut client = request.target.open(Access::ReadOnly).map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    let mut after = None;
    loop {
        let page = client.list(after.clone()).map_err(|e| e.to_string())?;
        let Some(last) = page.last() else { break };
        after = Some(last.clip.id().to_string());
        for stored in &page {
            let id = stored.clip.id().to_string();
            if request.selected.contains(&id) {
                if let Some(conflict) = qnc_import_fit::conflict(&stored.clip.snapshot, request.needs) {
                    found.push((id, stored.clip.name.clone(), conflict));
                }
            }
        }
    }
    Ok(found)
}

/// The info box for the clips in the way: what is in the way, how it is solved in the
/// project's settings, and the answers.
pub(crate) fn choice_of(found: &[InTheWay], needs: Needs, selected: usize) -> Choice {
    let key = |conflict: &Conflict| match conflict {
        Conflict::ProxyMissing => "ingest.choice.proxy_missing",
        Conflict::ProxyIncomplete { .. } => "ingest.choice.proxy_incomplete",
        Conflict::OriginalIncomplete { .. } => "ingest.choice.original_incomplete",
    };
    let mut keys: Vec<&str> = found.iter().map(|(_, _, conflict)| key(conflict)).collect();
    keys.sort_unstable();
    keys.dedup();
    let message = keys.iter().map(|key| qnc_ui_text::text(key).to_string()).collect::<Vec<_>>().join(" ");
    let proxy_in_the_way = found.iter().any(|(_, _, c)| !matches!(c, Conflict::OriginalIncomplete { .. }));
    let solution = match needs.playback {
        _ if !proxy_in_the_way => "ingest.choice.solve_original_incomplete",
        Playback::Proxy => "ingest.choice.solve_proxy",
        Playback::ProxyIfAvailable => "ingest.choice.solve_proxy_if_available",
        Playback::Original => "ingest.choice.solve_copy_proxy",
    };
    let items = found
        .iter()
        .map(|(_, name, conflict)| match conflict {
            Conflict::ProxyMissing => name.clone(),
            Conflict::ProxyIncomplete { missing } | Conflict::OriginalIncomplete { missing } => format!("{name}: {}", missing.join(", ")),
        })
        .collect();
    let mut options = Vec::new();
    if selected > found.len() {
        let others = (selected - found.len()).to_string();
        options.push(ChoiceOption { action_id: SKIP_CONFLICTS.into(), label: qnc_ui_text::fill("ingest.choice.import_others", &[("count", &others)]), strong: true });
    }
    options.push(ChoiceOption { action_id: CANCEL_IMPORT.into(), label: qnc_ui_text::text("ingest.choice.cancel").into(), strong: options.is_empty() });
    Choice { title: qnc_ui_text::text("ingest.choice.title").into(), message, items, solution: qnc_ui_text::text(solution).into(), options }
}
