//! The A1/A2 sound chosen for a clip on the source timeline (user 2026-10-24: "what is set in
//! the Story for a clip is used for that clip"; A1 on camera channel 1 and A2 on channel 2
//! by default): the channel, on/off and level of each, kept per clip in the project
//! database, so another click on the clip, another form or another computer hears it the
//! same and a new segment or cover of the clip takes it. A segment or cover keeps its own
//! sound once made and stays changeable on its own (Story Wrap row and zoom).
//!
//! A table module of the project database intermediary (`qnc-db-broker`): it owns only its
//! table, is written only through that intermediary and knows no form, worker or
//! application.

mod store;

use std::sync::{mpsc, Arc};

use qnc_db_broker::{Access, ProjectDbTarget, ProjectDbWriter, TableModuleFactory};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.clip-sound";

pub type Result<T> = std::result::Result<T, String>;

/// One lane of a clip: the camera channel (zero based), whether it is off and its level in dB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaneSound {
    pub channel: u16,
    pub off: bool,
    pub level_db: i8,
}

/// The sound kept for one clip: A1 and A2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipSound {
    pub clip_id: String,
    pub a1: LaneSound,
    pub a2: LaneSound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// The sound of a clip is kept (it replaces what the clip had).
    Set(ClipSound),
    /// The sound kept for a clip, when it has one.
    Read { clip_id: String },
}

impl Operation {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Set(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Sound(Option<ClipSound>),
    Changed,
}

/// Joins the clip sound table to a project database.
pub struct ClipSoundModule;

impl ClipSoundModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        qnc_db_broker::owned_tables::<store::Store>()
    }
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

/// The sound kept for `clip_id` in the project behind `target`; `None` when it has none.
pub fn read(target: &ProjectDbTarget, clip_id: &str) -> Result<Option<ClipSound>> {
    let mut client = target.open(Access::ReadOnly, vec![ClipSoundModule::factory()])?;
    let answer = client.execute(MODULE_ID, encode(&Operation::Read { clip_id: clip_id.into() })?)?;
    match serde_json::from_value(answer).map_err(|e| e.to_string())? {
        Data::Sound(sound) => Ok(sound),
        Data::Changed => Err(qnc_ui_text::text("db.clip_sound_reply").into()),
    }
}

/// The sound of the clips one form shows: read once per clip in the background, kept
/// through the project's serial writer without waiting. The database stays the truth:
/// another clip or another project reads it again.
#[derive(Default)]
pub struct ClipSounds {
    /// The clip read (project URI, clip) and what it holds.
    current: Option<(String, String, Option<ClipSound>)>,
    reading: Option<(String, String, mpsc::Receiver<Option<ClipSound>>)>,
    writer: Option<(String, ProjectDbWriter)>,
    pending: Vec<qnc_db_broker::Pending>,
}

impl std::fmt::Debug for ClipSounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipSounds").field("current", &self.current).finish_non_exhaustive()
    }
}

impl ClipSounds {
    pub fn new() -> Self {
        Self::default()
    }

    /// The sound kept for `clip_id`: `Some(None)` when it has none, `None` while it is read.
    pub fn for_clip(&mut self, target: Option<&ProjectDbTarget>, clip_id: Option<&str>) -> Option<Option<ClipSound>> {
        self.pending.retain(|pending| match pending.try_take() {
            None => true,
            Some(Ok(_)) => false,
            Some(Err(error)) => {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("clip-sound not kept: {error}"));
                false
            }
        });
        let (target, clip_id) = (target?, clip_id?);
        if let Some((uri, id, done)) = &self.reading {
            if let Ok(sound) = done.try_recv() {
                self.current = Some((uri.clone(), id.clone(), sound));
                self.reading = None;
            }
        }
        let is = |uri: &str, id: &str| uri == target.uri() && id == clip_id;
        if let Some((_, _, sound)) = self.current.as_ref().filter(|(uri, id, _)| is(uri, id)) {
            return Some(sound.clone());
        }
        if !self.reading.as_ref().is_some_and(|(uri, id, _)| is(uri, id)) {
            let (send, done) = mpsc::channel();
            let (owned, clip) = (target.clone(), clip_id.to_string());
            std::thread::spawn(move || {
                let sound = read(&owned, &clip).unwrap_or_else(|error| {
                    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("clip-sound not read: {error}"));
                    None
                });
                let _ = send.send(sound);
            });
            self.reading = Some((target.uri().to_string(), clip_id.to_string(), done));
        }
        None
    }

    /// `sound` is kept for its clip (written in the background; the form never waits).
    pub fn keep(&mut self, target: &ProjectDbTarget, sound: ClipSound) {
        if self.current.as_ref().is_some_and(|(uri, id, kept)| uri == target.uri() && *id == sound.clip_id && kept.as_ref() == Some(&sound)) {
            return;
        }
        if !self.writer.as_ref().is_some_and(|(uri, _)| uri == target.uri()) {
            match ProjectDbWriter::spawn(target.clone(), vec![ClipSoundModule::factory()]) {
                Ok(writer) => self.writer = Some((target.uri().to_string(), writer)),
                Err(error) => {
                    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("clip-sound not kept: {error}"));
                    return;
                }
            }
        }
        let Some((_, writer)) = &self.writer else { return };
        match encode(&Operation::Set(sound.clone())).and_then(|payload| writer.submit(MODULE_ID, payload)) {
            Ok(pending) => self.pending.push(pending),
            Err(error) => qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("clip-sound not kept: {error}")),
        }
        self.current = Some((target.uri().to_string(), sound.clip_id.clone(), Some(sound)));
    }
}

#[cfg(test)]
mod tests;
