//! The generic pass of a form's application layer through its pieces (user 2026-10-23:
//! "Ingest into blocks", way B: a new possibility needs no new line in the application).
//! A piece takes the actions it knows and gives what changed; the application only hands
//! every action and every poll to the pieces in order, the first piece that takes an
//! action answers it. A piece knows no form and no application: it sees the clip list,
//! the chosen pool tab, the clip on the preview, the IN/OUT of the source timeline and
//! the project database, and nothing else. The database stays the truth.

use qnc_clip_row::ClipRow;
use qnc_db_broker::ProjectDbTarget;

pub const MODULE_ID: &str = "qnc.module.form-pieces";

/// What a piece sees of the form when it acts or polls.
pub struct Now<'a> {
    /// The clip list the form shows (a piece may mark clips or put posters on them).
    pub clips: &'a mut Vec<ClipRow>,
    /// The chosen pool tab (its action id); `None`: the first.
    pub tab: &'a mut Option<String>,
    /// Only clips new to this Select are shown (filter New).
    pub only_new: bool,
    /// The clip on the preview.
    pub chosen: Option<&'a str>,
    /// IN/OUT the source timeline shows for that clip.
    pub marks: Option<(u64, u64)>,
    /// The project database of the loaded project.
    pub project: Option<&'a ProjectDbTarget>,
    /// Where this computer reads a picture of a clip (a piece may show a frame of it).
    pub stills: Option<qnc_still_locator::StillLocator>,
}

impl Now<'_> {
    /// The clips the pool shows of the clip list.
    pub fn visible(&self) -> impl Iterator<Item = &ClipRow> {
        self.clips.iter().filter(|clip| !self.only_new || !clip.previously_seen)
    }
}

/// What a piece made of an action or a poll.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Answer {
    pub changed: bool,
    /// Shown to the user.
    pub message: Option<String>,
    /// The action was refused (its message says why).
    pub refused: bool,
    /// The preview opens this clip with these IN/OUT.
    pub open: Option<(String, (u64, u64))>,
}

impl Answer {
    pub fn changed() -> Self {
        Self { changed: true, ..Self::default() }
    }

    pub fn refused(message: impl Into<String>) -> Self {
        Self { changed: true, message: Some(message.into()), refused: true, open: None }
    }

    pub fn said(message: impl Into<String>) -> Self {
        Self { changed: true, message: Some(message.into()), ..Self::default() }
    }

    fn join(&mut self, other: Answer) {
        self.changed |= other.changed;
        self.refused |= other.refused;
        self.message = other.message.or(self.message.take());
        self.open = other.open.or(self.open.take());
    }
}

/// One piece of a form.
pub trait Piece: std::fmt::Debug + Send {
    /// The answer to `action_id` (`payload`: the clip or row id it names), `None` when this
    /// piece does not take the action.
    fn act(&mut self, action_id: &str, payload: Option<&str>, now: &mut Now<'_>) -> Option<Answer>;

    /// Finished background work; the project may have changed.
    fn poll(&mut self, _now: &mut Now<'_>) -> Answer {
        Answer::default()
    }

    /// The actions this piece has work for now (Uvezi while its parts wait), so the form offers them.
    fn ready(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// The rows the pool shows on `tab` instead of the clip list, when the tab is this piece's.
    fn rows(&self, _tab: &str, _clips: &[ClipRow]) -> Option<Vec<ClipRow>> {
        None
    }
}

/// What the pieces of a form show it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Shown {
    /// The rows the pool shows on the chosen tab instead of the clip list.
    pub rows: Option<Vec<ClipRow>>,
    /// The actions a piece has work for (the form offers them).
    pub ready: Vec<String>,
}

/// The pieces of a form, in the order they are asked.
#[derive(Debug, Default)]
pub struct Pieces(Vec<Box<dyn Piece>>);

impl Pieces {
    pub fn new(pieces: Vec<Box<dyn Piece>>) -> Self {
        Self(pieces)
    }

    /// The first piece that takes the action answers it.
    pub fn act(&mut self, action_id: &str, payload: Option<&str>, now: &mut Now<'_>) -> Option<Answer> {
        self.0.iter_mut().find_map(|piece| piece.act(action_id, payload, now))
    }

    pub fn poll(&mut self, now: &mut Now<'_>) -> Answer {
        let mut answer = Answer::default();
        for piece in &mut self.0 {
            answer.join(piece.poll(now));
        }
        answer
    }

    /// What the pieces show now: the rows of the chosen tab and the actions they have work for.
    pub fn shown(&self, tab: Option<&str>, clips: &[ClipRow]) -> Shown {
        Shown { rows: self.rows(tab, clips), ready: self.0.iter().flat_map(|piece| piece.ready()).map(str::to_string).collect() }
    }

    /// The rows of the chosen tab, `None` when the clip list is shown.
    pub fn rows(&self, tab: Option<&str>, clips: &[ClipRow]) -> Option<Vec<ClipRow>> {
        let tab = tab?;
        self.0.iter().find_map(|piece| piece.rows(tab, clips))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Echo;

    impl Piece for Echo {
        fn act(&mut self, action_id: &str, payload: Option<&str>, _now: &mut Now<'_>) -> Option<Answer> {
            (action_id == "echo").then(|| Answer::said(payload.unwrap_or_default()))
        }
        fn rows(&self, tab: &str, _clips: &[ClipRow]) -> Option<Vec<ClipRow>> {
            (tab == "echo_tab").then(Vec::new)
        }
    }

    #[test]
    fn the_first_piece_that_takes_an_action_answers_and_others_pass() {
        let mut pieces = Pieces::new(vec![Box::new(Echo)]);
        let (mut clips, mut tab) = (Vec::new(), None);
        let mut now = Now { clips: &mut clips, tab: &mut tab, only_new: false, chosen: None, marks: None, project: None, stills: None };
        assert_eq!(pieces.act("echo", Some("hi"), &mut now).and_then(|answer| answer.message).as_deref(), Some("hi"));
        assert!(pieces.act("other", None, &mut now).is_none());
        assert_eq!(pieces.rows(Some("echo_tab"), &[]), Some(Vec::new()));
        assert_eq!(pieces.rows(Some("all"), &[]), None);
        assert_eq!(pieces.poll(&mut now), Answer::default());
    }
}
