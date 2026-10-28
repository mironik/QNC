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
    pub items: Vec<Item>,
}

/// One block of the script.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Its mark (`OFF 2`, `Take SOT 1`, `*CG 2line`).
    pub mark: String,
    /// What it is (none for a kind the profile does not have).
    pub role: Option<Role>,
    pub text: String,
    /// The clip of a clip block: its name, start and end timecodes.
    pub clip: Option<(String, String, String)>,
    /// Its length as `m:ss` (empty for a note).
    pub length: String,
    /// Name and value of each of its fields with a value.
    pub fields: Vec<(String, String)>,
    /// Its in-cue and out-cue when its kind shows them.
    pub cues: Option<(String, String)>,
    /// The timecode of its recording, when it was recorded.
    pub recorded_tc: Option<String>,
}

impl Item {
    /// The picture side: what is seen and what the control room does (in capitals).
    fn picture(&self) -> Vec<String> {
        let mut lines = vec![match (self.role, &self.clip) {
            (Some(Role::Anchor), _) => format!("{} (ON CAM)", self.mark),
            (_, Some((name, start, end))) => format!("{}: {name} {start} - {end}", self.mark),
            (Some(Role::Spoken), _) => format!("{} (B-ROLL)", self.mark),
            _ => self.mark.clone(),
        }];
        lines.extend(self.fields.iter().map(|(name, value)| format!("{name}: {value}")));
        if let Some(tc) = &self.recorded_tc {
            lines.push(format!("VO TC {tc}"));
        }
        lines.into_iter().map(|line| line.to_uppercase()).collect()
    }

    /// The sound side: the words, a statement in quotes, its cues.
    fn sound(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.text.trim().is_empty() {
            lines.push(if self.role == Some(Role::Clip) { format!("\"{}\"", self.text.trim()) } else { self.text.trim().to_string() });
        }
        if let Some((first, last)) = &self.cues {
            lines.push(format!("IN: {first}   OUT: {last}"));
        }
        lines
    }
}

impl Story {
    /// The head of every form: the sheet, the cues and the lengths.
    fn head_lines(&self) -> Vec<(String, String)> {
        let mut lines = self.header.clone();
        lines.push(("IN-CUE".into(), self.in_cue.clone()));
        lines.push(("OUT-CUE".into(), self.out_cue.clone()));
        lines.push(("TRT".into(), match &self.target {
            Some(target) => format!("{} / {target}", self.total),
            None => self.total.clone(),
        }));
        lines.push(("ANCHOR".into(), self.anchor.clone()));
        lines
    }
}

/// Clean text: the head, then each block with its mark and length, its picture lines and words.
pub fn text(story: &Story) -> String {
    let mut out = format!("{}\n", story.title.to_uppercase());
    for (name, value) in story.head_lines() {
        out.push_str(&format!("{}: {value}\n", name.to_uppercase()));
    }
    for item in &story.items {
        out.push('\n');
        let picture = item.picture();
        match item.length.as_str() {
            "" => out.push_str(&format!("{}\n", picture[0])),
            length => out.push_str(&format!("{} ({length})\n", picture[0])),
        }
        for line in picture.iter().skip(1) {
            out.push_str(&format!("{line}\n"));
        }
        for line in item.sound() {
            out.push_str(&format!("{line}\n"));
        }
    }
    out
}

/// The two-column A/V script as an HTML page (it prints on A4).
pub fn av_html(story: &Story) -> String {
    let mut out = String::from(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\n<style>\nbody{font-family:Arial,Helvetica,sans-serif;font-size:12pt;margin:16mm}\nh1{font-size:16pt}\ntable{border-collapse:collapse;width:100%}\nth,td{border:1px solid #888;padding:6px;vertical-align:top}\n.head td{border:none;padding:2px 8px 2px 0}\n.pic{width:38%;text-transform:uppercase}\n.len{width:10%;text-align:right;white-space:nowrap}\n.cue{color:#555;font-size:10pt}\n</style>\n",
    );
    out.push_str(&format!("<title>{}</title></head><body>\n<h1>{}</h1>\n<table class=\"head\">\n", html(&story.title), html(&story.title.to_uppercase())));
    for (name, value) in story.head_lines() {
        out.push_str(&format!("<tr><td><b>{}</b></td><td>{}</td></tr>\n", html(&name.to_uppercase()), html(&value)));
    }
    out.push_str("</table>\n<br>\n<table>\n<tr><th>VIDEO</th><th>AUDIO</th><th>DUR</th></tr>\n");
    for item in &story.items {
        let picture = item.picture().iter().map(|line| html(line)).collect::<Vec<_>>().join("<br>");
        let mut sound: Vec<String> = Vec::new();
        if !item.text.trim().is_empty() {
            let words = html(item.text.trim());
            sound.push(if item.role == Some(Role::Clip) { format!("&quot;{words}&quot;") } else { words });
        }
        if let Some((first, last)) = &item.cues {
            sound.push(format!("<span class=\"cue\">IN: {} &nbsp; OUT: {}</span>", html(first), html(last)));
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
    for item in &story.items {
        for line in item.picture() {
            body.push_str(&format!("<p><pi>{}</pi></p>\n", xml(&line)));
        }
        if !item.text.trim().is_empty() {
            body.push_str(&format!("<p>{}</p>\n", xml(item.text.trim())));
        }
        if let Some((first, last)) = &item.cues {
            body.push_str(&format!("<p><pi>IN: {} OUT: {}</pi></p>\n", xml(first), xml(last)));
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
