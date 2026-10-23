//! The camera adapter of one record format, driven by camera profiles (user 2026-10-23: "a
//! block for every camera, so that new cameras are added without touching the code"). The
//! format block reads the card's records; the profiles say which camera wrote them and what
//! the procedure is. Each clip's record gets the profile it was read with
//! ([`qnc_camera_profiles::RECORD_TAG`], `<id>@<version>`), so a project knows how every clip
//! was read, also on a computer without that profile. A card whose camera no profile knows is
//! read with the format's default profile, and its record says so.

use qnc_camera_adapter::{CameraAdapter, MetadataSufficiency, RecordFormat};
use qnc_camera_profiles::{MetadataRule, Profile, RECORD_TAG};
use qnc_media_metadata::{ClipMetadata, Fact};
use qnc_source_contract::SourceReference;
use qnc_source_groups::{GroupProposal, IndexDocument, IndexReader};
use std::sync::Arc;

pub const MODULE_ID: &str = "qnc.module.camera-profile-adapter";

pub struct ProfileAdapter {
    id: String,
    format: Arc<dyn RecordFormat>,
    /// The profiles of this format.
    profiles: Vec<Profile>,
}

impl ProfileAdapter {
    /// The adapter of `format` with the `profiles` of its reader. Refused without a default
    /// profile: a card of this format must always be readable.
    pub fn new(format: Arc<dyn RecordFormat>, profiles: &[Profile]) -> Result<Self, String> {
        let reader = format.index().reader_id().to_string();
        let profiles: Vec<Profile> = profiles.iter().filter(|p| p.reader == reader).cloned().collect();
        if !profiles.iter().any(|p| p.default_for_reader) {
            return Err(format!("no default camera profile for the record format '{reader}'"));
        }
        Ok(Self { id: format!("camera.profiles.{reader}"), format, profiles })
    }

    /// The profile a record was read with.
    fn profile_of(&self, metadata: &ClipMetadata) -> Option<&Profile> {
        let stamp = &metadata.original.tags.get(RECORD_TAG)?.value;
        self.profiles.iter().find(|p| &p.stamp() == stamp)
    }
}

impl CameraAdapter for ProfileAdapter {
    fn adapter_id(&self) -> &str {
        &self.id
    }

    /// The card layouts the profiles name.
    fn pattern_ids(&self) -> Vec<&str> {
        let mut layouts: Vec<&str> = self.profiles.iter().flat_map(|p| p.layouts.iter().map(String::as_str)).collect();
        layouts.sort_unstable();
        layouts.dedup();
        layouts
    }

    fn index(&self) -> &dyn IndexReader {
        self.format.index()
    }

    fn documents(&self, group: &GroupProposal) -> Vec<SourceReference> {
        self.format.documents(group)
    }

    fn thumbnail(&self, group: &GroupProposal) -> Option<SourceReference> {
        self.format.thumbnail(group)
    }

    fn metadata(&self, clip_id: &str, group: &GroupProposal, documents: &[IndexDocument]) -> Result<ClipMetadata, String> {
        let mut metadata = self.format.metadata(clip_id, group, documents)?;
        let records: Vec<&str> = documents.iter().map(|d| d.text.as_str()).collect();
        let (profile, recognised) = qnc_camera_profiles::recognise(&self.profiles, self.index().reader_id(), &records)
            .ok_or_else(|| format!("no camera profile for the record format '{}'", self.index().reader_id()))?;
        // The evidence the camera was told by: the sidecar when the format read one.
        let evidence = metadata.evidence.iter().find(|e| e.id.starts_with("sidecar")).or(metadata.evidence.first()).map(|e| e.id.clone());
        if let Some(evidence_id) = evidence {
            let locator = if recognised { "camera-profile:recognised" } else { "camera-profile:default" };
            metadata.original.tags.insert(RECORD_TAG.into(), Fact { value: profile.stamp(), evidence_id, locator: locator.into() });
        }
        Ok(metadata)
    }

    /// The profile's procedure, per record: a record that does not state what a probe would is
    /// probed once (for example a missing sidecar).
    fn sufficiency(&self, metadata: &ClipMetadata) -> MetadataSufficiency {
        let rule = self.profile_of(metadata).map_or(MetadataRule::DeclaredWhenComplete, |p| p.procedure.metadata);
        match rule {
            MetadataRule::DeclaredWhenComplete if qnc_camera_adapter::has_probe_facts(metadata) => MetadataSufficiency::Declared,
            _ => MetadataSufficiency::NeedsProbe,
        }
    }
}

#[cfg(test)]
mod tests;
