//! The order of the applications of a project.
//!
//! A project stores which applications it uses and in which priority groups
//! (`a`, `b`, `c`, ...). This component reads that order through the public view of the
//! project database, read-only, so any form can hand over to the next group without
//! knowing the project application.

use qnc_work_settings::{SettingsReader, WorkSettings};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub const MODULE_ID: &str = "qnc.module.application-sequence";
pub const VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceStep {
    pub application_id: String,
    pub tab_id: String,
    pub priority_group: String,
}

/// The sequence of the project the settings describe. The project folder must be on
/// this machine.
pub fn read(reader: &SettingsReader, settings: &WorkSettings) -> Result<Vec<SequenceStep>, String> {
    let dir = reader
        .local_workspace_dir(settings)
        .map_err(|e| e.to_string())?
        .ok_or("Slijed aplikacija trazi lokalni direktorij projekta.")?;
    read_file(&dir.join("qnc_project.db"), &settings.project_id)
}

pub fn read_file(database: &Path, project_id: &str) -> Result<Vec<SequenceStep>, String> {
    let db = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    let mut statement = db
        .prepare(
            "SELECT application_id, tab_id, priority_group \
             FROM public_project_application_sequence WHERE project_id=?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    let steps = statement
        .query_map([project_id], |row| {
            Ok(SequenceStep {
                application_id: row.get(0)?,
                tab_id: row.get(1)?,
                priority_group: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    validate(&steps)?;
    Ok(steps)
}

fn validate(steps: &[SequenceStep]) -> Result<(), String> {
    let invalid = steps.first().is_none_or(|step| step.priority_group != "a")
        || steps.iter().any(|step| {
            step.priority_group.len() != 1
                || !step.priority_group.as_bytes()[0].is_ascii_lowercase()
                || !step.application_id.starts_with("qnc.")
                || step.tab_id.is_empty()
        })
        || steps
            .windows(2)
            .any(|pair| pair[0].priority_group >= pair[1].priority_group);
    if invalid {
        return Err("Projekt nema valjan slijed prioritetnih grupa.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database(rows: &[(&str, &str, &str, i64)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let db = Connection::open(dir.path().join("p.db")).unwrap();
        db.execute_batch(
            "CREATE TABLE seq(project_id TEXT, application_id TEXT, tab_id TEXT, priority_group TEXT, position INTEGER);
             CREATE VIEW public_project_application_sequence AS SELECT * FROM seq;",
        )
        .unwrap();
        for (app, tab, group, position) in rows {
            db.execute(
                "INSERT INTO seq VALUES('p1',?1,?2,?3,?4)",
                rusqlite::params![app, tab, group, position],
            )
            .unwrap();
        }
        dir
    }

    #[test]
    fn reads_the_ordered_sequence() {
        let dir = database(&[
            ("qnc.ingest", "ingest", "b", 2),
            ("qnc.project", "project", "a", 1),
            ("qnc.media", "media", "c", 3),
        ]);
        let steps = read_file(&dir.path().join("p.db"), "p1").unwrap();
        let groups: Vec<_> = steps.iter().map(|s| s.priority_group.as_str()).collect();
        assert_eq!(groups, ["a", "b", "c"]);
        assert_eq!(steps[1].tab_id, "ingest");
    }

    #[test]
    fn a_sequence_that_does_not_start_at_group_a_is_refused() {
        let dir = database(&[("qnc.ingest", "ingest", "b", 1)]);
        assert!(read_file(&dir.path().join("p.db"), "p1").is_err());
    }

    #[test]
    fn a_project_without_a_sequence_is_refused() {
        let dir = database(&[]);
        assert!(read_file(&dir.path().join("p.db"), "p1").is_err());
    }
}
