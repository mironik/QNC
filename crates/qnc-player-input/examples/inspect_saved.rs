//! Read-only diagnostic. Does not open any source media or start playback.
use qnc_ingest_store::content::{Access, ContentTarget, PAGE_SIZE};
use qnc_player_input::{InputReader, PlayerClipRecord, PlayerContentRead};
use qnc_work_settings::SettingsReader;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
struct StorePlayerContentReader {
    target: ContentTarget,
}

impl PlayerContentRead for StorePlayerContentReader {
    fn read_clip(&self, clip_id: &str) -> Result<Option<PlayerClipRecord>, String> {
        let stored = self.target.open(Access::ReadOnly)?.read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
        }))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().ok_or("Usage: inspect_saved ROOT [CLIP_ID]")?);
    let clip_id = args
        .next()
        .map(|v| v.into_string().map_err(|_| "Invalid clip ID"))
        .transpose()?;
    if args.next().is_some() {
        return Err("Too many arguments".into());
    }
    let reader = SettingsReader::from_root(&root)?;
    let settings = reader.read()?;
    let content_target = ContentTarget::for_project(&reader, &settings)?;
    let inputs = InputReader::with_content_reader(
        reader.clone(),
        Arc::new(StorePlayerContentReader {
            target: content_target.clone(),
        }),
    );
    let mut ids = Vec::new();
    if let Some(id) = clip_id {
        ids.push(id);
    } else {
        let mut content = content_target.open(Access::ReadOnly)?;
        let mut after = None;
        loop {
            let rows = content.list(after.clone())?;
            if rows.is_empty() {
                break;
            }
            let last = rows.last().unwrap().clip.id().to_owned();
            if after.as_ref().is_some_and(|a| a >= &last) {
                return Err("Non-advancing catalog page".into());
            }
            ids.extend(rows.iter().map(|r| r.clip.id().to_owned()));
            if rows.len() < PAGE_SIZE {
                break;
            }
            after = Some(last);
        }
    }
    let mut failed = 0;
    for id in &ids {
        match inputs.load(&settings.workspace_db_uri, id) {
            Ok(input) => println!(
                "{}",
                serde_json::json!({
                    "clip_id":id, "representation":input.representation,
                    "media_uri":input.media()?.media_uri, "layout":input.layout,
                    "original_audio": input.snapshot.metadata.original.streams.iter().filter(|s| matches!(s.details, qnc_media_metadata::StreamDetails::Audio(_))).collect::<Vec<_>>(),
                    "proxy_audio": input.snapshot.metadata.proxy.as_ref().map(|media| media.streams.iter().filter(|s| matches!(s.details, qnc_media_metadata::StreamDetails::Audio(_))).collect::<Vec<_>>()),
                })
            ),
            Err(error) => {
                failed += 1;
                eprintln!("{id}: {error}");
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"project_name":settings.project_name,"read":ids.len(),"failed":failed,"media_opened":0,"database_writes":0})
    );
    if failed != 0 {
        return Err("Some stored clips could not be prepared".into());
    }
    Ok(())
}
