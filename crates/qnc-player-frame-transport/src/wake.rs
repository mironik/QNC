//! Cross-process post signal. The player posts; the monitor waits.
//! This is not a clock and not a display refresh.

use std::{path::Path, time::Duration};

pub(crate) struct Wake {
    inner: Inner,
}

impl Wake {
    pub(crate) fn create(path: &Path) -> Result<Self, String> {
        Ok(Self {
            inner: Inner::create(path)?,
        })
    }

    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            inner: Inner::open(path)?,
        })
    }

    pub(crate) fn signal(&self) -> Result<(), String> {
        self.inner.signal()
    }

    pub(crate) fn wait(&self, timeout: Duration) -> Result<bool, String> {
        self.inner.wait(timeout)
    }
}

#[cfg(windows)]
mod sys {
    use super::*;
    use std::{
        collections::hash_map::DefaultHasher,
        ffi::OsString,
        hash::{Hash, Hasher},
        os::windows::ffi::OsStrExt,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{CreateEventW, ResetEvent, SetEvent, WaitForSingleObject},
    };

    pub(super) struct Inner {
        handle: HANDLE,
    }

    unsafe impl Send for Inner {}
    unsafe impl Sync for Inner {}

    impl Inner {
        pub(super) fn create(path: &Path) -> Result<Self, String> {
            open_named(path)
        }

        pub(super) fn open(path: &Path) -> Result<Self, String> {
            open_named(path)
        }

        pub(super) fn signal(&self) -> Result<(), String> {
            if unsafe { SetEvent(self.handle) } == 0 {
                return Err("monitor wake signal failed".into());
            }
            Ok(())
        }

        pub(super) fn wait(&self, timeout: Duration) -> Result<bool, String> {
            let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
            match unsafe { WaitForSingleObject(self.handle, ms) } {
                WAIT_OBJECT_0 => {
                    let _ = unsafe { ResetEvent(self.handle) };
                    Ok(true)
                }
                WAIT_TIMEOUT => Ok(false),
                _ => Err("monitor wake wait failed".into()),
            }
        }
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    CloseHandle(self.handle);
                }
            }
        }
    }

    fn open_named(path: &Path) -> Result<Inner, String> {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        let name = format!("Local\\qnc-frm-{:016x}", hasher.finish());
        let wide: Vec<u16> = OsString::from(name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        if handle.is_null() || handle == (-1isize as HANDLE) {
            return Err("monitor wake event unavailable".into());
        }
        Ok(Inner { handle })
    }
}

#[cfg(unix)]
mod sys {
    use super::*;
    use std::{
        fs::{File, OpenOptions},
        io::{Read, Write},
        os::fd::AsRawFd,
    };

    pub(super) struct Inner {
        file: File,
    }

    impl Inner {
        pub(super) fn create(path: &Path) -> Result<Self, String> {
            let wake_path = wake_path(path);
            match unsafe {
                libc::mkfifo(
                    std::ffi::CString::new(wake_path.to_string_lossy().as_bytes())
                        .map_err(|e| e.to_string())?
                        .as_ptr(),
                    0o600,
                )
            } {
                0 => {}
                _ if std::io::Error::last_os_error().kind()
                    == std::io::ErrorKind::AlreadyExists => {}
                _ => return Err(std::io::Error::last_os_error().to_string()),
            }
            open_fifo(&wake_path)
        }

        pub(super) fn open(path: &Path) -> Result<Self, String> {
            Self::create(path)
        }

        pub(super) fn signal(&self) -> Result<(), String> {
            let mut file = self.file.try_clone().map_err(|e| e.to_string())?;
            file.write_all(&[1]).map_err(|e| e.to_string())
        }

        pub(super) fn wait(&self, timeout: Duration) -> Result<bool, String> {
            let mut pollfd = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ms = libc::c_int::try_from(timeout.as_millis()).unwrap_or(libc::c_int::MAX);
            match unsafe { libc::poll(&mut pollfd, 1, ms) } {
                0 => Ok(false),
                n if n > 0 => {
                    let mut file = self.file.try_clone().map_err(|e| e.to_string())?;
                    let mut discard = [0u8; 32];
                    loop {
                        match file.read(&mut discard) {
                            Ok(0) => break,
                            Ok(_) => continue,
                            Err(error)
                                if error.kind() == std::io::ErrorKind::WouldBlock
                                    || error.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                break;
                            }
                            Err(error) => return Err(error.to_string()),
                        }
                    }
                    Ok(true)
                }
                _ => Err(std::io::Error::last_os_error().to_string()),
            }
        }
    }

    fn wake_path(path: &Path) -> std::path::PathBuf {
        path.with_extension("wake")
    }

    fn open_fifo(path: &Path) -> Result<Inner, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        Ok(Inner { file })
    }
}

#[cfg(not(any(windows, unix)))]
mod sys {
    use super::*;

    pub(super) struct Inner;

    impl Inner {
        pub(super) fn create(_: &Path) -> Result<Self, String> {
            Ok(Self)
        }
        pub(super) fn open(_: &Path) -> Result<Self, String> {
            Ok(Self)
        }
        pub(super) fn signal(&self) -> Result<(), String> {
            Ok(())
        }
        pub(super) fn wait(&self, timeout: Duration) -> Result<bool, String> {
            std::thread::sleep(timeout);
            Ok(false)
        }
    }
}

use sys::Inner;
