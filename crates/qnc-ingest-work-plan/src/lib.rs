pub use qnc_work_settings::PlaybackInput;
use qnc_work_settings::WorkSettings;

pub const MODULE_ID: &str = "qnc.module.ingest-work-plan";
pub const VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestMedia {
    Link,
    Proxy,
    Original,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IngestWorkPlan {
    pub settings: WorkSettings,
    pub media: IngestMedia,
    pub playback_input: PlaybackInput,
    pub original_uri: String,
    pub proxy_uri: String,
    pub audio_uri: String,
    pub incoming_uri: String,
    pub thumbnails_uri: String,
    pub filmstrip_uri: String,
}

impl IngestWorkPlan {
    pub fn from_settings(settings: WorkSettings) -> Result<Self, String> {
        settings.validate().map_err(|e| e.to_string())?;
        let media = match settings.storage.ingest_media.as_str() {
            "link" => IngestMedia::Link,
            "proxy" => IngestMedia::Proxy,
            "original" => IngestMedia::Original,
            _ => return Err("Baza sadrzi nepodrzani storage.ingest_media.".into()),
        };
        let playback_input = settings.playback_input().map_err(|e| e.to_string())?;
        let root = &settings.output_root_uri;
        Ok(Self {
            media,
            playback_input,
            original_uri: format!("{root}/original"),
            proxy_uri: format!("{root}/proxy"),
            audio_uri: format!("{root}/audio"),
            incoming_uri: format!("{root}/incoming"),
            thumbnails_uri: format!("{root}/ingest/thumbnails"),
            filmstrip_uri: format!("{root}/filmstrip"),
            settings,
        })
    }
}
