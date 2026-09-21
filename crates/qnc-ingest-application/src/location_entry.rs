use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LocationEntry {
    pub name: String,
    pub qnc_uri: String,
    pub serial_number: String,
    pub volume_name: String,
}

impl qnc_source_browse::BrowseEntry for LocationEntry {
    fn browse_uri(&self) -> &str {
        &self.qnc_uri
    }

    fn browse_name(&self) -> &str {
        &self.name
    }

    fn browse_serial_number(&self) -> &str {
        &self.serial_number
    }

    fn browse_volume_name(&self) -> &str {
        &self.volume_name
    }
}
