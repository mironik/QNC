//! Pure full-report reader. Execution and persistence belong to other modules.
use qnc_media_metadata::*;
use qnc_media_records::{validate_resource_uri, MAX_DOCUMENT_BYTES};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidDocument,
    InvalidBinding,
    InvalidField(String),
    TooLarge,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ffprobe metadata: {self:?}")
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub evidence: Evidence,
    pub media: MediaRepresentation,
}

pub fn read(text: &str, media_uri: &str, document_uri: &str, evidence_id: &str) -> Result<Parsed> {
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    validate_resource_uri(media_uri).map_err(|_| Error::InvalidBinding)?;
    validate_resource_uri(document_uri).map_err(|_| Error::InvalidBinding)?;
    if evidence_id.is_empty() || evidence_id.len() > 128 {
        return Err(Error::InvalidBinding);
    }
    let root: Value = serde_json::from_str(text).map_err(|_| Error::InvalidDocument)?;
    if root.get("error").is_some() {
        return Err(Error::InvalidDocument);
    }
    let format = root
        .get("format")
        .filter(|f| f.is_object())
        .ok_or(Error::InvalidDocument)?;
    if format.get("filename").and_then(Value::as_str) != Some(media_uri) {
        return Err(Error::InvalidBinding);
    }
    let streams = root
        .get("streams")
        .and_then(Value::as_array)
        .ok_or(Error::InvalidDocument)?;
    if streams.is_empty() || streams.len() > 128 {
        return Err(Error::InvalidDocument);
    }
    let reader = Reader { id: evidence_id };
    let mut media = MediaRepresentation {
        media_uri: media_uri.into(),
        container: reader.string(format, "format_name", "/format")?,
        duration_seconds: reader.ratio(format, "duration", "/format")?,
        streams_complete: Some(reader.fact("/streams", true)),
        streams: Vec::new(),
        tags: BTreeMap::new(),
    };
    reader.tags(format, "/format", &mut media.tags)?;
    let mut indices = BTreeSet::new();
    for (i, stream) in streams.iter().enumerate() {
        let path = format!("/streams/{i}");
        let index = reader
            .integer::<u32>(stream, "index", &path)?
            .ok_or_else(|| Error::InvalidField(path.clone()))?;
        if !indices.insert(index.value) {
            return Err(Error::InvalidField(format!("{path}/index")));
        }
        let kind = reader
            .string(stream, "codec_type", &path)?
            .ok_or_else(|| Error::InvalidField(path.clone()))?;
        let details = match kind.value.as_str() {
            "video" => StreamDetails::Video(Box::new(reader.video(stream, &path)?)),
            "audio" => StreamDetails::Audio(Box::new(AudioMetadata {
                sample_rate_hz: reader.integer(stream, "sample_rate", &path)?,
                channels: reader.integer(stream, "channels", &path)?,
                sample_format: reader.signal(stream, "sample_fmt", &path)?,
                channel_layout: reader.signal(stream, "channel_layout", &path)?,
                bits_per_sample: reader
                    .integer::<u16>(stream, "bits_per_raw_sample", &path)?
                    .filter(|v| v.value > 0)
                    .or(reader
                        .integer::<u16>(stream, "bits_per_sample", &path)?
                        .filter(|v| v.value > 0)),
            })),
            _ => StreamDetails::Other { stream_type: kind },
        };
        media.streams.push(MediaStream {
            index: Some(index),
            codec: reader.signal(stream, "codec_name", &path)?,
            profile: reader.string(stream, "profile", &path)?,
            time_base: reader.ratio(stream, "time_base", &path)?,
            start_pts: reader.integer(stream, "start_pts", &path)?,
            duration_ts: reader.integer(stream, "duration_ts", &path)?,
            details,
        });
        reader.tags(stream, &path, &mut media.tags)?;
        for key in ["codec_tag_string", "codec_tag"] {
            if let Some(value) = reader.raw(stream, key, &path)? {
                let pointer = format!("{path}/{key}");
                media
                    .tags
                    .insert(format!("ffprobe:{pointer}"), reader.fact(&pointer, value));
            }
        }
        if media.tags.len() > 512 {
            return Err(Error::TooLarge);
        }
    }
    let evidence = Evidence {
        id: evidence_id.into(),
        kind: EvidenceKind::Ffprobe,
        document_uri: document_uri.into(),
        media_uri: media_uri.into(),
    };
    let clip = ClipMetadata {
        contract_id: CONTRACT_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        clip_id: "validation".into(),
        evidence: vec![evidence.clone()],
        original: media.clone(),
        proxy: None,
    };
    if let Some(issue) = inspect(&clip)
        .issues
        .into_iter()
        .find(|i| i.code == IssueCode::Invalid)
    {
        return Err(Error::InvalidField(issue.path));
    }
    Ok(Parsed { evidence, media })
}

struct Reader<'a> {
    id: &'a str,
}
impl Reader<'_> {
    fn fact<T>(&self, path: &str, value: T) -> Fact<T> {
        Fact {
            value,
            evidence_id: self.id.into(),
            locator: path.into(),
        }
    }
    fn raw(&self, object: &Value, key: &str, path: &str) -> Result<Option<String>> {
        match object.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(v)) => Ok(Some(v.clone())),
            Some(Value::Number(v)) => Ok(Some(v.to_string())),
            _ => Err(Error::InvalidField(format!("{path}/{key}"))),
        }
    }
    fn string(&self, object: &Value, key: &str, path: &str) -> Result<Option<Fact<String>>> {
        Ok(self
            .raw(object, key, path)?
            .filter(|s| !missing(s))
            .map(|v| self.fact(&format!("{path}/{key}"), v)))
    }
    fn signal(
        &self,
        object: &Value,
        key: &str,
        path: &str,
    ) -> Result<Option<Fact<Signal<String>>>> {
        Ok(self.raw(object, key, path)?.map(|v| {
            self.fact(
                &format!("{path}/{key}"),
                if missing(&v) {
                    Signal::Unspecified
                } else {
                    Signal::Known(v)
                },
            )
        }))
    }
    fn integer<T: std::str::FromStr>(
        &self,
        object: &Value,
        key: &str,
        path: &str,
    ) -> Result<Option<Fact<T>>> {
        self.string(object, key, path)?
            .map(|f| {
                let value = f
                    .value
                    .parse()
                    .map_err(|_| Error::InvalidField(f.locator.clone()))?;
                Ok(self.fact(&f.locator, value))
            })
            .transpose()
    }
    fn ratio(&self, object: &Value, key: &str, path: &str) -> Result<Option<Fact<Rational>>> {
        self.string(object, key, path)?
            .map(|f| {
                let value =
                    rational(&f.value).ok_or_else(|| Error::InvalidField(f.locator.clone()))?;
                Ok(self.fact(&f.locator, value))
            })
            .transpose()
            .map(|r| r.filter(|f| f.value.numerator > 0 && f.value.denominator > 0))
    }
    fn video(&self, stream: &Value, path: &str) -> Result<VideoMetadata> {
        let rate = self.ratio(stream, "avg_frame_rate", path)?.or(self.ratio(
            stream,
            "r_frame_rate",
            path,
        )?);
        let frame_rate = rate.as_ref().map(|r| {
            self.fact(
                &r.locator,
                FrameTimebase {
                    fps_num: r.value.numerator,
                    fps_den: r.value.denominator,
                },
            )
        });
        let mut frame_count = self
            .integer::<u64>(stream, "nb_frames", path)?
            .filter(|f| f.value > 0)
            .map(|f| self.fact(&f.locator, FrameCount::Exact(f.value)));
        if frame_count.is_none() {
            if let (Some(duration), Some(rate)) =
                (self.ratio(stream, "duration", path)?, rate.as_ref())
            {
                let num = i128::from(duration.value.numerator) * i128::from(rate.value.numerator);
                let den =
                    i128::from(duration.value.denominator) * i128::from(rate.value.denominator);
                let count = (num + den / 2) / den;
                if let Ok(n @ 1..=i64::MAX) = i64::try_from(count) {
                    frame_count = Some(self.fact(
                        &format!("{path}/duration*{}", rate.locator),
                        FrameCount::Estimated(n as u64),
                    ));
                }
            }
        }
        let scan_mode = self.string(stream, "field_order", path)?.and_then(|f| {
            let mode = match f.value.as_str() {
                "progressive" => ScanMode::Progressive,
                "tt" | "bt" => ScanMode::InterlacedTopFieldFirst,
                "bb" | "tb" => ScanMode::InterlacedBottomFieldFirst,
                _ => return None,
            };
            Some(self.fact(&f.locator, mode))
        });
        let mut rotation = None;
        let mut declared = false;
        if let Some(sides) = stream.get("side_data_list") {
            let sides = sides
                .as_array()
                .ok_or_else(|| Error::InvalidField(format!("{path}/side_data_list")))?;
            for (i, side) in sides.iter().enumerate() {
                if !side.is_object() {
                    return Err(Error::InvalidField(format!("{path}/side_data_list/{i}")));
                }
                declared |= side.get("rotation").is_some()
                    || side.get("displaymatrix").is_some()
                    || side.get("side_data_type").and_then(Value::as_str) == Some("Display Matrix");
                if let Some(value) =
                    self.integer::<i32>(side, "rotation", &format!("{path}/side_data_list/{i}"))?
                {
                    if rotation
                        .as_ref()
                        .is_some_and(|old: &Fact<i32>| old.value != value.value)
                    {
                        return Err(Error::InvalidField(format!("{path}/rotation")));
                    }
                    rotation = Some(value);
                }
            }
        }
        // An unhandled legacy declaration is not evidence of absence. Keep it in raw tags.
        declared |= stream
            .get("tags")
            .is_some_and(|tags| tags.get("rotate").is_some());
        let rotation = match rotation {
            Some(value) => Some(self.fact(&value.locator, Signal::Known(value.value))),
            None if !declared => Some(self.fact(path, Signal::Unspecified)),
            None => None,
        };
        Ok(VideoMetadata {
            width: self.integer(stream, "width", path)?,
            height: self.integer(stream, "height", path)?,
            frame_rate,
            frame_rate_mode: Some(self.fact(path, FrameRateMode::Unknown)),
            frame_count,
            scan_mode,
            pixel_format: self.string(stream, "pix_fmt", path)?,
            sample_aspect_ratio: self.ratio(stream, "sample_aspect_ratio", path)?,
            rotation_degrees: rotation,
            color: ColorMetadata {
                primaries: self.signal(stream, "color_primaries", path)?,
                transfer: self.signal(stream, "color_transfer", path)?,
                matrix: self.signal(stream, "color_space", path)?,
                range: self.signal(stream, "color_range", path)?,
            },
        })
    }
    fn tags(
        &self,
        object: &Value,
        path: &str,
        out: &mut BTreeMap<String, Fact<String>>,
    ) -> Result<()> {
        if let Some(tags) = object.get("tags") {
            let tags = tags.as_object().ok_or(Error::InvalidDocument)?;
            for (key, value) in tags {
                let pointer = format!("{path}/tags/{}", key.replace('~', "~0").replace('/', "~1"));
                let value = value
                    .as_str()
                    .ok_or_else(|| Error::InvalidField(pointer.clone()))?;
                out.insert(
                    format!("ffprobe:{pointer}"),
                    self.fact(&pointer, value.to_string()),
                );
                if path == "/format" && matches!(key.as_str(), "creation_time" | "timecode") {
                    out.insert(key.clone(), self.fact(&pointer, value.to_string()));
                }
            }
        }
        if out.len() > 512 {
            return Err(Error::TooLarge);
        }
        Ok(())
    }
}
fn missing(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "" | "n/a" | "unknown" | "unspecified" | "none"
    )
}
fn rational(s: &str) -> Option<Rational> {
    let (n, d) = if let Some((n, d)) = s.split_once('/').or_else(|| s.split_once(':')) {
        (n.parse::<i64>().ok()?, d.parse::<i64>().ok()?)
    } else if let Some((n, fraction)) = s.split_once('.') {
        if fraction.len() > 9
            || fraction.is_empty()
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let d = 10i64.checked_pow(fraction.len() as u32)?;
        let whole = n.parse::<i64>().ok()?;
        let f = fraction.parse::<i64>().ok()?;
        (
            whole
                .checked_mul(d)?
                .checked_add(if s.starts_with('-') { -f } else { f })?,
            d,
        )
    } else {
        (s.parse::<i64>().ok()?, 1)
    };
    if n == 0 || d == 0 {
        return Some(Rational {
            numerator: n,
            denominator: d,
        });
    }
    if n < 0 || d < 0 {
        return None;
    }
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    Some(Rational {
        numerator: n / a,
        denominator: d / a,
    })
}

#[cfg(test)]
mod tests;
