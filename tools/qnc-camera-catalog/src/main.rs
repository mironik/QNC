use std::{collections::HashSet, fs, io::Write, path::Path};

use rusqlite::{params, Connection, DatabaseName, OpenFlags};
use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SCHEMA: &str = include_str!("../../../catalogs/camera-patterns/schema.sql");
const APPLICATION_ID: i64 = 1364083523;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    id: String,
    kind: String,
    publisher: String,
    title: String,
    locator: String,
    section: String,
    reviewed_on: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRule {
    path: String,
    role: String,
    condition: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataField {
    document: String,
    namespace: String,
    selector: String,
    meaning: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    source: String,
    supports: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pattern {
    id: String,
    manufacturer: String,
    family: String,
    applicability: String,
    naming_rule: String,
    evidence_level: String,
    status: String,
    status_reason: String,
    root_scope: String,
    roots: Vec<String>,
    grouping_method: String,
    grouping_notes: String,
    limitations: String,
    files: Vec<FileRule>,
    metadata: Vec<MetadataField>,
    evidence: Vec<Evidence>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gap {
    id: String,
    manufacturer: String,
    family: String,
    missing_evidence: String,
    source: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Seed {
    dataset_version: String,
    reviewed_on: String,
    sources: Vec<Source>,
    patterns: Vec<Pattern>,
    gaps: Vec<Gap>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    base_version: String,
    dataset_version: String,
    changed_on: String,
    #[serde(default)]
    sources: Vec<Source>,
    changes: Vec<Change>,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Change {
    Add {
        pattern: Box<Pattern>,
        reason: String,
    },
    Replace {
        pattern: Box<Pattern>,
        reason: String,
    },
    Delete {
        id: String,
        reason: String,
    },
    MarkIncorrect {
        id: String,
        reason: String,
    },
    Enable {
        id: String,
        reason: String,
    },
    Disable {
        id: String,
        reason: String,
    },
}

fn required(value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err("Required value is empty".into());
    }
    Ok(())
}

fn relative(value: &str) -> Result<()> {
    if value == "." {
        return Ok(());
    }
    if value.is_empty()
        || value.contains(['\\', ':'])
        || value.chars().any(char::is_control)
        || value.split('/').any(|s| matches!(s, "" | "." | ".."))
    {
        return Err(format!("Not a neutral relative path expression: {value:?}").into());
    }
    Ok(())
}

fn validate_source(s: &Source) -> Result<()> {
    for value in [&s.id, &s.publisher, &s.title, &s.section, &s.reviewed_on] {
        required(value)?;
    }
    match s.kind.as_str() {
        "manufacturer_document" if s.locator.starts_with("https://") => Ok(()),
        "card_observation" => relative(&s.locator),
        _ => Err(format!("Invalid source provenance: {}", s.id).into()),
    }
}

fn insert_source(db: &Connection, s: &Source) -> Result<()> {
    validate_source(s)?;
    db.execute(
        "INSERT INTO source VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            s.id,
            s.kind,
            s.publisher,
            s.title,
            s.locator,
            s.section,
            s.reviewed_on
        ],
    )?;
    Ok(())
}

fn validate_pattern(db: &Connection, p: &Pattern) -> Result<()> {
    for value in [
        &p.id,
        &p.manufacturer,
        &p.family,
        &p.applicability,
        &p.naming_rule,
        &p.status_reason,
        &p.grouping_notes,
        &p.limitations,
    ] {
        required(value)?;
    }
    if p.root_scope == "unknown" && !p.roots.is_empty() {
        return Err(format!("{}: unknown root must not contain guessed paths", p.id).into());
    }
    for root in &p.roots {
        relative(root)?;
    }
    for file in &p.files {
        relative(&file.path)?;
        required(&file.condition)?;
    }
    for m in &p.metadata {
        relative(&m.document)?;
        required(&m.namespace)?;
        required(&m.selector)?;
        required(&m.meaning)?;
    }
    if p.evidence.is_empty() {
        return Err(format!("{}: missing evidence", p.id).into());
    }
    let mut documented = false;
    let mut observed = false;
    for evidence in &p.evidence {
        required(&evidence.supports)?;
        let kind: String = db.query_row(
            "SELECT kind FROM source WHERE source_id=?1",
            [&evidence.source],
            |r| r.get(0),
        )?;
        documented |= kind == "manufacturer_document";
        observed |= kind == "card_observation";
    }
    if p.evidence_level == "observed" && !observed {
        return Err(format!("{}: observation claim without observation evidence", p.id).into());
    }
    if p.evidence_level == "documented" && !documented {
        return Err(format!(
            "{}: documentation claim without manufacturer evidence",
            p.id
        )
        .into());
    }
    if p.status == "enabled"
        && (!documented
            || p.evidence_level == "partial"
            || p.root_scope == "unknown"
            || p.roots.is_empty()
            || !p.files.iter().any(|f| f.role == "original_candidate"))
    {
        return Err(format!(
            "{}: enabled pattern needs factory evidence, directories and original rules",
            p.id
        )
        .into());
    }
    Ok(())
}

fn insert_pattern(db: &Connection, p: &Pattern) -> Result<()> {
    validate_pattern(db, p)?;
    db.execute(
        "INSERT INTO pattern VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            p.id,
            p.manufacturer,
            p.family,
            p.applicability,
            p.naming_rule,
            p.evidence_level,
            p.status,
            p.status_reason,
            p.root_scope,
            p.grouping_method,
            p.grouping_notes,
            p.limitations
        ],
    )?;
    for root in &p.roots {
        db.execute(
            "INSERT INTO pattern_root VALUES (?1,?2)",
            params![p.id, root],
        )?;
    }
    for f in &p.files {
        db.execute(
            "INSERT INTO file_rule VALUES (?1,?2,?3,?4)",
            params![p.id, f.path, f.role, f.condition],
        )?;
    }
    for m in &p.metadata {
        db.execute(
            "INSERT INTO metadata_field VALUES (?1,?2,?3,?4,?5)",
            params![p.id, m.document, m.namespace, m.selector, m.meaning],
        )?;
    }
    for e in &p.evidence {
        db.execute(
            "INSERT INTO evidence VALUES (?1,?2,?3)",
            params![p.id, e.source, e.supports],
        )?;
    }
    Ok(())
}

fn pattern(db: &Connection, id: &str) -> Result<Pattern> {
    let mut p = db.query_row("SELECT * FROM pattern WHERE pattern_id=?1", [id], |r| {
        Ok(Pattern {
            id: r.get(0)?,
            manufacturer: r.get(1)?,
            family: r.get(2)?,
            applicability: r.get(3)?,
            naming_rule: r.get(4)?,
            evidence_level: r.get(5)?,
            status: r.get(6)?,
            status_reason: r.get(7)?,
            root_scope: r.get(8)?,
            grouping_method: r.get(9)?,
            grouping_notes: r.get(10)?,
            limitations: r.get(11)?,
            roots: vec![],
            files: vec![],
            metadata: vec![],
            evidence: vec![],
        })
    })?;
    p.roots = db.prepare("SELECT relative_pattern FROM pattern_root WHERE pattern_id=?1 ORDER BY relative_pattern")?.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    p.files = db.prepare("SELECT relative_pattern,role,condition FROM file_rule WHERE pattern_id=?1 ORDER BY relative_pattern,role")?.query_map([id], |r| Ok(FileRule {path:r.get(0)?,role:r.get(1)?,condition:r.get(2)?}))?.collect::<rusqlite::Result<_>>()?;
    p.metadata = db.prepare("SELECT document_pattern,xml_namespace,selector,meaning FROM metadata_field WHERE pattern_id=?1 ORDER BY document_pattern,selector")?.query_map([id], |r| Ok(MetadataField {document:r.get(0)?,namespace:r.get(1)?,selector:r.get(2)?,meaning:r.get(3)?}))?.collect::<rusqlite::Result<_>>()?;
    p.evidence = db
        .prepare("SELECT source_id,supports FROM evidence WHERE pattern_id=?1 ORDER BY source_id")?
        .query_map([id], |r| {
            Ok(Evidence {
                source: r.get(0)?,
                supports: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(p)
}

fn log_change(
    db: &Connection,
    version: &str,
    date: &str,
    operation: &str,
    reason: &str,
    before: Option<&Pattern>,
    after: Option<&Pattern>,
) -> Result<()> {
    required(reason)?;
    let id = after
        .or(before)
        .ok_or("Change needs a pattern")?
        .id
        .as_str();
    db.execute("INSERT INTO change_log (dataset_version,changed_on,pattern_id,operation,reason,before_json,after_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![version,date,id,operation,reason,before.map(serde_json::to_string).transpose()?,after.map(serde_json::to_string).transpose()?])?;
    Ok(())
}

fn build(seed: &Seed) -> Result<Connection> {
    required(&seed.dataset_version)?;
    required(&seed.reviewed_on)?;
    let mut db = Connection::open_in_memory()?;
    db.execute_batch(SCHEMA)?;
    let tx = db.transaction()?;
    tx.execute(
        "INSERT INTO catalog VALUES ('qnc.catalog.camera-patterns',1,?1,?2,'research_only')",
        params![seed.dataset_version, seed.reviewed_on],
    )?;
    for s in &seed.sources {
        insert_source(&tx, s)?;
    }
    for p in &seed.patterns {
        insert_pattern(&tx, p)?;
        log_change(
            &tx,
            &seed.dataset_version,
            &seed.reviewed_on,
            "add",
            "Initial sourced catalog",
            None,
            Some(p),
        )?;
    }
    for g in &seed.gaps {
        for value in [&g.id, &g.manufacturer, &g.family, &g.missing_evidence] {
            required(value)?;
        }
        tx.execute(
            "INSERT INTO coverage_gap VALUES (?1,?2,?3,?4,?5,'research_pending')",
            params![g.id, g.manufacturer, g.family, g.missing_evidence, g.source],
        )?;
    }
    tx.commit()?;
    check(&db)?;
    Ok(db)
}

fn schema_description(db: &Connection) -> Result<Vec<(String, String, Option<String>)>> {
    Ok(db.prepare("SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")?
        .query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?)
}

fn check(db: &Connection) -> Result<()> {
    let identity: i64 = db.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if identity != APPLICATION_ID || version != 1 {
        return Err("Not a supported camera catalog schema".into());
    }
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(SCHEMA)?;
    // Validate physical schema before reading views or copying an external catalog.
    if schema_description(db)? != schema_description(&expected)? {
        return Err("Catalog physical schema differs from contract".into());
    }
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" || db.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err("Catalog integrity/foreign-key check failed".into());
    }
    let count: i64 = db.query_row("SELECT count(*) FROM catalog", [], |r| r.get(0))?;
    if count != 1 {
        return Err("Catalog must contain one identity record".into());
    }
    let sources = db
        .prepare("SELECT * FROM source")?
        .query_map([], |r| {
            Ok(Source {
                id: r.get(0)?,
                kind: r.get(1)?,
                publisher: r.get(2)?,
                title: r.get(3)?,
                locator: r.get(4)?,
                section: r.get(5)?,
                reviewed_on: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for s in sources {
        validate_source(&s)?;
    }
    let ids = db
        .prepare("SELECT pattern_id FROM pattern")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in ids {
        validate_pattern(db, &pattern(db, &id)?)?;
    }
    Ok(())
}

fn open_read_only(path: &Path) -> Result<Connection> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")?;
    check(&db)?;
    Ok(db)
}

fn revise(source: &Connection, revision: &Revision) -> Result<Connection> {
    check(source)?;
    required(&revision.dataset_version)?;
    required(&revision.changed_on)?;
    let previous: String =
        source.query_row("SELECT dataset_version FROM catalog", [], |r| r.get(0))?;
    if previous != revision.base_version
        || previous == revision.dataset_version
        || source
            .prepare("SELECT 1 FROM change_log WHERE dataset_version=?1")?
            .exists([&revision.dataset_version])?
    {
        return Err("Stale base_version or reused dataset_version".into());
    }
    if revision.changes.is_empty() {
        return Err("Revision has no pattern changes".into());
    }
    let mut db = Connection::open_in_memory()?;
    let result = rusqlite::backup::Backup::new(source, &mut db)?.step(-1)?;
    if result != rusqlite::backup::StepResult::Done {
        return Err("Catalog snapshot unavailable; source unchanged".into());
    }
    db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")?;
    let tx = db.transaction()?;
    for s in &revision.sources {
        insert_source(&tx, s)?;
    }
    let mut changed = HashSet::new();
    for change in &revision.changes {
        let (id, reason, operation) = match change {
            Change::Add { pattern, reason } => (&pattern.id, reason, "add"),
            Change::Replace { pattern, reason } => (&pattern.id, reason, "replace"),
            Change::Delete { id, reason } => (id, reason, "delete"),
            Change::MarkIncorrect { id, reason } => (id, reason, "mark_incorrect"),
            Change::Enable { id, reason } => (id, reason, "enable"),
            Change::Disable { id, reason } => (id, reason, "disable"),
        };
        required(reason)?;
        if !changed.insert(id) {
            return Err(format!("Duplicate change for {id}").into());
        }
        let before = if matches!(change, Change::Add { .. }) {
            None
        } else {
            Some(pattern(&tx, id)?)
        };
        let after = match change {
            Change::Add { pattern, .. } | Change::Replace { pattern, .. } => {
                Some(pattern.as_ref().clone())
            }
            Change::Delete { .. } => None,
            _ => {
                let mut p = before.clone().ok_or("Pattern missing")?;
                p.status = match change {
                    Change::MarkIncorrect { .. } => "incorrect",
                    Change::Enable { .. } => "enabled",
                    _ => "disabled",
                }
                .into();
                p.status_reason = reason.clone();
                Some(p)
            }
        };
        if before.is_some() {
            tx.execute("DELETE FROM pattern WHERE pattern_id=?1", [id])?;
        }
        if let Some(p) = &after {
            insert_pattern(&tx, p)?;
        }
        log_change(
            &tx,
            &revision.dataset_version,
            &revision.changed_on,
            operation,
            reason,
            before.as_ref(),
            after.as_ref(),
        )?;
    }
    tx.execute(
        "UPDATE catalog SET dataset_version=?1,reviewed_on=?2",
        params![revision.dataset_version, revision.changed_on],
    )?;
    tx.commit()?;
    check(&db)?;
    Ok(db)
}

fn publish(db: &Connection, output: &Path) -> Result<()> {
    check(db)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Publish a complete snapshot without replacing any existing path or database.
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&db.serialize(DatabaseName::Main)?)?;
    file.as_file().sync_all()?;
    open_read_only(file.path())?;
    file.persist_noclobber(output).map_err(|e| e.error)?;
    Ok(())
}

fn summary(db: &Connection) -> Result<()> {
    let row: (String, i64, i64, i64) = db.query_row("SELECT dataset_version,(SELECT count(*) FROM pattern),(SELECT count(*) FROM public_analysis_patterns),(SELECT count(*) FROM coverage_gap) FROM catalog", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    println!("Catalog {}: {} patterns, {} documented analysis candidates, {} coverage gaps. Runtime detector: not implemented.", row.0,row.1,row.2,row.3);
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|v| v.to_str()) {
        Some("build") if args.len() == 3 => {
            let seed: Seed = serde_json::from_slice(&fs::read(&args[1])?)?;
            let db = build(&seed)?;
            publish(&db, Path::new(&args[2]))?;
            summary(&db)
        }
        Some("revise") if args.len() == 4 => {
            let source = open_read_only(Path::new(&args[1]))?;
            let revision: Revision = serde_json::from_slice(&fs::read(&args[2])?)?;
            let db = revise(&source, &revision)?;
            publish(&db, Path::new(&args[3]))?;
            summary(&db)
        }
        Some("check") if args.len() == 2 => summary(&open_read_only(Path::new(&args[1]))?),
        Some("show") if args.len() == 3 => {
            let db = open_read_only(Path::new(&args[1]))?;
            let id = args[2].to_str().ok_or("Pattern ID must be Unicode")?;
            println!("{}", serde_json::to_string_pretty(&pattern(&db,id)?)?);
            Ok(())
        }
        _ => Err("Usage: qnc-camera-catalog build SEED.json NEW.sqlite | check CATALOG.sqlite | show CATALOG.sqlite PATTERN_ID | revise CATALOG.sqlite CHANGES.json NEW.sqlite".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("camera-catalog: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
