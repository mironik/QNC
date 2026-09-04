use serde_json::Value;

use qnc_contracts::{parse_qnc_uri, ValidationReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseContract {
    pub database_id: String,
    pub owner_application: String,
    pub schema_version: String,
    pub qnc_uri: String,
    pub tables: Vec<String>,
    pub public_read_views: Vec<String>,
    pub write_owner: String,
    pub public_read_policy: PublicReadPolicy,
    pub migration_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicReadPolicy {
    PublicReadViews,
    OwnerOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbAccessError {
    WriteDenied {
        actor_application: String,
        database_id: String,
        owner_application: String,
    },
    ReadDenied {
        actor_application: String,
        database_id: String,
    },
}

impl DatabaseContract {
    pub fn from_json_str(name: &str, contents: &str) -> Result<Self, ValidationReport> {
        let mut report = ValidationReport::new();
        let value = match serde_json::from_str::<Value>(contents) {
            Ok(value) => value,
            Err(err) => {
                report.error(format!("{name}: invalid JSON: {err}"));
                return Err(report);
            }
        };

        let Some(object) = value.as_object() else {
            report.error(format!("{name}: DB contract must be a JSON object"));
            return Err(report);
        };

        for field in [
            "database_id",
            "owner_application",
            "schema_version",
            "qnc_uri",
            "tables",
            "public_read_views",
            "write_owner",
            "public_read_policy",
            "migration_policy",
            "created_at",
            "updated_at",
        ] {
            if !object.contains_key(field) {
                report.error(format!("{name}: missing required DB field '{field}'"));
            }
        }

        let database_id = string_field(&mut report, name, object, "database_id");
        let owner_application = string_field(&mut report, name, object, "owner_application");
        let schema_version = string_field(&mut report, name, object, "schema_version");
        let qnc_uri = string_field(&mut report, name, object, "qnc_uri");
        let tables = string_array_field(&mut report, name, object, "tables");
        let public_read_views = string_array_field(&mut report, name, object, "public_read_views");
        let write_owner = string_field(&mut report, name, object, "write_owner");
        let migration_policy = string_field(&mut report, name, object, "migration_policy");
        let public_read_policy = parse_public_read_policy(&mut report, name, object);

        if let Err(err) = parse_qnc_uri(&qnc_uri) {
            report.error(format!("{name}: invalid qnc_uri: {err}"));
        }

        if !owner_application.is_empty()
            && !write_owner.is_empty()
            && owner_application != write_owner
        {
            report.error(format!("{name}: write_owner must match owner_application"));
        }

        if public_read_policy == Some(PublicReadPolicy::PublicReadViews)
            && public_read_views.is_empty()
        {
            report.error(format!(
                "{name}: public_read_policy requires public_read_views"
            ));
        }

        if report.is_ok() {
            Ok(Self {
                database_id,
                owner_application,
                schema_version,
                qnc_uri,
                tables,
                public_read_views,
                write_owner,
                public_read_policy: public_read_policy.unwrap_or(PublicReadPolicy::OwnerOnly),
                migration_policy,
            })
        } else {
            Err(report)
        }
    }

    pub fn validate_write(&self, actor_application: &str) -> Result<(), DbAccessError> {
        if actor_application == self.write_owner {
            Ok(())
        } else {
            Err(DbAccessError::WriteDenied {
                actor_application: actor_application.to_string(),
                database_id: self.database_id.clone(),
                owner_application: self.owner_application.clone(),
            })
        }
    }

    pub fn validate_read(&self, actor_application: &str) -> Result<(), DbAccessError> {
        if actor_application == self.owner_application
            || self.public_read_policy == PublicReadPolicy::PublicReadViews
        {
            Ok(())
        } else {
            Err(DbAccessError::ReadDenied {
                actor_application: actor_application.to_string(),
                database_id: self.database_id.clone(),
            })
        }
    }
}

pub fn validate_db_contract_json(name: &str, contents: &str) -> ValidationReport {
    match DatabaseContract::from_json_str(name, contents) {
        Ok(_) => ValidationReport::new(),
        Err(report) => report,
    }
}

fn string_field(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> String {
    match object.get(field).and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => value.to_string(),
        _ => {
            report.error(format!(
                "{name}: field '{field}' must be a non-empty string"
            ));
            String::new()
        }
    }
}

fn string_array_field(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Vec<String> {
    let Some(values) = object.get(field).and_then(Value::as_array) else {
        report.error(format!("{name}: field '{field}' must be an array"));
        return Vec::new();
    };

    let mut output = Vec::new();
    for value in values {
        match value.as_str() {
            Some(value) if !value.trim().is_empty() => output.push(value.to_string()),
            _ => report.error(format!(
                "{name}: field '{field}' entries must be non-empty strings"
            )),
        }
    }
    output
}

fn parse_public_read_policy(
    report: &mut ValidationReport,
    name: &str,
    object: &serde_json::Map<String, Value>,
) -> Option<PublicReadPolicy> {
    match object.get("public_read_policy").and_then(Value::as_str) {
        Some("public_read_views") => Some(PublicReadPolicy::PublicReadViews),
        Some("owner_only") => Some(PublicReadPolicy::OwnerOnly),
        Some(value) => {
            report.error(format!(
                "{name}: public_read_policy value '{value}' is not allowed"
            ));
            None
        }
        None => {
            report.error(format!(
                "{name}: field 'public_read_policy' must be a string"
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract() -> DatabaseContract {
        DatabaseContract::from_json_str(
            "sample.database.json",
            r#"{
                "database_id": "qnc.db.ingest_content",
                "owner_application": "qnc.ingest",
                "schema_version": "0.1.0",
                "qnc_uri": "qnc://local/db/ingest_content",
                "tables": ["clips", "sources", "probe_records"],
                "public_read_views": ["public_clips", "public_probe_records"],
                "write_owner": "qnc.ingest",
                "public_read_policy": "public_read_views",
                "migration_policy": "owner_application_only",
                "created_at": "2026-09-04T00:00:00Z",
                "updated_at": "2026-09-04T00:00:00Z"
            }"#,
        )
        .expect("valid contract")
    }

    #[test]
    fn validates_owner_write_policy() {
        let contract = sample_contract();
        assert!(contract.validate_write("qnc.ingest").is_ok());
        assert!(contract.validate_write("qnc.story").is_err());
    }

    #[test]
    fn public_views_allow_cross_application_reads() {
        let contract = sample_contract();
        assert!(contract.validate_read("qnc.story").is_ok());
        assert!(contract.validate_read("qnc.media-assist").is_ok());
    }

    #[test]
    fn rejects_raw_path_database_identity() {
        let report = validate_db_contract_json(
            "bad.database.json",
            r#"{
                "database_id": "qnc.db.bad",
                "owner_application": "qnc.project",
                "schema_version": "0.1.0",
                "qnc_uri": "C:\\media\\bad.sqlite",
                "tables": ["bad"],
                "public_read_views": ["public_bad"],
                "write_owner": "qnc.project",
                "public_read_policy": "public_read_views",
                "migration_policy": "owner_application_only",
                "created_at": "2026-09-04T00:00:00Z",
                "updated_at": "2026-09-04T00:00:00Z"
            }"#,
        );
        assert!(!report.is_ok());
    }

    #[test]
    fn rejects_write_owner_mismatch() {
        let report = validate_db_contract_json(
            "bad.database.json",
            r#"{
                "database_id": "qnc.db.bad",
                "owner_application": "qnc.ingest",
                "schema_version": "0.1.0",
                "qnc_uri": "qnc://local/db/bad",
                "tables": ["bad"],
                "public_read_views": ["public_bad"],
                "write_owner": "qnc.story",
                "public_read_policy": "public_read_views",
                "migration_policy": "owner_application_only",
                "created_at": "2026-09-04T00:00:00Z",
                "updated_at": "2026-09-04T00:00:00Z"
            }"#,
        );
        assert!(!report.is_ok());
    }
}
