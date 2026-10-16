//! The disks attached to this computer now, with their volume serial number (user
//! 2026-10-16: a USB disk or card the system sees is a source without being added by
//! hand). Read only: it lists, it never mounts, writes or scans.
//!
//! Windows: every drive letter with a file system and its volume serial. Other systems
//! do not expose a volume serial the same way; there the list is empty for now and the
//! host configuration names the sources.

use std::path::PathBuf;

pub const MODULE_ID: &str = "qnc.module.source-volumes";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// An attached disk: its root, its volume serial (8 hex digits) and its label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub root: PathBuf,
    pub serial_number: String,
    pub volume_name: Option<String>,
}

impl Volume {
    /// The QNC source URI of the disk: one per volume serial, whatever letter it got.
    pub fn source_uri(&self) -> String {
        format!("qnc://local/source/volume-{}", self.serial_number)
    }
}

/// The disks attached now.
pub fn mounted() -> Vec<Volume> {
    #[cfg(windows)]
    {
        (b'A'..=b'Z')
            .filter_map(|letter| {
                let root = PathBuf::from(format!("{}:\\", letter as char));
                if !root.is_dir() {
                    return None;
                }
                let (serial_number, volume_name) = identity(&root)?;
                Some(Volume { root, serial_number, volume_name })
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn identity(root: &std::path::Path) -> Option<(String, Option<String>)> {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
    let wide: Vec<u16> = root.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut name = vec![0u16; 260];
    let mut serial = 0u32;
    // SAFETY: the buffers live through the call and their lengths are given.
    let ok = unsafe {
        GetVolumeInformationW(wide.as_ptr(), name.as_mut_ptr(), name.len() as u32, &mut serial, null_mut(), null_mut(), null_mut(), 0)
    };
    if ok == 0 {
        return None;
    }
    let end = name.iter().position(|value| *value == 0).unwrap_or(name.len());
    let label = String::from_utf16_lossy(&name[..end]);
    Some((format!("{serial:08x}"), (!label.trim().is_empty()).then_some(label)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disk_is_a_source_by_its_serial() {
        let volume = Volume { root: PathBuf::from("E:\\"), serial_number: "7e80f1fb".into(), volume_name: None };
        assert_eq!(volume.source_uri(), "qnc://local/source/volume-7e80f1fb");
    }

    #[cfg(windows)]
    #[test]
    fn the_system_disk_is_listed_with_its_serial() {
        let volumes = mounted();
        assert!(volumes.iter().any(|volume| volume.root.to_string_lossy().starts_with("C:") && volume.serial_number.len() == 8));
    }
}
