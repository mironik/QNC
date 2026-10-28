//! A title card (user 2026-10-28: a recorded voice-over gets a picture with its text, so it is
//! a clip like any other and the Broadcast Player plays it): white text on black, the size of
//! the project picture, the words wrapped to the width and made smaller until all fit. The
//! font is the one the QNC interface draws with, so the card is the same on every computer.
//!
//! It knows no form, script or database; it only makes the picture.

use std::path::Path;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgb, RgbImage};

pub const MODULE_ID: &str = "qnc.module.title-card";

/// The margin around the text, a share of the height.
const MARGIN: f32 = 0.08;

/// The card of `text`, `width` x `height`; `heading` is a small line above it.
pub fn render(text: &str, heading: &str, width: u32, height: u32) -> Result<RgbImage, String> {
    if width == 0 || height == 0 {
        return Err("A title card needs a size.".into());
    }
    let font = FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).map_err(|e| e.to_string())?;
    let mut card = RgbImage::from_pixel(width, height, Rgb([0, 0, 0]));
    let margin = height as f32 * MARGIN;
    let area = (width as f32 - 2.0 * margin, height as f32 - 2.0 * margin);
    let small = height as f32 / 24.0;
    let mut top = margin;
    if !heading.is_empty() {
        draw_line(&mut card, &font, heading, PxScale::from(small), margin, top, Rgb([170, 170, 170]));
        top += small * 1.8;
    }
    // The biggest size at which the words fit; one tenth of the height at most.
    let mut size = height as f32 / 10.0;
    let lines = loop {
        let scale = PxScale::from(size);
        let lines = wrap(&font, scale, text, area.0);
        let needed = lines.len() as f32 * size * 1.25;
        if needed <= area.1 - (top - margin) || size <= small {
            break lines;
        }
        size *= 0.9;
    };
    let scale = PxScale::from(size);
    for line in lines {
        if top + size > height as f32 - margin * 0.5 {
            break;
        }
        draw_line(&mut card, &font, &line, scale, margin, top, Rgb([255, 255, 255]));
        top += size * 1.25;
    }
    Ok(card)
}

/// Writes the card of `text` as a PNG at `path`.
pub fn write_png(path: &Path, text: &str, heading: &str, width: u32, height: u32) -> Result<(), String> {
    render(text, heading, width, height)?.save(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The words of `text` in lines no wider than `width`.
fn wrap(font: &FontRef<'_>, scale: PxScale, text: &str, width: f32) -> Vec<String> {
    let scaled = font.as_scaled(scale);
    let measure = |line: &str| line.chars().map(|c| scaled.h_advance(font.glyph_id(c))).sum::<f32>();
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && measure(&next) > width {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    lines
}

fn draw_line(card: &mut RgbImage, font: &FontRef<'_>, line: &str, scale: PxScale, left: f32, top: f32, color: Rgb<u8>) {
    let scaled = font.as_scaled(scale);
    let mut x = left;
    let baseline = top + scaled.ascent();
    let (width, height) = card.dimensions();
    for c in line.chars() {
        let id = font.glyph_id(c);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(x, baseline));
        x += scaled.h_advance(id);
        let Some(outline) = font.outline_glyph(glyph) else { continue };
        let bounds = outline.px_bounds();
        outline.draw(|gx, gy, coverage| {
            let (px, py) = (bounds.min.x as i64 + gx as i64, bounds.min.y as i64 + gy as i64);
            if px < 0 || py < 0 || px >= width as i64 || py >= height as i64 {
                return;
            }
            let pixel = card.get_pixel_mut(px as u32, py as u32);
            for (channel, target) in pixel.0.iter_mut().zip(color.0) {
                *channel = (*channel as f32 + (target as f32 - *channel as f32) * coverage.clamp(0.0, 1.0)).round() as u8;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_has_the_size_asked_and_its_words_in_white() {
        let card = render("Upravo ovdje u Međugorju otvara se teniski centar, čiji je vlasnik naš tenisač.", "OFF 1", 1920, 1080).unwrap();
        assert_eq!(card.dimensions(), (1920, 1080));
        let white = card.pixels().filter(|p| p.0 == [255, 255, 255]).count();
        assert!(white > 1000, "the words are drawn: {white}");
        assert_eq!(card.get_pixel(5, 1075).0, [0, 0, 0], "the margin stays black");
    }

    #[test]
    fn a_long_text_is_made_smaller_to_fit() {
        let long = "riječ ".repeat(400);
        let card = render(&long, "", 1280, 720).unwrap();
        assert_eq!(card.get_pixel(640, 715).0, [0, 0, 0], "nothing runs past the bottom margin");
    }
}
