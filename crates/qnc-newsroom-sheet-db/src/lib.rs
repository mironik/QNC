//! The story sheet of a project (user 2026-10-27: the data of the news story is a sheet a
//! newsroom can shape; its fields are kept by code, so their names are free): the sheet as it
//! was when it was filled (its fields, names and choices, as JSON) and the value of each
//! field by its code. Kept in the project database, so the script form and every export read
//! the names and values from the project itself, on any computer.
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! tables, is written only through that intermediary and knows no form or application.

mod store;

use std::collections::BTreeMap;
use std::sync::Arc;

use qnc_db_broker::{Access, ProjectDbTarget, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.newsroom-sheet-db";

pub type Result<T> = std::result::Result<T, String>;

/// The sheet of a project and its values; an empty sheet (`Null`) when none was kept.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorySheet {
    /// The description of the fields as it was when the sheet was filled.
    pub sheet: Value,
    /// The value of each field, by its code.
    pub values: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The sheet and all its values are kept (they replace what was kept).
    Save(StorySheet),
    /// The sheet and its values.
    Read,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Sheet(StorySheet),
    Changed,
}

impl StorySheet {
    /// What a search of projects finds this sheet by: every typed value, and the names of the
    /// chosen choices (the code is kept, its name is what people type).
    pub fn search_text(&self) -> String {
        let options = self.sheet.get("options");
        let mut words = Vec::new();
        for value in self.values.values() {
            let text = match value {
                Value::String(text) => text.trim().to_string(),
                Value::Number(number) => number.to_string(),
                _ => continue,
            };
            if text.is_empty() {
                continue;
            }
            let named = options.and_then(Value::as_object).into_iter().flat_map(|sets| sets.values()).filter_map(Value::as_array).flatten().find_map(|pair| {
                let pair = pair.as_array()?;
                (pair.first()?.as_str()? == text).then(|| pair.get(1)?.as_str().map(str::to_string)).flatten()
            });
            words.push(text);
            words.extend(named);
        }
        words.join(" ")
    }
}

/// Joins the story sheet tables to a project database.
pub struct StorySheetModule;

impl StorySheetModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

/// The story sheet of the project behind `target`.
pub fn read(target: &ProjectDbTarget) -> Result<StorySheet> {
    let mut client = target.open(Access::ReadOnly, vec![StorySheetModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::Read)?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Sheet(sheet) => Ok(sheet),
        Data::Changed => Err(qnc_ui_text::text("db.story_sheet_reply").into()),
    }
}

/// Keeps `sheet` in the project behind `target` through its serial writer.
pub fn save(target: &ProjectDbTarget, sheet: StorySheet) -> Result<()> {
    let writer = qnc_db_broker::ProjectDbWriter::spawn(target.clone(), vec![StorySheetModule::factory()])?;
    writer.call(MODULE_ID, encode(&Operation::Save(sheet))?).map(drop)
}

#[cfg(test)]
mod tests;
