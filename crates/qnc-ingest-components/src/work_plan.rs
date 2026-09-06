use qnc_work_settings::WorkSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestMedia {
    Link,
    Proxy,
    Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackInput {
    Proxy,
    Original,
    ProxyIfAvailable,
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
        let playback_input = match settings.playback["input"].as_str() {
            Some("proxy") => PlaybackInput::Proxy,
            Some("original") => PlaybackInput::Original,
            Some("proxy_if_available") => PlaybackInput::ProxyIfAvailable,
            _ => return Err("Baza sadrzi nepodrzani playback.input.".into()),
        };
        // v4 directory roles, expressed as transport resources, not OS path joins.
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
