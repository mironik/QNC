use crate::*;
use rusqlite::{limits::Limit, Connection, OpenFlags, Row};
use std::path::Path;

const SCHEMA: &str = include_str!("../../../catalogs/camera-patterns/schema.sql");

fn schema(db: &Connection) -> rusqlite::Result<Vec<(String, String, Option<String>)>> {
    db.prepare(
        "SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
    )?
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
    .collect()
}

fn query<T>(
    db: &Connection,
    sql: &str,
    id: Option<&str>,
    row: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    let mut statement = db.prepare(sql)?;
    match id {
        Some(id) => statement.query_map([id], row)?.collect(),
        None => statement.query_map([], row)?.collect(),
    }
}

pub(crate) fn read(path: &Path, uri: &str) -> Result<Catalog> {
    let metadata = std::fs::metadata(path).map_err(|_| "catalog unavailable")?;
    if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err("catalog file exceeds limit".into());
    }
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "catalog unavailable")?;
    let mut steps = 0;
    db.progress_handler(
        1000,
        Some(move || {
            steps += 1;
            steps > 100_000
        }),
    );
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, 1024 * 1024);
    db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 1024 * 1024);
    read_db(&db, uri).map_err(|_error| {
        #[cfg(test)]
        eprintln!("catalog validation: {_error}");
        "invalid or unsupported camera catalog database".into()
    })
}

fn read_db(db: &Connection, uri: &str) -> std::result::Result<Catalog, Box<dyn std::error::Error>> {
    db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON; BEGIN;")?;
    let application_id: i64 = db.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if application_id != 1364083523 || version != 1 {
        return Err("wrong schema identity".into());
    }
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(SCHEMA)?;
    if schema(db)? != schema(&expected)? {
        return Err("unsupported physical schema".into());
    }
    let integrity: String = db.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if integrity != "ok" || db.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err("invalid database integrity".into());
    }
    for view in [
        "public_patterns",
        "public_sources",
        "public_roots",
        "public_file_rules",
        "public_metadata_fields",
        "public_evidence",
        "public_coverage_gaps",
    ] {
        let count: i64 = db.query_row(&format!("SELECT COUNT(*) FROM {view}"), [], |r| r.get(0))?;
        if count > 65536 {
            return Err("catalog row limit".into());
        }
    }
    if db.query_row("SELECT COUNT(*) FROM public_catalog", [], |r| {
        r.get::<_, i64>(0)
    })? != 1
    {
        return Err("catalog identity count".into());
    }
    let mut catalog = db.query_row("SELECT catalog_id,schema_version,dataset_version,reviewed_on,runtime_status FROM public_catalog", [], |r| Ok(Catalog {
        catalog_uri: uri.into(), contract_version: VERSION.into(), catalog_id: r.get(0)?, schema_version: r.get(1)?, dataset_version: r.get(2)?, reviewed_on: r.get(3)?, runtime_status: r.get(4)?, patterns: vec![], sources: vec![], gaps: vec![]
    }))?;
    catalog.sources = query(db, "SELECT source_id,kind,publisher,title,locator,section,reviewed_on FROM public_sources ORDER BY source_id", None, |r| Ok(Source { id: r.get(0)?, kind: r.get(1)?, publisher: r.get(2)?, title: r.get(3)?, locator: r.get(4)?, section: r.get(5)?, reviewed_on: r.get(6)? }))?;
    catalog.patterns = query(db, "SELECT pattern_id,manufacturer,family,applicability,naming_rule,evidence_level,status,status_reason,root_scope,grouping_method,grouping_notes,limitations FROM public_patterns ORDER BY pattern_id", None, |r| Ok(Pattern {
        id: r.get(0)?, manufacturer: r.get(1)?, family: r.get(2)?, applicability: r.get(3)?, naming_rule: r.get(4)?, evidence_level: r.get(5)?, status: r.get(6)?, status_reason: r.get(7)?, root_scope: r.get(8)?, grouping_method: r.get(9)?, grouping_notes: r.get(10)?, limitations: r.get(11)?, roots: vec![], files: vec![], metadata: vec![], evidence: vec![]
    }))?;
    if catalog.patterns.len() > 1024 || catalog.sources.len() > 4096 {
        return Err("catalog row limit".into());
    }
    for p in &mut catalog.patterns {
        p.roots = query(db, "SELECT relative_pattern FROM public_roots WHERE pattern_id=?1 ORDER BY relative_pattern", Some(&p.id), |r| r.get(0))?;
        p.files = query(db, "SELECT relative_pattern,role,condition FROM public_file_rules WHERE pattern_id=?1 ORDER BY relative_pattern,role", Some(&p.id), |r| Ok(FileRule { path: r.get(0)?, role: r.get(1)?, condition: r.get(2)? }))?;
        p.metadata = query(db, "SELECT document_pattern,xml_namespace,selector,meaning FROM public_metadata_fields WHERE pattern_id=?1 ORDER BY document_pattern,selector", Some(&p.id), |r| Ok(MetadataField { document: r.get(0)?, namespace: r.get(1)?, selector: r.get(2)?, meaning: r.get(3)? }))?;
        p.evidence = query(
            db,
            "SELECT source_id,supports FROM public_evidence WHERE pattern_id=?1 ORDER BY source_id",
            Some(&p.id),
            |r| {
                Ok(Evidence {
                    source: r.get(0)?,
                    supports: r.get(1)?,
                })
            },
        )?;
    }
    catalog.gaps = query(db, "SELECT gap_id,manufacturer,family,missing_evidence,source_id,status FROM public_coverage_gaps ORDER BY gap_id", None, |r| Ok(CoverageGap { id: r.get(0)?, manufacturer: r.get(1)?, family: r.get(2)?, missing_evidence: r.get(3)?, source: r.get(4)?, status: r.get(5)? }))?;
    let candidates: BTreeSet<String> = query(
        db,
        "SELECT pattern_id FROM public_analysis_patterns",
        None,
        |r| r.get(0),
    )?
    .into_iter()
    .collect();
    if candidates
        != catalog
            .patterns
            .iter()
            .filter(|p| p.analysis_candidate())
            .map(|p| p.id.clone())
            .collect()
    {
        return Err("analysis view mismatch".into());
    }
    catalog.validate()?;
    db.execute_batch("COMMIT;")?;
    Ok(catalog)
}
