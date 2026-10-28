//! The script of a story written out for the people after the field crew (user 2026-10-28:
//! the field crew sends the text to the editor and the proofreader; a newsroom takes a story
//! by MOS). Three forms of the same script:
//!
//! - clean text, to send by mail or a message when nothing else reaches the newsroom;
//! - the two-column A/V script (picture and directions left, sound and the words read right,
//!   length at the side) as an HTML page every computer opens and prints;
//! - a MOS `roStorySend` story (MOS 2.8): the words read as paragraphs, the cues and
//!   directions as presenter instructions, the story sheet as external metadata.
//!
//! The broadcast rule of writing: directions, cue labels and graphics in capitals, the words
//! read aloud as they are written. It reads and writes nothing; the caller gives the script
//! and keeps the files.

pub use qnc_script_profile::Labels;
use qnc_script_profile::Role;

pub const MODULE_ID: &str = "qnc.module.script-export";

/// The script to write out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Story {
    /// The name of the project (the slug when the sheet has none).
    pub title: String,
    /// The story sheet as it is shown: name and value of each field with a value.
    pub header: Vec<(String, String)>,
    pub in_cue: String,
    pub out_cue: String,
    /// The length of the story, its target and the anchor's time, as `m:ss`.
    pub total: String,
    pub target: Option<String>,
    pub anchor: String,
    /// The words of the written-out script, from the profile the project is written with.
    pub labels: Labels,
    pub items: Vec<Item>,
}

/// One block of the script.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Its line on the picture side, as its kind writes it (`OFF 2 (B-ROLL)`,
    /// `SOT 1: Mironik 2002 10:00:02:00 - 10:00:06:49`).
    pub heading: String,
    /// What it is (none for a kind the profile does not have).
    pub role: Option<Role>,
    pub text: String,
    /// Who is heard (`REPORTER (VO)`, `IVAN MARIĆ (SOT)`); empty: nobody named.
    pub speaker: String,
    /// Its length as `m:ss` (empty for a note).
    pub length: String,
    /// Its length in seconds and whether it is in the length of the story (its place in the
    /// running time of the package).
    pub seconds: f64,
    pub in_story: bool,
    /// Name and value of each of its fields with a value.
    pub fields: Vec<(String, String)>,
    /// Its in-cue and out-cue when its kind shows them.
    pub cues: Option<(String, String)>,
    /// The timecode of its recording, when it was recorded.
    pub recorded_tc: Option<String>,
}

/// Seconds as `m:ss`.
fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// Where an item stands in the story.
struct Place {
    /// From and to of the package's running time, for an item in the length of the story.
    running: Option<(f64, f64)>,
    /// The package starts here after the anchor's text: the direction written there.
    cut: Option<String>,
}

impl Item {
    /// The picture side: where it runs in the package, what is seen and what the control room
    /// does. Directions, marks and field names in capitals; what a field says as it is written.
    fn picture(&self, place: &Place, labels: &Labels) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(cut) = &place.cut {
            lines.push(cut.to_uppercase());
        }
        if let Some((from, to)) = place.running {
            lines.push(format!("{} - {}", clock(from), clock(to)));
        }
        lines.push(self.heading.to_uppercase());
        lines.extend(self.fields.iter().map(|(name, value)| format!("{}: {value}", name.to_uppercase())));
        if let Some(tc) = self.recorded_tc.as_ref().filter(|_| !labels.recorded.is_empty()) {
            lines.push(labels.recorded.replace("{tc}", tc));
        }
        lines
    }

    /// Its cues as the profile writes them.
    fn cue_line(&self, labels: &Labels) -> Option<String> {
        let (first, last) = self.cues.as_ref()?;
        Some(labels.cues.replace("{in}", first).replace("{out}", last))
    }

    /// Where the mark stands among the picture lines (after the cut and the running time).
    fn mark_line(place: &Place) -> usize {
        usize::from(place.cut.is_some()) + usize::from(place.running.is_some())
    }

    /// The sound side: who is heard, the words (a statement in quotes), its cues.
    fn sound(&self, labels: &Labels) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.speaker.is_empty() {
            lines.push(format!("{}:", self.speaker.to_uppercase()));
        }
        if !self.text.trim().is_empty() {
            lines.push(if self.role == Some(Role::Clip) { format!("\"{}\"", self.text.trim()) } else { self.text.trim().to_string() });
        }
        lines.extend(self.cue_line(labels));
        lines
    }
}

impl Story {
    /// The head of every form: the sheet, the cues and the lengths.
    fn head_lines(&self) -> Vec<(String, String)> {
        let labels = &self.labels;
        let mut lines = self.header.clone();
        lines.push((labels.in_cue.clone(), self.in_cue.clone()));
        lines.push((labels.out_cue.clone(), self.out_cue.clone()));
        lines.push((labels.trt.clone(), match &self.target {
            Some(target) => format!("{} / {target}", self.total),
            None => self.total.clone(),
        }));
        lines.push((labels.anchor.clone(), self.anchor.clone()));
        lines
    }

    /// Each item with its place: the running time of the package counts from its first item in
    /// the length of the story; the cut to the package stands on that item when anchor text
    /// comes before it.
    fn placed(&self) -> Vec<(&Item, Place)> {
        let (mut at, mut started, mut anchor_before) = (0.0, false, false);
        self.items
            .iter()
            .map(|item| {
                let running = item.in_story.then(|| {
                    let from = at;
                    at += item.seconds.max(0.0);
                    (from, at)
                });
                let cut = (item.in_story && !started && anchor_before && !self.labels.cut_to_package.is_empty()).then(|| self.labels.cut_to_package.clone());
                started |= item.in_story;
                anchor_before |= item.role == Some(Role::Anchor);
                (item, Place { running, cut })
            })
            .collect()
    }
}

/// Clean text: the head, then each block with its mark and length, its picture lines and words.
pub fn text(story: &Story) -> String {
    let mut out = format!("{}\n", story.title.to_uppercase());
    for (name, value) in story.head_lines() {
        out.push_str(&format!("{}: {value}\n", name.to_uppercase()));
    }
    for (item, place) in story.placed() {
        out.push('\n');
        let mark = Item::mark_line(&place);
        for (index, line) in item.picture(&place, &story.labels).iter().enumerate() {
            match item.length.as_str() {
                length if index == mark && !length.is_empty() => out.push_str(&format!("{line} ({length})\n")),
                _ => out.push_str(&format!("{line}\n")),
            }
        }
        for line in item.sound(&story.labels) {
            out.push_str(&format!("{line}\n"));
        }
    }
    out
}

/// The two-column A/V script as an HTML page (it prints on A4).
pub fn av_html(story: &Story) -> String {
    let mut out = String::from(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\n<style>\nbody{font-family:Arial,Helvetica,sans-serif;font-size:12pt;margin:16mm}\nh1{font-size:16pt}\ntable{border-collapse:collapse;width:100%}\nth,td{border:1px solid #888;padding:6px;vertical-align:top}\n.head td{border:none;padding:2px 8px 2px 0}\n.pic{width:38%}\n.len{width:10%;text-align:right;white-space:nowrap}\n.cue{color:#555;font-size:10pt}\n</style>\n",
    );
    out.push_str(&format!("<title>{}</title></head><body>\n<h1>{}</h1>\n<table class=\"head\">\n", html(&story.title), html(&story.title.to_uppercase())));
    for (name, value) in story.head_lines() {
        out.push_str(&format!("<tr><td><b>{}</b></td><td>{}</td></tr>\n", html(&name.to_uppercase()), html(&value)));
    }
    let labels = &story.labels;
    out.push_str(&format!(
        "</table>\n<br>\n<table>\n<tr><th>{}</th><th>{}</th><th>{}</th></tr>\n",
        html(&labels.video),
        html(&labels.audio),
        html(&labels.duration)
    ));
    for (item, place) in story.placed() {
        let picture = item.picture(&place, &story.labels).iter().map(|line| html(line)).collect::<Vec<_>>().join("<br>");
        let mut sound: Vec<String> = Vec::new();
        if !item.speaker.is_empty() {
            sound.push(format!("<b>{}:</b>", html(&item.speaker.to_uppercase())));
        }
        if !item.text.trim().is_empty() {
            let words = html(item.text.trim());
            sound.push(if item.role == Some(Role::Clip) { format!("&quot;{words}&quot;") } else { words });
        }
        if let Some(line) = item.cue_line(&story.labels) {
            sound.push(format!("<span class=\"cue\">{}</span>", html(&line)));
        }
        out.push_str(&format!("<tr><td class=\"pic\">{picture}</td><td>{}</td><td class=\"len\">{}</td></tr>\n", sound.join("<br>"), html(&item.length)));
    }
    out.push_str("</table>\n</body></html>\n");
    out
}

/// What a MOS story needs besides the script.
#[derive(Debug, Clone, PartialEq)]
pub struct MosIds {
    /// The MOS id of QNC and of the newsroom system it is sent to.
    pub mos_id: String,
    pub ncs_id: String,
    /// The running order and story ids (the project's).
    pub ro_id: String,
    pub story_id: String,
    pub message_id: u64,
}

/// A MOS `roStorySend` message: the words read as paragraphs, every mark, picture line and cue as
/// a presenter instruction (`<pi>`), the sheet and lengths as external metadata.
pub fn mos_xml(story: &Story, ids: &MosIds) -> String {
    let mut body = String::new();
    for (item, place) in story.placed() {
        for line in item.picture(&place, &story.labels) {
            body.push_str(&format!("<p><pi>{}</pi></p>\n", xml(&line)));
        }
        if !item.speaker.is_empty() {
            body.push_str(&format!("<p><pi>{}:</pi></p>\n", xml(&item.speaker.to_uppercase())));
        }
        if !item.text.trim().is_empty() {
            body.push_str(&format!("<p>{}</p>\n", xml(item.text.trim())));
        }
        if let Some(line) = item.cue_line(&story.labels) {
            body.push_str(&format!("<p><pi>{}</pi></p>\n", xml(&line)));
        }
    }
    let metadata: String = story
        .head_lines()
        .iter()
        .map(|(name, value)| format!("<field name=\"{}\">{}</field>", xml(name), xml(value)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<mos>\n<mosID>{}</mosID>\n<ncsID>{}</ncsID>\n<messageID>{}</messageID>\n<roStorySend>\n<roID>{}</roID>\n<storyID>{}</storyID>\n<storySlug>{}</storySlug>\n<storyNum></storyNum>\n<storyBody>\n{body}</storyBody>\n<mosExternalMetadata>\n<mosScope>STORY</mosScope>\n<mosSchema>urn:qnc:script-sheet:1</mosSchema>\n<mosPayload>\n{metadata}\n</mosPayload>\n</mosExternalMetadata>\n</roStorySend>\n</mos>\n",
        xml(&ids.mos_id),
        xml(&ids.ncs_id),
        ids.message_id,
        xml(&ids.ro_id),
        xml(&ids.story_id),
        xml(&story.title),
    )
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests;
