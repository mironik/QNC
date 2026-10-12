//! The media of a request opened through the reader of this process
//! (`qnc-media-readahead`) instead of by the FFmpeg library itself: every decoder of the
//! player reads its file from memory and one thread reads the card, in long sequential
//! runs, ahead of every open decoder (user 2026-10-12: breaking news from the camera card
//! in a USB 2.0 reader; the bursts of reads at each cut made the story stall).

use std::{
    ffi::{c_int, c_void, CString},
    io::{Read, Seek, SeekFrom},
    path::Path,
    ptr,
};

use ffmpeg_next as ff;
use ff::ffi;
use qnc_media_decode::{DecodeError, ErrorKind, Result};
use qnc_media_readahead::{MediaReader, Stream};

/// The buffer of the FFmpeg IO context; the reader keeps the real read ahead.
const IO_BUFFER: usize = 1 << 16;
/// What the stream analysis after opening may read: the saved record already holds the
/// stream's description, and the analysis read 14-25 MB from the start of the file at
/// every cover (1.2 s on a cold card, 2026-10-12) before the decoder went to its start.
const ANALYZE_BYTES: i64 = 1 << 20;
const ANALYZE_MICROS: i64 = 100_000;

/// The IO context and the stream it reads; freed after the format context that uses it.
pub(crate) struct ReaderIo {
    avio: *mut ffi::AVIOContext,
    stream: *mut Stream,
}

// SAFETY: moved once with its decoder to the decoder thread and used only there.
unsafe impl Send for ReaderIo {}

impl Drop for ReaderIo {
    fn drop(&mut self) {
        // SAFETY: allocated in `open_input`; the format context that used them is gone.
        unsafe {
            if !self.avio.is_null() {
                ffi::av_freep(ptr::addr_of_mut!((*self.avio).buffer).cast());
                ffi::avio_context_free(&mut self.avio);
            }
            if !self.stream.is_null() {
                drop(Box::from_raw(self.stream));
            }
        }
    }
}

fn error(message: impl std::fmt::Display) -> DecodeError {
    DecodeError::new(ErrorKind::Stream, format!("in-process decoder: {message}"))
}

/// Opens the media as `ffmpeg_next::format::input` does (open, then the stream info),
/// its bytes read through the reader of this process; the container is the saved one
/// (`demuxer`, as the CLI adapter gives it with `-f`), and the analysis is short.
pub(crate) fn open_input(path: &Path, demuxer: &str) -> Result<(ff::format::context::Input, ReaderIo)> {
    let stream = MediaReader::shared().open(path).map_err(error)?;
    let url = CString::new(path.to_string_lossy().as_bytes()).map_err(error)?;
    let format_name = CString::new(demuxer).map_err(error)?;
    // SAFETY: FFmpeg allocation and open calls; every pointer is checked, and on an error
    // what was allocated is freed (`ReaderIo` drop; `avformat_open_input` frees the
    // context itself when it fails).
    unsafe {
        let mut io = ReaderIo {
            avio: ptr::null_mut(),
            stream: Box::into_raw(Box::new(stream)),
        };
        let buffer = ffi::av_malloc(IO_BUFFER).cast::<u8>();
        if buffer.is_null() {
            return Err(error("no memory for the IO buffer"));
        }
        io.avio = ffi::avio_alloc_context(
            buffer,
            IO_BUFFER as c_int,
            0,
            io.stream.cast(),
            Some(read_packet),
            None,
            Some(seek),
        );
        if io.avio.is_null() {
            ffi::av_free(buffer.cast());
            return Err(error("no IO context"));
        }
        let mut context = ffi::avformat_alloc_context();
        if context.is_null() {
            return Err(error("no format context"));
        }
        (*context).pb = io.avio;
        (*context).flags |= ffi::AVFMT_FLAG_CUSTOM_IO;
        (*context).probesize = ANALYZE_BYTES;
        (*context).max_analyze_duration = ANALYZE_MICROS;
        let format = ffi::av_find_input_format(format_name.as_ptr());
        if format.is_null() {
            ffi::avformat_free_context(context);
            return Err(error("saved container has no demuxer"));
        }
        let opened = ffi::avformat_open_input(&mut context, url.as_ptr(), format, ptr::null_mut());
        if opened < 0 {
            return Err(error(ff::Error::from(opened)));
        }
        let found = ffi::avformat_find_stream_info(context, ptr::null_mut());
        if found < 0 {
            ffi::avformat_close_input(&mut context);
            return Err(error(ff::Error::from(found)));
        }
        Ok((ff::format::context::Input::wrap(context), io))
    }
}

unsafe extern "C" fn read_packet(opaque: *mut c_void, buf: *mut u8, size: c_int) -> c_int {
    // SAFETY: `opaque` is the stream of this IO context; `buf` holds `size` bytes.
    let (stream, out) = unsafe {
        (
            &mut *opaque.cast::<Stream>(),
            std::slice::from_raw_parts_mut(buf, usize::try_from(size).unwrap_or(0)),
        )
    };
    match stream.read(out) {
        Ok(0) => ffi::AVERROR_EOF,
        Ok(n) => c_int::try_from(n).unwrap_or(c_int::MAX),
        Err(_) => ffi::AVERROR(ffi::EIO),
    }
}

unsafe extern "C" fn seek(opaque: *mut c_void, offset: i64, whence: c_int) -> i64 {
    // SAFETY: `opaque` is the stream of this IO context.
    let stream = unsafe { &mut *opaque.cast::<Stream>() };
    if whence & ffi::AVSEEK_SIZE != 0 {
        return i64::try_from(stream.len()).unwrap_or(-1);
    }
    let to = match whence & !ffi::AVSEEK_FORCE {
        0 => match u64::try_from(offset) {
            Ok(at) => SeekFrom::Start(at),
            Err(_) => return -1,
        },
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return -1,
    };
    stream
        .seek(to)
        .ok()
        .and_then(|at| i64::try_from(at).ok())
        .unwrap_or(-1)
}
