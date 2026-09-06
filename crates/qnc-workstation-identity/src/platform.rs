#[cfg(windows)]
#[path = "platform/windows.rs"]
mod os;
#[cfg(target_os = "linux")]
#[path = "platform/linux.rs"]
mod os;
#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod os;

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub(crate) use os::serial_candidates;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) fn serial_candidates() -> Vec<crate::SerialCandidate> {
    Vec::new()
}
