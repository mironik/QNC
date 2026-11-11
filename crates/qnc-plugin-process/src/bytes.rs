//! The content a request carries inside its JSON line (plug-in contracts version 2, docs/102
//! step 5): a picture or a sound itself, not the path of a file of the application, so the
//! program may run on another computer (an AI server on the LAN or the intranet).

use base64::Engine as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Bytes written into a JSON line as base64.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Bytes(pub Vec<u8>);

impl std::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bytes({} bytes)", self.0.len())
    }
}

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD.decode(text).map(Bytes).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bytes_go_through_a_json_line_as_they_are() {
        let bytes = super::Bytes((0..=255).collect());
        let line = serde_json::to_string(&bytes).unwrap();
        assert!(line.starts_with('"'));
        assert_eq!(serde_json::from_str::<super::Bytes>(&line).unwrap(), bytes);
        assert!(serde_json::from_str::<super::Bytes>("\"not base64!\"").is_err());
    }
}
