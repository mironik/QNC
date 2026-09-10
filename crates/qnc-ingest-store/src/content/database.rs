use super::*;
use qnc_media_metadata::{MediaRepresentation, Signal, StreamDetails};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::{path::Path, time::Duration};

pub struct ContentStore {
    conn: Connection,
    uri: String,
    access: Access,
    schema_ready: bool,
    has_thumbnail_uri: bool,
}

impl ContentStore {
    /// Bind the owned Ingest schema, including inside an existing project DB.
    pub fn open_owner_binding(file: &Path, uri: &str, access: Access) -> Result<Self> {
        let id = project_id(uri)?;
        if file.exists() {
            let check =
                Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(err)?;
            check.busy_timeout(Duration::from_secs(5)).map_err(err)?;
            validate_container(&check, &id)?;
            drop(check);
            if access == Access::ReadWrite {
                enable_owner_write(file)?;
            }
        }
        let flags = if access == Access::ReadOnly {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        };
        let mut conn = Connection::open_with_flags(file, flags).map_err(err)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        if access == Access::ReadWrite {
            // Set the connection's journal policy before any schema read/recovery.
            // The owner directory denies deletion; keep the rollback journal.
            let mode: String = conn
                .pragma_query_value(None, "journal_mode", |r| r.get(0))
                .map_err(err)?;
            if !mode.eq_ignore_ascii_case("wal") {
                conn.pragma_update(None, "journal_mode", "PERSIST")
                    .map_err(|e| format!("Ingest journal: {e:?}"))?;
            }
            conn.pragma_update(None, "synchronous", "FULL")
                .map_err(err)?;
        }
        let project_settings: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name IN ('project_settings','public_project_settings'))",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        validate_container(&conn, &id)?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(err)?;
        let mut schema: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='ingest_content_schema')",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !schema && access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            let schema_exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='ingest_content_schema')",
                    [],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if !schema_exists {
                let tables: u32 = tx.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                    [], |row| row.get(0),
                ).map_err(err)?;
                if tables != 0 && !project_settings {
                    return Err(
                        "Datoteka vec sadrzi drugu shemu; nema migracije ni preuzimanja baze."
                            .into(),
                    );
                }
                tx.execute_batch(include_str!("schema.sql"))
                    .map_err(|e| format!("Ingest schema: {e:?}"))?;
                tx.execute(
                    "INSERT INTO ingest_content_schema VALUES (?1,?2)",
                    params![SCHEMA_VERSION, id],
                )
                .map_err(err)?;
            }
            tx.commit()
                .map_err(|e| format!("Ingest schema commit: {e:?}"))?;
            schema = true;
        }
        if schema {
            let (version, stored_project): (String, String) = conn
                .query_row(
                    "SELECT version,project_id FROM ingest_content_schema",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(err)?;
            if version != SCHEMA_VERSION {
                return Err("Nepodrzana Ingest shema; migracije nisu dopustene.".into());
            }
            if stored_project != id {
                return Err("Ingest baza pripada drugom projektu.".into());
            }
        }
        if access == Access::ReadWrite {
            ensure_summary_columns(&conn)?;
        }
        let has_thumbnail_uri = schema && has_column(&conn, "clips", "thumbnail_uri")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
            use rusqlite::hooks::{AuthAction as A, Authorization as R};
            match ctx.action {
                A::Insert { table_name }
                | A::Delete { table_name }
                | A::Update { table_name, .. }
                    if access == Access::ReadWrite && owned_table(table_name) =>
                {
                    R::Allow
                }
                A::Read { .. }
                | A::Select
                | A::Transaction { .. }
                | A::Savepoint { .. }
                | A::Function { .. }
                | A::Recursive => R::Allow,
                _ => R::Deny,
            }
        }));
        Ok(Self {
            conn,
            uri: uri.into(),
            access,
            schema_ready: schema,
            has_thumbnail_uri,
        })
    }

    pub fn execute(&mut self, request: &Request) -> Result<Data> {
        if request.version != VERSION || request.db_uri != self.uri {
            return Err("Neispravan DB ugovor.".into());
        }
        if request.operation.is_write() && self.access == Access::ReadOnly {
            return Err("Pristup je read-only.".into());
        }
        if !self.schema_ready {
            return match &request.operation {
                Operation::List { .. } => Ok(Data::Clips(Vec::new())),
                Operation::ListSummary { .. } => Ok(Data::ClipSummaries(Vec::new())),
                Operation::Stats => Ok(Data::CatalogStats(CatalogStats::default())),
                Operation::Inventory { source_uri, .. } => {
                    let source = qnc_contracts::parse_qnc_uri(source_uri).map_err(err)?;
                    if source.resource_kind != "source" {
                        return Err("Neispravan izvor.".into());
                    }
                    Ok(Data::Inventory(Vec::new()))
                }
                Operation::Read { clip_id } => {
                    qnc_media_records::valid_id(clip_id).map_err(err)?;
                    Ok(Data::Clip(None))
                }
                _ => Err("Ingest shema nije inicijalizirana.".into()),
            };
        }
        match &request.operation {
            Operation::Publish(clip) => self.publish(clip),
            Operation::PublishBatch(clips) => {
                if clips.is_empty() || clips.len() > 16 {
                    return Err("Neispravna velicina upisnog paketa.".into());
                }
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                for clip in clips {
                    Self::write_clip(&tx, clip)?;
                }
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::Inventory { source_uri, after } => {
                let source = qnc_contracts::parse_qnc_uri(source_uri).map_err(err)?;
                if source.resource_kind != "source" {
                    return Err("Neispravan izvor.".into());
                }
                let mut stmt = self.conn.prepare(
                    "SELECT clip_id,source_uri,original_uri,revision,final FROM clips WHERE source_uri=?1 AND clip_id>?2 ORDER BY clip_id LIMIT ?3"
                ).map_err(err)?;
                let rows = stmt
                    .query_map(
                        params![source_uri, after.as_deref().unwrap_or(""), PAGE_SIZE],
                        |r| {
                            Ok(InventoryClip {
                                clip_id: r.get(0)?,
                                source_uri: r.get(1)?,
                                original_uri: r.get(2)?,
                                revision: r.get(3)?,
                                final_record: r.get(4)?,
                            })
                        },
                    )
                    .map_err(err)?;
                Ok(Data::Inventory(
                    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?,
                ))
            }
            Operation::RemoveMissing { clips } => {
                if clips.len() > 4096 {
                    return Err("Previse klipova u naredbi.".into());
                }
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                let mut removed = Vec::new();
                for clip in clips {
                    let matches: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM clips WHERE clip_id=?1 AND source_uri=?2 AND original_uri=?3 AND revision=?4 AND import_status IN ('detected','failed'))",
                        params![clip.clip_id,clip.source_uri,clip.original_uri,clip.revision], |r| r.get(0),
                    ).map_err(err)?;
                    if !matches {
                        continue;
                    }
                    for table in [
                        "clip_sources",
                        "clip_proxy",
                        "probe_records",
                        "filmstrip_artifacts",
                        "wave_artifacts",
                    ] {
                        tx.execute(
                            &format!("DELETE FROM {table} WHERE clip_id=?1"),
                            [&clip.clip_id],
                        )
                        .map_err(err)?;
                    }
                    tx.execute("DELETE FROM clips WHERE clip_id=?1", [&clip.clip_id])
                        .map_err(err)?;
                    removed.push(clip.clip_id.clone());
                }
                tx.commit().map_err(err)?;
                Ok(Data::Removed(removed))
            }
            Operation::Read { clip_id } => {
                qnc_media_records::valid_id(clip_id).map_err(err)?;
                let clip = self.conn.query_row(
                    "SELECT catalog_json,selected,import_status,import_error,imported_media_uri FROM clips WHERE clip_id=?1",
                    [clip_id], row,
                ).optional().map_err(err)?;
                Ok(Data::Clip(clip.map(Box::new)))
            }
            Operation::List { after } => {
                let mut statement = self
                    .conn
                    .prepare(
                        "SELECT catalog_json,selected,import_status,import_error,imported_media_uri
                     FROM clips WHERE clip_id > ?1 ORDER BY clip_id LIMIT ?2",
                    )
                    .map_err(err)?;
                let rows = statement
                    .query_map(params![after.as_deref().unwrap_or(""), PAGE_SIZE], row)
                    .map_err(err)?;
                Ok(Data::Clips(
                    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?,
                ))
            }
            Operation::ListSummary { after } => {
                let thumbnail_expr = if self.has_thumbnail_uri {
                    "c.thumbnail_uri"
                } else {
                    "NULL"
                };
                let sql = format!(
                    "SELECT c.clip_id,c.name,c.source_uri,s.source_name,s.serial_number,
                        s.volume_name,{thumbnail_expr},c.duration_seconds,c.selected,
                        c.import_status,c.import_error,c.imported_media_uri,c.revision,c.final
                     FROM clips c
                     JOIN clip_sources s ON s.clip_id=c.clip_id
                     WHERE c.clip_id > ?1
                     ORDER BY c.clip_id
                     LIMIT ?2"
                );
                let mut statement = self.conn.prepare(&sql).map_err(err)?;
                let rows = statement
                    .query_map(
                        params![after.as_deref().unwrap_or(""), PAGE_SIZE],
                        summary_row,
                    )
                    .map_err(err)?;
                Ok(Data::ClipSummaries(
                    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?,
                ))
            }
            Operation::Stats => self.catalog_stats().map(Data::CatalogStats),
            Operation::Select { clip_ids, selected } => {
                if clip_ids.len() > 4096 {
                    return Err("Previse klipova u naredbi.".into());
                }
                let tx = self.conn.transaction().map_err(err)?;
                for id in clip_ids {
                    if tx
                        .execute(
                            "UPDATE clips SET selected=?1 WHERE clip_id=?2",
                            params![selected, id],
                        )
                        .map_err(err)?
                        != 1
                    {
                        return Err("Clip nije pronadjen u projektnoj bazi.".into());
                    }
                }
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::QueueSelected => {
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                let mut stmt = tx.prepare("SELECT catalog_json,selected,import_status,import_error,imported_media_uri FROM clips WHERE selected != 0").map_err(err)?;
                let clips = stmt
                    .query_map([], row)
                    .map_err(err)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(err)?;
                drop(stmt);
                if clips.is_empty() {
                    return Err("Nema odabranih klipova.".into());
                }
                if clips.iter().any(|c| !ready(c)) {
                    return Err(
                        "Odabrani klip nema zavrsene metapodatke u bazi. Novi probe nije dopusten."
                            .into(),
                    );
                }
                tx.execute("UPDATE clips SET import_status='queued',import_error=NULL WHERE selected != 0 AND import_status IN ('detected','failed')", []).map_err(err)?;
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::ClaimNext => {
                let tx = self
                    .conn
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                    .map_err(err)?;
                let mut clip = tx.query_row(
                    "SELECT catalog_json,selected,import_status,import_error,imported_media_uri FROM clips WHERE import_status='queued' ORDER BY clip_id LIMIT 1",
                    [], row).optional().map_err(err)?;
                if let Some(clip) = &mut clip {
                    tx.execute(
                        "UPDATE clips SET import_status='processing' WHERE clip_id=?1",
                        [clip.clip.id()],
                    )
                    .map_err(err)?;
                    clip.import_status = ImportStatus::Processing;
                }
                tx.commit().map_err(err)?;
                Ok(Data::Claimed(clip.map(Box::new)))
            }
            Operation::FinishImport {
                clip_id,
                media_uri,
                error,
            } => {
                if media_uri.is_some() == error.is_some() {
                    return Err("Nedostaje ishod importa.".into());
                }
                if let Some(uri) = media_uri {
                    qnc_contracts::parse_qnc_uri(uri).map_err(err)?;
                }
                if error.as_ref().is_some_and(|s| s.len() > 4096) {
                    return Err("Prevelika poruka greske.".into());
                }
                let status = if error.is_some() {
                    "failed"
                } else {
                    "imported"
                };
                let n = self.conn.execute(
                    "UPDATE clips SET import_status=?1,imported_media_uri=?2,import_error=?3 WHERE clip_id=?4 AND import_status='processing'",
                    params![status,media_uri,error,clip_id]).map_err(err)?;
                if n != 1 {
                    return Err("Import posao nije preuzet ili je vec zavrsen.".into());
                }
                Ok(Data::Changed)
            }
        }
    }

    fn catalog_stats(&self) -> Result<CatalogStats> {
        let thumbnail_expr = if self.has_thumbnail_uri {
            "thumbnail_uri"
        } else {
            "NULL"
        };
        let sql = format!(
            "SELECT clip_id,revision,selected,import_status,import_error,
                imported_media_uri,{thumbnail_expr},source_uri,original_uri
             FROM clips
             ORDER BY clip_id"
        );
        let mut statement = self.conn.prepare(&sql).map_err(err)?;
        let mut rows = statement.query([]).map_err(err)?;
        let mut stats = CatalogStats {
            fingerprint: CATALOG_FINGERPRINT_OFFSET,
            ..Default::default()
        };
        while let Some(row) = rows.next().map_err(err)? {
            let clip_id: String = row.get(0).map_err(err)?;
            let revision_raw: i64 = row.get(1).map_err(err)?;
            let revision = u32::try_from(revision_raw)
                .map_err(|_| "Neispravna revizija klipa u Ingest bazi.")?;
            let selected: bool = row.get(2).map_err(err)?;
            let import_status: String = row.get(3).map_err(err)?;
            let import_error: Option<String> = row.get(4).map_err(err)?;
            let imported_media_uri: Option<String> = row.get(5).map_err(err)?;
            let thumbnail_uri: Option<String> = row.get(6).map_err(err)?;
            let source_uri: String = row.get(7).map_err(err)?;
            let original_uri: String = row.get(8).map_err(err)?;

            stats.clip_count += 1;
            if selected {
                stats.selected_count += 1;
            }
            stats.revision_sum = stats.revision_sum.wrapping_add(revision as u64);
            stats.max_revision = stats.max_revision.max(revision);
            fingerprint_str(&mut stats.fingerprint, &clip_id);
            fingerprint_u64(&mut stats.fingerprint, revision as u64);
            fingerprint_u64(&mut stats.fingerprint, if selected { 1 } else { 0 });
            fingerprint_str(&mut stats.fingerprint, &import_status);
            fingerprint_opt_str(&mut stats.fingerprint, import_error.as_deref());
            fingerprint_opt_str(&mut stats.fingerprint, imported_media_uri.as_deref());
            fingerprint_opt_str(&mut stats.fingerprint, thumbnail_uri.as_deref());
            fingerprint_str(&mut stats.fingerprint, &source_uri);
            fingerprint_str(&mut stats.fingerprint, &original_uri);
        }
        Ok(stats)
    }

    fn publish(&mut self, clip: &CatalogClip) -> Result<Data> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        Self::write_clip(&tx, clip)?;
        let saved = tx.query_row("SELECT catalog_json,selected,import_status,import_error,imported_media_uri FROM clips WHERE clip_id=?1",[clip.id()],row).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Saved(Box::new(saved)))
    }

    fn write_clip(tx: &rusqlite::Transaction<'_>, clip: &CatalogClip) -> Result<()> {
        clip.validate()?;
        let json = serde_json::to_string(clip).map_err(err)?;
        if json.len() > MAX_BYTES / PAGE_SIZE {
            return Err("Prevelik zapis klipa.".into());
        }
        let old: Option<(i64, bool, String)> = tx
            .query_row(
                "SELECT revision,final,catalog_json FROM clips WHERE clip_id=?1",
                [clip.id()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(err)?;
        if let Some((revision, final_record, old_json)) = old {
            if old_json == json {
                return Ok(());
            }
            let old: CatalogClip = serde_json::from_str(&old_json).map_err(err)?;
            if old.source_uri != clip.source_uri || old.snapshot.binding != clip.snapshot.binding {
                return Err("Postojeci clip_id ne smije preuzeti drugi izvor ili medij.".into());
            }
            if revision > clip.snapshot.revision as i64
                || (final_record && old.snapshot != clip.snapshot)
            {
                return Err("Zavrseni probe zapis se ne smije zamijeniti.".into());
            }
        }
        let original = &clip.snapshot.metadata.original;
        let video = original.streams.iter().find_map(|s| match &s.details {
            StreamDetails::Video(v) => Some(v.as_ref()),
            _ => None,
        });
        let duration = original
            .duration_seconds
            .as_ref()
            .map(|d| d.value.numerator as f64 / d.value.denominator as f64);
        let fps = video.and_then(|v| v.frame_rate.as_ref()).map(|f| &f.value);
        tx.execute("INSERT INTO clips(clip_id,source_uri,original_uri,name,created_at_utc,duration_seconds,duration_frames,fps_num,fps_den,thumbnail_uri,catalog_json,revision,final)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
            ON CONFLICT(clip_id) DO UPDATE SET name=excluded.name,catalog_json=excluded.catalog_json,revision=excluded.revision,final=excluded.final,
            duration_seconds=excluded.duration_seconds,duration_frames=excluded.duration_frames,fps_num=excluded.fps_num,fps_den=excluded.fps_den,
            created_at_utc=excluded.created_at_utc,thumbnail_uri=excluded.thumbnail_uri",
            params![clip.id(),clip.source_uri,original.media_uri,clip.name,original.tags.get("creation_time").map(|f| &f.value),duration,
                video.and_then(|v|v.exact_frame_count()),fps.map(|f|f.fps_num),fps.map(|f|f.fps_den),clip.thumbnail_uri.as_deref(),json,
                clip.snapshot.revision,clip.snapshot.phase == Phase::Final]).map_err(err)?;
        tx.execute("INSERT INTO clip_sources VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(clip_id) DO UPDATE SET original_container=excluded.original_container,original_codec=excluded.original_codec,serial_number=excluded.serial_number,volume_name=excluded.volume_name,source_name=excluded.source_name",
            params![clip.id(),clip.source_uri,original.media_uri,original.container.as_ref().map(|v| &v.value),codec(original),clip.serial_number,clip.volume_name,clip.source_name]).map_err(err)?;
        if let Some(proxy) = &clip.snapshot.metadata.proxy {
            tx.execute("INSERT INTO clip_proxy VALUES(?1,?2,?3,?4) ON CONFLICT(clip_id) DO UPDATE SET proxy_container=excluded.proxy_container,proxy_codec=excluded.proxy_codec",
                params![clip.id(),proxy.media_uri,proxy.container.as_ref().map(|v| &v.value),codec(proxy)]).map_err(err)?;
        }
        tx.execute("INSERT INTO probe_records VALUES(?1,?2,?3,?4,?5) ON CONFLICT(clip_id) DO UPDATE SET probe_json=excluded.probe_json,probed_at_utc=excluded.probed_at_utc,record_revision=excluded.record_revision",
            params![clip.id(),serde_json::to_string(&clip.snapshot.metadata).map_err(err)?,format!("unix_ms:{}",clip.snapshot.recorded_at_unix_ms),clip.media_records_uri,clip.snapshot.revision]).map_err(err)?;
        Ok(())
    }
}

fn owned_table(name: &str) -> bool {
    matches!(
        name,
        "clips"
            | "clip_sources"
            | "clip_proxy"
            | "probe_records"
            | "filmstrip_artifacts"
            | "wave_artifacts"
    )
}

fn validate_container(conn: &Connection, id: &str) -> Result<()> {
    // All schema/identity checks must observe one snapshot during concurrent bootstrap.
    let snapshot = conn.unchecked_transaction().map_err(err)?;
    validate_container_snapshot(&snapshot, id)
}

fn validate_container_snapshot(conn: &Connection, id: &str) -> Result<()> {
    let has_project: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='public_project_settings')",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    if has_project {
        let matches: bool = conn
            .query_row(
                "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                [id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !matches {
            return Err("Projektna baza pripada drugom projektu.".into());
        }
    }
    let has_content: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='ingest_content_schema')",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    if has_content {
        let matches: bool = conn.query_row(
            "SELECT count(*)=1 AND min(project_id)=?1 AND min(version)=?2 FROM ingest_content_schema",
            params![id, SCHEMA_VERSION], |r| r.get(0),
        ).map_err(err)?;
        if !matches {
            return Err("Pogresan projekt ili verzija Ingest baze; nema migracije.".into());
        }
    } else if !has_project {
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        if count != 0 {
            return Err("Nepoznata postojeca baza nije Ingest odrediste.".into());
        }
    }
    Ok(())
}

// Only the validated output DB and its journal directory need owner write access.
// Never recurse or remove delete ACLs; card/source paths are not accepted here.
fn enable_owner_write(file: &Path) -> Result<()> {
    // SQLite companions belong to this validated binding, not separate databases.
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = file.as_os_str().to_os_string();
        name.push(suffix);
        let path = std::path::PathBuf::from(name);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            Ok(_) => return Err("DB write target must be a regular file.".into()),
            Err(error) if !suffix.is_empty() && error.kind() == std::io::ErrorKind::NotFound => {
                continue
            }
            Err(error) => return Err(err(error)),
        };
        let mut permissions = metadata.permissions();
        #[cfg(windows)]
        if permissions.readonly() {
            permissions.set_readonly(false);
            std::fs::set_permissions(&path, permissions).map_err(err)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = permissions.mode();
            if mode & 0o200 == 0 {
                permissions.set_mode(mode | 0o200);
                std::fs::set_permissions(&path, permissions).map_err(err)?;
            }
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for (path, bits) in [(file.parent().ok_or("DB parent missing")?, 0o300)] {
            let permissions = std::fs::metadata(path).map_err(err)?.permissions();
            let mode = permissions.mode();
            if mode & bits != bits {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | bits))
                    .map_err(err)?;
            }
        }
    }
    Ok(())
}

fn codec(media: &MediaRepresentation) -> Option<&str> {
    media
        .streams
        .iter()
        .find_map(|stream| match stream.codec.as_ref().map(|f| &f.value) {
            Some(Signal::Known(codec)) => Some(codec.as_str()),
            _ => None,
        })
}
fn ensure_summary_columns(conn: &Connection) -> Result<()> {
    if !has_column(conn, "clips", "thumbnail_uri")? {
        conn.execute("ALTER TABLE clips ADD COLUMN thumbnail_uri TEXT", [])
            .map_err(err)?;
    }
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_clips;
        CREATE VIEW public_clips AS SELECT clip_id,source_uri,original_uri,name,created_at_utc,
            duration_seconds,duration_frames,fps_num,fps_den,selected,import_status,
            imported_media_uri,import_error,thumbnail_uri FROM clips;",
    )
    .map_err(err)?;
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(err)?;
    let rows = statement
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(err)?;
    for row in rows {
        if row.map_err(err)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn import_status(value: String) -> rusqlite::Result<ImportStatus> {
    match value.as_str() {
        "detected" => Ok(ImportStatus::Detected),
        "queued" => Ok(ImportStatus::Queued),
        "processing" => Ok(ImportStatus::Processing),
        "imported" => Ok(ImportStatus::Imported),
        "failed" => Ok(ImportStatus::Failed),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredClip> {
    let json: String = row.get(0)?;
    let clip = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let import_status = import_status(row.get(2)?)?;
    Ok(StoredClip {
        clip,
        selected: row.get(1)?,
        import_status,
        import_error: row.get(3)?,
        imported_media_uri: row.get(4)?,
    })
}
fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredClipSummary> {
    Ok(StoredClipSummary {
        clip_id: row.get(0)?,
        name: row.get(1)?,
        source_uri: row.get(2)?,
        source_name: row.get(3)?,
        serial_number: row.get(4)?,
        volume_name: row.get(5)?,
        thumbnail_uri: row.get(6)?,
        duration_seconds: row.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
        selected: row.get(8)?,
        import_status: import_status(row.get(9)?)?,
        import_error: row.get(10)?,
        imported_media_uri: row.get(11)?,
        revision: row.get(12)?,
        final_record: row.get(13)?,
    })
}

const CATALOG_FINGERPRINT_OFFSET: u64 = 14_695_981_039_346_656_037;
const CATALOG_FINGERPRINT_PRIME: u64 = 1_099_511_628_211;

fn fingerprint_str(hash: &mut u64, value: &str) {
    fingerprint_u64(hash, value.len() as u64);
    for byte in value.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(CATALOG_FINGERPRINT_PRIME);
    }
}

fn fingerprint_opt_str(hash: &mut u64, value: Option<&str>) {
    match value {
        Some(value) => {
            fingerprint_u64(hash, 1);
            fingerprint_str(hash, value);
        }
        None => fingerprint_u64(hash, 0),
    }
}

fn fingerprint_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(CATALOG_FINGERPRINT_PRIME);
    }
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
