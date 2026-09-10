use crate::*;
use std::{fs::File, time::UNIX_EPOCH};

pub(crate) struct LocalMedia {
    file: File,
    pub info: MediaInfo,
}
impl LocalMedia {
    pub fn open(source: &LocalSource, uri: &str) -> io::Result<Self> {
        let file = source
            .open_read_only_file(&reference(uri)?)
            .map_err(source_error)?;
        let (byte_len, storage_stamp) = stamp(&file)?;
        Ok(Self {
            file,
            info: MediaInfo {
                media_uri: uri.into(),
                byte_len,
                storage_stamp,
            },
        })
    }
    fn check(&self) -> io::Result<()> {
        if stamp(&self.file)?.1 != self.info.storage_stamp {
            return Err(changed());
        }
        Ok(())
    }
}
fn stamp(file: &File) -> io::Result<(u64, String)> {
    let meta = file.metadata()?;
    let modified = meta.modified()?;
    let time = match modified.duration_since(UNIX_EPOCH) {
        Ok(t) => format!("{:x}", t.as_nanos()),
        Err(e) => format!("-{:x}", e.duration().as_nanos()),
    };
    if meta.len() > i64::MAX as u64 {
        return Err(invalid());
    }
    Ok((meta.len(), format!("{:x}-{time}", meta.len())))
}
impl Read for LocalMedia {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.check()?;
        let max = buffer.len().min(MAX_READ_BYTES);
        let n = self.file.read(&mut buffer[..max])?;
        self.check()?;
        Ok(n)
    }
}
impl Seek for LocalMedia {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.check()?;
        let value = match position {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.file.stream_position()?) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.info.byte_len) + i128::from(n),
        };
        if !(0..=i128::from(i64::MAX)).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid byte seek",
            ));
        }
        self.file.seek(SeekFrom::Start(value as u64))
    }
}
