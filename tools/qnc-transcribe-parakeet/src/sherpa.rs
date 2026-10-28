//! The few functions of the sherpa-onnx C API (v2) a non-streaming transducer needs, loaded
//! from the library beside this program. The recognizer configuration is zeroed and only its
//! leading fields are set (the C API takes every zero field as its default); the result is
//! read through its leading fields only.

use libloading::{Library, Symbol};
use std::{
    ffi::{c_char, CStr, CString},
    path::Path,
};

/// The leading fields of `SherpaOnnxOfflineRecognizerConfig` (feature config, then the model
/// config up to `model_type`), followed by zeroes for everything after them.
#[repr(C)]
struct Config {
    sample_rate: i32,
    feature_dim: i32,
    transducer_encoder: *const c_char,
    transducer_decoder: *const c_char,
    transducer_joiner: *const c_char,
    paraformer_model: *const c_char,
    nemo_ctc_model: *const c_char,
    whisper_encoder: *const c_char,
    whisper_decoder: *const c_char,
    whisper_language: *const c_char,
    whisper_task: *const c_char,
    whisper_tail_paddings: i32,
    whisper_token_timestamps: i32,
    whisper_segment_timestamps: i32,
    tdnn_model: *const c_char,
    tokens: *const c_char,
    num_threads: i32,
    debug: i32,
    provider: *const c_char,
    model_type: *const c_char,
    /// The rest of the C struct, zero (larger than it).
    rest: [u8; 4096],
}

/// The leading fields of `SherpaOnnxOfflineRecognizerResult`.
#[repr(C)]
struct RawResult {
    text: *const c_char,
    timestamps: *const f32,
    count: i32,
    tokens: *const c_char,
    tokens_arr: *const *const c_char,
    json: *const c_char,
    lang: *const c_char,
    emotion: *const c_char,
    event: *const c_char,
    durations: *const f32,
    ys_log_probs: *const f32,
}

#[repr(C)]
struct Opaque {
    _private: [u8; 0],
}

/// One token of a result: its text and where it is in the sound (seconds).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub start: f32,
    pub duration: f32,
    pub log_prob: Option<f32>,
}

/// The sherpa-onnx library and one recognizer of a model.
pub struct Recognizer {
    library: Library,
    recognizer: *const Opaque,
}

// The recognizer is used from the one job thread of this program.
unsafe impl Send for Recognizer {}

const LIBRARY: &str = if cfg!(windows) {
    "sherpa-onnx-c-api.dll"
} else if cfg!(target_os = "macos") {
    "libsherpa-onnx-c-api.dylib"
} else {
    "libsherpa-onnx-c-api.so"
};

impl Recognizer {
    /// The recognizer of the transducer model in `folder` (encoder, decoder, joiner, tokens).
    pub fn open(folder: &Path, threads: u32) -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let path = exe.parent().ok_or("the program has no folder")?.join(LIBRARY);
        // SAFETY: the library is sherpa-onnx's C API, loaded from beside this program.
        let library = unsafe { Library::new(&path) }.map_err(|e| format!("{} is not beside the program: {e}", path.display()))?;
        let file = |prefix: &str, suffix: &str| -> Result<CString, String> {
            let entries = std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            let mut found: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(prefix) && n.ends_with(suffix)))
                .collect();
            found.sort();
            let path = found.into_iter().next().ok_or_else(|| format!("{}: no {prefix}*{suffix}", folder.display()))?;
            CString::new(path.to_string_lossy().into_owned()).map_err(|e| e.to_string())
        };
        let (encoder, decoder, joiner, tokens) = (file("encoder", ".onnx")?, file("decoder", ".onnx")?, file("joiner", ".onnx")?, file("tokens", ".txt")?);
        let (provider, model_type) = (CString::new("cpu").unwrap(), CString::new("nemo_transducer").unwrap());
        // SAFETY: every field the C API reads is set or zero (its default).
        let mut config: Config = unsafe { std::mem::zeroed() };
        config.transducer_encoder = encoder.as_ptr();
        config.transducer_decoder = decoder.as_ptr();
        config.transducer_joiner = joiner.as_ptr();
        config.tokens = tokens.as_ptr();
        config.num_threads = threads.max(1) as i32;
        config.provider = provider.as_ptr();
        config.model_type = model_type.as_ptr();
        let recognizer = unsafe {
            let create: Symbol<unsafe extern "C" fn(*const Config) -> *const Opaque> = symbol(&library, b"SherpaOnnxCreateOfflineRecognizer\0")?;
            create(&config)
        };
        if recognizer.is_null() {
            return Err(format!("sherpa-onnx could not load the model in {}", folder.display()));
        }
        Ok(Self { library, recognizer })
    }

    /// The tokens of `samples` (mono, -1..1, `rate` Hz; sherpa-onnx resamples) and the language.
    pub fn decode(&self, samples: &[f32], rate: u32) -> Result<(Vec<Token>, String), String> {
        let n = i32::try_from(samples.len()).map_err(|_| "too much sound for one decode")?;
        unsafe {
            let create: Symbol<unsafe extern "C" fn(*const Opaque) -> *const Opaque> = symbol(&self.library, b"SherpaOnnxCreateOfflineStream\0")?;
            let accept: Symbol<unsafe extern "C" fn(*const Opaque, i32, *const f32, i32)> = symbol(&self.library, b"SherpaOnnxAcceptWaveformOffline\0")?;
            let decode: Symbol<unsafe extern "C" fn(*const Opaque, *const Opaque)> = symbol(&self.library, b"SherpaOnnxDecodeOfflineStream\0")?;
            let result: Symbol<unsafe extern "C" fn(*const Opaque) -> *const RawResult> = symbol(&self.library, b"SherpaOnnxGetOfflineStreamResult\0")?;
            let free_result: Symbol<unsafe extern "C" fn(*const RawResult)> = symbol(&self.library, b"SherpaOnnxDestroyOfflineRecognizerResult\0")?;
            let free_stream: Symbol<unsafe extern "C" fn(*const Opaque)> = symbol(&self.library, b"SherpaOnnxDestroyOfflineStream\0")?;
            let stream = create(self.recognizer);
            if stream.is_null() {
                return Err("sherpa-onnx gave no stream".into());
            }
            accept(stream, rate as i32, samples.as_ptr(), n);
            decode(self.recognizer, stream);
            let raw = result(stream);
            let read = if raw.is_null() { Err("sherpa-onnx gave no result".to_string()) } else { Ok(read_result(&*raw)) };
            if !raw.is_null() {
                free_result(raw);
            }
            free_stream(stream);
            read
        }
    }
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        unsafe {
            if let Ok(destroy) = symbol::<unsafe extern "C" fn(*const Opaque)>(&self.library, b"SherpaOnnxDestroyOfflineRecognizer\0") {
                destroy(self.recognizer);
            }
        }
    }
}

unsafe fn symbol<'l, T>(library: &'l Library, name: &[u8]) -> Result<Symbol<'l, T>, String> {
    library.get(name).map_err(|e| format!("sherpa-onnx has no {}: {e}", String::from_utf8_lossy(&name[..name.len() - 1])))
}

unsafe fn read_result(raw: &RawResult) -> (Vec<Token>, String) {
    let text = |p: *const c_char| if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() };
    let count = raw.count.max(0) as usize;
    let mut tokens = Vec::with_capacity(count);
    if !raw.tokens_arr.is_null() && !raw.timestamps.is_null() {
        for i in 0..count {
            tokens.push(Token {
                text: text(*raw.tokens_arr.add(i)),
                start: *raw.timestamps.add(i),
                duration: if raw.durations.is_null() { 0.0 } else { *raw.durations.add(i) },
                log_prob: if raw.ys_log_probs.is_null() { None } else { Some(*raw.ys_log_probs.add(i)) },
            });
        }
    }
    (tokens, text(raw.lang))
}
