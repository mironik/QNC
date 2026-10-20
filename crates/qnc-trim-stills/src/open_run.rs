//! The edit pictures from a decoder that stays open (user 2026-10-13: "presporo se
//! prikazuju"): one decoder of the decoder catalog per clip, inside this process when the
//! program has the in-process decoder, kept open between pictures; a picture far from
//! where it stands is reached by a jump inside it, not by a new decoder process. The
//! pictures are decoded as `yuv420p` (lighter) and made small here. A program without
//! the in-process decoder keeps the pass of the extractor (`decode_run`).

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use qnc_image_assets::RgbaImage;
use qnc_media_decode::{DecodeRequest, DecodedFormat, Decoder};
use qnc_media_metadata::Rational;
use qnc_program_view::StillPicture;
use qnc_still_locator::LocatedPicture;

/// The size a picture is made at here (the zoom draws up to 480x270).
const SIZE: [usize; 2] = [480, 270];
/// Decoders kept open at most (the oldest closes).
const OPEN: usize = 4;
/// A picture this far ahead of where a decoder stands is read on; farther, it jumps.
const READ_ON: u64 = 30;

struct Open {
    decoder: Decoder,
    /// The clip frame the next picture of the decoder is.
    next: u64,
}

/// The decoders of the zoom, by clip; `in_process` is learned at the first open.
#[derive(Default)]
pub(crate) struct OpenDecoders {
    clips: HashMap<String, Arc<Mutex<Open>>>,
    order: VecDeque<String>,
    in_process: Option<bool>,
}

pub(crate) type Shared = Arc<Mutex<OpenDecoders>>;

/// The decoders of this process (the zoom is one per program).
pub(crate) fn shared() -> Shared {
    static SHARED: std::sync::OnceLock<Shared> = std::sync::OnceLock::new();
    SHARED.get_or_init(Shared::default).clone()
}

/// `count` pictures of the clip from `first` through its open decoder; `None` when this
/// program has no in-process decoder (the caller uses the extractor).
pub(crate) fn decode(
    picture: &StillPicture,
    first: u64,
    count: u32,
    place: &LocatedPicture,
    decoders: &Shared,
    cancel: &AtomicBool,
) -> Option<Result<Vec<(StillPicture, Arc<RgbaImage>)>, String>> {
    let open = {
        let mut all = decoders.lock().ok()?;
        if all.in_process == Some(false) {
            return None;
        }
        match all.clips.get(&picture.clip_id) {
            Some(open) => open.clone(),
            None => match open_decoder(place, first) {
                Ok(Some(open)) => {
                    all.in_process = Some(true);
                    let open = Arc::new(Mutex::new(open));
                    all.clips.insert(picture.clip_id.clone(), open.clone());
                    all.order.push_back(picture.clip_id.clone());
                    while all.order.len() > OPEN {
                        if let Some(old) = all.order.pop_front() {
                            all.clips.remove(&old);
                        }
                    }
                    open
                }
                Ok(None) => {
                    all.in_process = Some(false);
                    return None;
                }
                Err(error) => return Some(Err(error)),
            },
        }
    };
    let mut open = match open.lock() {
        Ok(open) => open,
        Err(_) => return Some(Err(qnc_ui_text::text("story.msg.still_decoder_unavailable").into())),
    };
    Some(read(&mut open, picture, first, count, place, cancel))
}

/// A decoder of the clip from `first`; `None` when it is a decoder process.
fn open_decoder(place: &LocatedPicture, first: u64) -> Result<Option<Open>, String> {
    let config = qnc_decoder_catalog::installed_config().map_err(|error| error.to_string())?;
    let start = start_of(place, first);
    let request = DecodeRequest {
        version: qnc_media_decode::VERSION.into(),
        media: place.media.clone(),
        stream_index: place.stream_index,
        start,
        output_pixel_format: Some("yuv420p".into()),
        progressive: true,
        delivered_rate: None,
    };
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(&place.file, &place.media.media_uri)
        .map_err(|error| error.to_string())?;
    let decoder = Decoder::open_endpoint(request, endpoint, place.stamp.clone(), config).map_err(|error| error.to_string())?;
    if decoder.process_id().is_some() {
        return Ok(None);
    }
    Ok(Some(Open { decoder, next: first.saturating_sub(2) }))
}

/// Where a decoder starts for `first`: two pictures early, as the player does.
fn start_of(place: &LocatedPicture, first: u64) -> Option<Rational> {
    let start = first.saturating_sub(2);
    (start > 0).then(|| Rational {
        numerator: (start * u64::from(place.fps.1)) as i64,
        denominator: i64::from(place.fps.0.max(1)),
    })
}

fn read(
    open: &mut Open,
    picture: &StillPicture,
    first: u64,
    count: u32,
    place: &LocatedPicture,
    cancel: &AtomicBool,
) -> Result<Vec<(StillPicture, Arc<RgbaImage>)>, String> {
    if first < open.next || first - open.next > READ_ON {
        let jumped = match start_of(place, first) {
            Some(start) => open.decoder.seek(start).map_err(|error| error.to_string())?,
            None => open.decoder.seek(Rational { numerator: 0, denominator: 1 }).map_err(|error| error.to_string())?,
        };
        if !jumped {
            return Err(qnc_ui_text::text("story.msg.still_decoder_no_seek").into());
        }
        open.next = first.saturating_sub(2);
    }
    let last = first + u64::from(count.max(1)) - 1;
    let mut made = Vec::new();
    while !cancel.load(Ordering::Acquire) {
        let Some(packet) = open.decoder.next_packet().map_err(|error| error.to_string())? else {
            break; // the end of the media
        };
        let seconds = packet.pts as f64 * packet.time_base.numerator as f64 / packet.time_base.denominator.max(1) as f64;
        let Some(frame) = place.frame_at(seconds) else { continue };
        open.next = frame + 1;
        if frame < first {
            continue;
        }
        if frame <= last {
            let DecodedFormat::Video { width, height, .. } = packet.format else {
                return Err(qnc_ui_text::text("story.msg.still_decoder_no_picture").into());
            };
            let image = small_rgba(&packet.bytes, width as usize, height as usize)?;
            made.push((StillPicture { clip_id: picture.clip_id.clone(), source_frame: frame }, Arc::new(image)));
        }
        if frame >= last {
            break;
        }
    }
    if !made.iter().any(|(made, _)| made == picture) {
        return Err(qnc_ui_text::text("story.msg.still_failed").into());
    }
    Ok(made)
}

/// A packed `yuv420p` picture made `SIZE` RGBA (BT.709, video range), each pixel the
/// average of its block of luma.
fn small_rgba(bytes: &[u8], width: usize, height: usize) -> Result<RgbaImage, String> {
    let (chroma_w, chroma_h) = (width.div_ceil(2), height.div_ceil(2));
    if width == 0 || height == 0 || bytes.len() < width * height + 2 * chroma_w * chroma_h {
        return Err(qnc_ui_text::text("story.msg.still_decoder_bad_picture").into());
    }
    let (luma, chroma) = bytes.split_at(width * height);
    let (u_plane, v_plane) = chroma.split_at(chroma_w * chroma_h);
    let [out_w, out_h] = SIZE;
    let mut pixels = vec![0u8; out_w * out_h * 4];
    for oy in 0..out_h {
        let (y0, y1) = (oy * height / out_h, ((oy + 1) * height / out_h).max(oy * height / out_h + 1).min(height));
        for ox in 0..out_w {
            let (x0, x1) = (ox * width / out_w, ((ox + 1) * width / out_w).max(ox * width / out_w + 1).min(width));
            let mut sum = 0u32;
            for y in y0..y1 {
                sum += luma[y * width + x0..y * width + x1].iter().map(|&value| u32::from(value)).sum::<u32>();
            }
            let y_value = sum as f32 / ((y1 - y0) * (x1 - x0)) as f32;
            let (cx, cy) = (((x0 + x1) / 2) / 2, ((y0 + y1) / 2) / 2);
            let u = f32::from(u_plane[cy * chroma_w + cx]) - 128.0;
            let v = f32::from(v_plane[cy * chroma_w + cx]) - 128.0;
            let y_lin = 1.164 * (y_value - 16.0);
            let at = (oy * out_w + ox) * 4;
            pixels[at] = (y_lin + 1.793 * v).clamp(0.0, 255.0) as u8;
            pixels[at + 1] = (y_lin - 0.213 * u - 0.533 * v).clamp(0.0, 255.0) as u8;
            pixels[at + 2] = (y_lin + 2.112 * u).clamp(0.0, 255.0) as u8;
            pixels[at + 3] = 255;
        }
    }
    let content_key = bytes.iter().step_by(4099).fold(0xcbf2_9ce4_8422_2325u64, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    Ok(RgbaImage { size: SIZE, pixels, content_key })
}

#[cfg(test)]
mod tests {
    use super::small_rgba;

    #[test]
    fn a_grey_picture_stays_grey_and_small() {
        let (width, height) = (64, 36);
        let mut bytes = vec![126u8; width * height];
        bytes.extend(vec![128u8; 2 * (width / 2) * (height / 2)]);
        let image = small_rgba(&bytes, width, height).unwrap();
        assert_eq!(image.size, super::SIZE);
        let pixel = &image.pixels[..4];
        assert!(pixel[0] == pixel[1] && pixel[1] == pixel[2], "grey stays grey: {pixel:?}");
        assert!(small_rgba(&bytes[..10], width, height).is_err());
    }
}
