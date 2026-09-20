use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LocationEntry {
    pub name: String,
    pub qnc_uri: String,
    pub serial_number: String,
    pub volume_name: String,
}
