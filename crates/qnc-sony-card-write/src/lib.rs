//! The parts of a Sony MediaProfile clip written as a card of their own (user 2026-10-23:
//! "make a card inside the original folder with XML files as on the original card, the
//! original name and a number, so the parts are imported the same way as the originals").
//! The documents of the original are copied and only what the part changes is set: its
//! `MEDIAPRO.XML` entry (file, length, a UMID of its own, no proxy), its `M01.XML` (UMID,
//! length, timecode from its first frame, frame counts of the change tables inside it) and
//! its thumbnail. The original card is only read. Writes go to the parts card only.

use std::{
    fs,
    path::{Path, PathBuf},
};

pub const MODULE_ID: &str = "qnc.module.sony-card-write";

const MEDIAPRO: &str = "MEDIAPRO.XML";

/// Where a clip of a Sony card is: the folder of its `MEDIAPRO.XML` and its material URI there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardClip {
    pub mediapro_dir: PathBuf,
    /// The path of the folder of `MEDIAPRO.XML` from the root of the card (`XDROOT`, `PRIVATE/M4ROOT`).
    pub card_relative: PathBuf,
    /// As `MEDIAPRO.XML` names it (`./Clip/Mironik 3396.MXF`).
    pub material_uri: String,
}

/// The card of `media` (a file under `card_root`), when its `MEDIAPRO.XML` lists it.
pub fn card_of(card_root: &Path, media: &Path) -> Option<CardClip> {
    let mut dir = media.parent()?;
    while dir.starts_with(card_root) {
        let mediapro = dir.join(MEDIAPRO);
        if mediapro.is_file() {
            let relative = media.strip_prefix(dir).ok()?.to_string_lossy().replace('\\', "/");
            let material_uri = format!("./{relative}");
            let text = fs::read_to_string(&mediapro).ok()?;
            if text.contains(&format!("<Material uri=\"{material_uri}\"")) {
                let card_relative = dir.strip_prefix(card_root).ok()?.to_path_buf();
                return Some(CardClip { mediapro_dir: dir.to_path_buf(), card_relative, material_uri });
            }
            return None;
        }
        dir = dir.parent()?;
    }
    None
}

/// The name of part `number` of a clip: its original name and the number (`Mironik 3396_01`).
pub fn part_stem(original_stem: &str, number: u32) -> String {
    format!("{original_stem}_{number:02}")
}

/// The part of a card clip as written: its media file (the caller cuts it there), and the
/// documents [`write_documents`] writes beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartFiles {
    pub stem: String,
    pub material_uri: String,
    pub media: PathBuf,
}

/// The files of part `number` of `clip` on the parts card whose `MEDIAPRO.XML` folder is `dest`.
pub fn part_files(clip: &CardClip, dest: &Path, number: u32) -> Option<PartFiles> {
    let (folder, file) = clip.material_uri.rsplit_once('/')?;
    let (stem, extension) = file.rsplit_once('.')?;
    let stem = part_stem(stem, number);
    let material_uri = format!("{folder}/{stem}.{extension}");
    Some(PartFiles { media: dest.join(material_uri.trim_start_matches("./")), stem, material_uri })
}

/// What the part is of its original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartFrames {
    pub number: u32,
    /// The frame of the original the part starts on.
    pub first: u64,
    pub frames: u64,
}

/// Writes the documents of `part` to the parts card (`dest` is the folder of its
/// `MEDIAPRO.XML`) from those of the original clip; `poster` becomes its thumbnail.
pub fn write_documents(clip: &CardClip, dest: &Path, files: &PartFiles, part: PartFrames, poster: Option<&Path>) -> Result<(), String> {
    let source = fs::read_to_string(clip.mediapro_dir.join(MEDIAPRO)).map_err(|e| format!("MEDIAPRO.XML: {e}"))?;
    let material = element(&source, &format!("<Material uri=\"{}\"", clip.material_uri), "</Material>")
        .ok_or_else(|| qnc_ui_text::text("card.clip_not_in_mediapro").to_string())?;
    let (_, old_file) = clip.material_uri.rsplit_once('/').ok_or("material uri")?;
    let old_stem = old_file.rsplit_once('.').map_or(old_file, |(stem, _)| stem);
    let renamed = |uri: &str| uri.replace(&format!("/{old_stem}"), &format!("/{}", files.stem));
    // The entry of the part: its file, its length, a UMID of its own, its documents, no proxy.
    let mut entry = without_element(material, "<Proxy ", "/>");
    entry = set_attribute(&entry, "uri", &files.material_uri);
    entry = set_attribute(&entry, "dur", &part.frames.to_string());
    if let Some(umid) = attribute(&entry, "umid") {
        entry = set_attribute(&entry, "umid", &part_umid(&umid, part.number));
    }
    let mut documents = Vec::new();
    let mut at = 0;
    while let Some(found) = entry[at..].find("<RelevantInfo ") {
        let start = at + found;
        let end = start + entry[start..].find("/>").ok_or("RelevantInfo")? + 2;
        let info = &entry[start..end];
        if let (Some(uri), Some(kind)) = (attribute(info, "uri"), attribute(info, "type")) {
            documents.push((uri.clone(), renamed(&uri), kind));
        }
        at = end;
    }
    for (old, new, _) in &documents {
        entry = entry.replace(&format!("uri=\"{old}\""), &format!("uri=\"{new}\""));
    }
    for (old, new, kind) in &documents {
        let from = clip.mediapro_dir.join(old.trim_start_matches("./"));
        let to = dest.join(new.trim_start_matches("./"));
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        match kind.as_str() {
            "XML" => {
                let text = fs::read_to_string(&from).map_err(|e| format!("{old}: {e}"))?;
                fs::write(&to, part_sidecar(&text, part)?).map_err(|e| e.to_string())?;
            }
            "JPG" => {
                if let Some(poster) = poster {
                    fs::copy(poster, &to).map_err(|e| e.to_string())?;
                }
            }
            _ => {}
        }
    }
    let mediapro = dest.join(MEDIAPRO);
    let index = match fs::read_to_string(&mediapro) {
        Ok(existing) => existing,
        // A new parts card: the original's MEDIAPRO.XML without its clips.
        Err(_) => {
            let open = source.find("<Contents>").ok_or("MEDIAPRO.XML: Contents")? + "<Contents>".len();
            let close = source.find("</Contents>").ok_or("MEDIAPRO.XML: Contents")?;
            format!("{}\n\t{}", &source[..open], &source[close..])
        }
    };
    let index = match element(&index, &format!("<Material uri=\"{}\"", files.material_uri), "</Material>") {
        Some(old) => index.replacen(old, &entry, 1),
        None => {
            let close = index.find("</Contents>").ok_or("MEDIAPRO.XML: Contents")?;
            format!("{}{}\n\t{}", index[..close].trim_end_matches(['\t', ' ']), entry, &index[close..])
        }
    };
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    fs::write(&mediapro, index).map_err(|e| e.to_string())
}

/// The `M01.XML` of the part from the original's: UMIDs of its own, its length, the
/// timecode of its first frame and the frame counts of the change tables inside the part.
fn part_sidecar(text: &str, part: PartFrames) -> Result<String, String> {
    let mut out = text.to_string();
    for name in ["umidRef", "value"] {
        let mut at = 0;
        while let Some(found) = out[at..].find(&format!("{name}=\"060A2B34")) {
            let start = at + found + name.len() + 2;
            let end = start + out[start..].find('"').ok_or("umid")?;
            let umid = part_umid(&out[start..end], part.number);
            out.replace_range(start..end, &umid);
            at = end;
        }
    }
    if let Some(duration) = element(&out, "<Duration ", "/>") {
        let new = set_attribute(duration, "value", &part.frames.to_string());
        out = out.replacen(duration, &new, 1);
    }
    if let Some(table) = element(&out, "<LtcChangeTable", "</LtcChangeTable>").map(str::to_string) {
        out = out.replacen(&table, &ltc_table(&table, part)?, 1);
    }
    // Change tables keep their state at the start of the part, end at its last frame.
    let mut at = 0;
    while let Some(found) = out[at..].find("frameCount=\"") {
        let start = at + found + "frameCount=\"".len();
        let end = start + out[start..].find('"').ok_or("frameCount")?;
        let element_end = end + out[end..].find('>').unwrap_or(0);
        let is_end = out[end..element_end].contains("status=\"end\"");
        let value: u64 = out[start..end].parse().unwrap_or(0);
        let last = part.frames.saturating_sub(1);
        let new = if is_end { last } else { value.saturating_sub(part.first).min(last) };
        out.replace_range(start..end, &new.to_string());
        at = start + new.to_string().len();
    }
    Ok(out)
}

/// The LTC table of the part: the timecode of its first and of its last frame.
fn ltc_table(table: &str, part: PartFrames) -> Result<String, String> {
    let head = &table[..table.find('>').ok_or("LtcChangeTable")? + 1];
    let tc_fps: u64 = attribute(head, "tcFps").and_then(|v| v.parse().ok()).ok_or("tcFps")?;
    let half = attribute(head, "halfStep").is_some_and(|v| v == "true");
    let rate = if half { tc_fps * 2 } else { tc_fps };
    let changes: Vec<(u64, String)> = table
        .match_indices("<LtcChange ")
        .filter_map(|(at, _)| {
            let item = &table[at..at + table[at..].find("/>")?];
            Some((attribute(item, "frameCount")?.parse().ok()?, attribute(item, "value")?))
        })
        .collect();
    let (base_frame, base_value) = changes.iter().filter(|(frame, _)| *frame <= part.first).last().ok_or("LtcChange")?;
    let start = ltc_to_frames(base_value, tc_fps, half)? + (part.first - base_frame);
    let end = start + part.frames.saturating_sub(1);
    let indent = table.lines().nth(1).map(|line| &line[..line.len() - line.trim_start().len()]).unwrap_or("\t\t");
    let close_indent = table.lines().last().map(|line| &line[..line.len() - line.trim_start().len()]).unwrap_or("\t");
    Ok(format!(
        "{head}\n{indent}<LtcChange frameCount=\"0\" value=\"{}\" status=\"increment\"/>\n{indent}<LtcChange frameCount=\"{}\" value=\"{}\" status=\"end\"/>\n{close_indent}</LtcChangeTable>",
        frames_to_ltc(start, rate, tc_fps, half),
        part.frames.saturating_sub(1),
        frames_to_ltc(end, rate, tc_fps, half)
    ))
}

/// A Sony LTC value (BCD bytes FF SS MM HH at `tc_fps`; with `half` the second clip frame
/// of each LTC frame is the top bit of the hours byte) as frames at the clip rate.
fn ltc_to_frames(value: &str, tc_fps: u64, half: bool) -> Result<u64, String> {
    let bytes: Vec<u8> = (0..4).map(|i| u8::from_str_radix(value.get(i * 2..i * 2 + 2).unwrap_or(""), 16)).collect::<Result<_, _>>().map_err(|_| "LTC")?;
    let bcd = |b: u8| u64::from(b >> 4) * 10 + u64::from(b & 0x0F);
    let (ff, ss, mm, hh) = (bcd(bytes[0] & 0x3F), bcd(bytes[1] & 0x7F), bcd(bytes[2] & 0x7F), bcd(bytes[3] & 0x3F));
    let second = u64::from(half && bytes[3] & 0x80 != 0);
    let ratio = if half { 2 } else { 1 };
    Ok(((hh * 60 + mm) * 60 + ss) * tc_fps * ratio + ff * ratio + second)
}

fn frames_to_ltc(frames: u64, rate: u64, tc_fps: u64, half: bool) -> String {
    let frames = frames % (24 * 3600 * rate);
    let (seconds, rest) = (frames / rate, frames % rate);
    let ratio = rate / tc_fps.max(1);
    let (ff, second) = (rest / ratio, rest % ratio);
    let bcd = |v: u64| (((v / 10) << 4) | (v % 10)) as u8;
    let hh = bcd(seconds / 3600) | if half && second == 1 { 0x80 } else { 0 };
    format!("{:02X}{:02X}{:02X}{:02X}", bcd(ff), bcd(seconds % 60), bcd(seconds / 60 % 60), hh)
}

/// A UMID of the part: the original's with its instance number set to the part's number
/// (SMPTE 330: the instance number tells copies and derivations of one material apart).
fn part_umid(umid: &str, number: u32) -> String {
    if umid.len() < 32 || !umid.is_ascii() {
        return umid.to_string();
    }
    format!("{}{:06X}{}", &umid[..26], number & 0xFF_FFFF, &umid[32..])
}

fn element<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)?;
    let end = start + text[start..].find(close)? + close.len();
    Some(&text[start..end])
}

fn without_element(text: &str, open: &str, close: &str) -> String {
    match element(text, open, close) {
        Some(found) => {
            let start = text.find(found).unwrap_or(0);
            let line_start = text[..start].rfind('\n').unwrap_or(start);
            text[..line_start].to_string() + &text[start + found.len()..]
        }
        None => text.to_string(),
    }
}

/// Where the value of attribute `name` is in `text` (the attribute after any white space).
fn value_range(text: &str, name: &str) -> Option<(usize, usize)> {
    let key = format!("{name}=\"");
    let mut at = 0;
    while let Some(found) = text[at..].find(&key) {
        let start = at + found;
        if text[..start].ends_with(|c: char| c.is_whitespace()) {
            let value = start + key.len();
            return Some((value, value + text[value..].find('"')?));
        }
        at = start + key.len();
    }
    None
}

fn attribute(text: &str, name: &str) -> Option<String> {
    value_range(text, name).map(|(start, end)| text[start..end].to_string())
}

fn set_attribute(text: &str, name: &str, value: &str) -> String {
    match value_range(text, name) {
        Some((start, end)) => format!("{}{value}{}", &text[..start], &text[end..]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests;
