//! The AI pack (docs/102 step 8, user 2026-11-11): QNC installs without AI, the AI comes as a pack
//! of its own for a computer or an AI server. What goes in it is what the AI catalog of this
//! computer selects (its programs, the files they need beside them, the models they use); what
//! goes with them and under which licence is `catalogs/ai/pack.json`. A model whose licence its
//! authors do not state stays out of a pack unless asked for (`--include-unchecked`, for one's own
//! computers). The pack lists every file with its SHA-256; an installation checks every file
//! before it copies one.
//!
//! Usage:
//!   qnc-ai-pack list                          what is used, its licences, and the unused models
//!   qnc-ai-pack make OUT [--include-unchecked]  the pack into the empty folder OUT
//!   qnc-ai-pack install PACK APP_DIR QNC_ROOT  the programs beside the application in APP_DIR,
//!                                              the catalog and the models into QNC_ROOT

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// What goes with the programs and the models, beside the AI catalog.
const PACK_FILE: &str = "pack.json";
/// The list of a pack's files, at its top.
const MANIFEST: &str = "ai-pack.json";
/// Where a pack keeps the programs (installed beside the application).
const APP: &str = "app";
/// Where a pack keeps the models of the shared AI folder (installed back there).
const SHARE: &str = "share";
/// Where a pack keeps the licences of its models and libraries.
const LICENCES: &str = "LICENSES";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    #[serde(rename = "note")]
    _note: String,
    programs: Vec<ProgramEntry>,
    libraries: Vec<Library>,
    models: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramEntry {
    program: String,
    beside: Vec<String>,
    licence: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Library {
    file: String,
    licence: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelEntry {
    model: String,
    licence: String,
    source: String,
    attribution: String,
    status: String,
}

/// One job the AI catalog of this computer selects: its program and the models it uses.
struct Used {
    job: &'static str,
    program: String,
    models: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    jobs: Vec<String>,
    left_out: Vec<String>,
    files: Vec<PackFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackFile {
    path: String,
    bytes: u64,
    sha256: String,
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-ai-pack");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["list"] => list(),
        ["make", out] => make(Path::new(out), false),
        ["make", out, "--include-unchecked"] => make(Path::new(out), true),
        ["install", pack, app, root] => install(Path::new(pack), Path::new(app), Path::new(root)),
        _ => Err("usage: qnc-ai-pack list | make OUT [--include-unchecked] | install PACK APP_DIR QNC_ROOT".into()),
    };
    if let Err(error) = done {
        eprintln!("qnc-ai-pack: {error}");
        std::process::exit(1);
    }
}

/// The jobs the AI catalog of this computer selects, or why one is not there.
fn used() -> Vec<(&'static str, Result<Used, String>)> {
    use qnc_ai_catalog::Quality;
    let transcriber = |job, quality| {
        qnc_ai_catalog::installed_transcriber_for(quality).map(|engine| Used { job, program: engine.program, models: std::iter::once(engine.model).chain(engine.vad_model).collect() })
    };
    vec![
        ("transcribe.best", transcriber("transcribe.best", Quality::Best)),
        ("transcribe.fast", transcriber("transcribe.fast", Quality::Fast)),
        ("letters", qnc_ai_catalog::installed_letter_aligner().map(|engine| Used { job: "letters", program: engine.program, models: vec![engine.model] })),
        ("shot_content", qnc_ai_catalog::installed_shot_content().map(|engine| Used { job: "shot_content", program: engine.program, models: vec![engine.model] })),
        ("shot_caption", qnc_ai_catalog::installed_shot_captioner().map(|engine| Used { job: "shot_caption", program: engine.program, models: vec![engine.model] })),
        ("translate", qnc_ai_catalog::installed_translator().map(|engine| Used { job: "translate", program: engine.program, models: vec![engine.model] })),
        ("smooth", qnc_ai_catalog::installed_frame_interpolator().map(|engine| Used { job: "smooth", program: engine.program, models: vec![engine.model] })),
    ]
}

fn pack_list() -> Result<(PathBuf, Pack), String> {
    let catalog = qnc_ai_catalog::installed_path()?;
    let path = catalog.with_file_name(PACK_FILE);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((catalog, pack))
}

fn model_entry<'a>(pack: &'a Pack, model: &Path) -> Option<&'a ModelEntry> {
    let name = qnc_ai_catalog::model_name(model);
    pack.models.iter().find(|entry| entry.model == name)
}

/// Why the models of `used` cannot go in a pack: a licence not stated or not in the pack list.
fn unchecked(pack: &Pack, used: &Used) -> Option<String> {
    used.models.iter().find_map(|model| match model_entry(pack, model) {
        Some(entry) if entry.status == "known" => None,
        Some(entry) => Some(format!("{}: licence {} ({})", qnc_ai_catalog::model_name(model), entry.licence, entry.source)),
        None => Some(format!("{}: not in {PACK_FILE}", qnc_ai_catalog::model_name(model))),
    })
}

fn list() -> Result<(), String> {
    let (catalog, pack) = pack_list()?;
    println!("AI catalog {}", catalog.display());
    let mut used_models = BTreeSet::new();
    let mut folders = BTreeSet::new();
    for (job, used) in used() {
        match used {
            Ok(used) => {
                println!("{job}: {}", used.program);
                for model in used.models.iter().map(|model| plain(model)) {
                    let model = &model;
                    used_models.insert(model.clone());
                    if let Some(folder) = model.parent() {
                        folders.insert(folder.to_path_buf());
                    }
                    let licence = model_entry(&pack, model).map_or_else(|| format!("not in {PACK_FILE}"), |entry| format!("{} [{}] {}", entry.licence, entry.status, entry.attribution));
                    println!("    {} {} MB  {licence}", qnc_ai_catalog::model_name(model), size(model) >> 20);
                }
            }
            Err(error) => println!("{job}: not in this catalog ({error})"),
        }
    }
    println!("unused models (not removed; removing them is the user's choice):");
    for folder in folders {
        for entry in fs::read_dir(&folder).map_err(|e| format!("{}: {e}", folder.display()))?.flatten() {
            let path = entry.path();
            if !used_models.contains(&path) {
                println!("    {} {} MB", path.display(), size(&path) >> 20);
            }
        }
    }
    Ok(())
}

fn make(out: &Path, include_unchecked: bool) -> Result<(), String> {
    if out.exists() && fs::read_dir(out).map_err(|e| e.to_string())?.next().is_some() {
        return Err(format!("{} is not empty", out.display()));
    }
    let (catalog, pack) = pack_list()?;
    let app_dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("no application folder")?.to_path_buf();
    let catalog = plain(&catalog);
    let catalog_dir = catalog.parent().ok_or("no catalog folder")?;
    let (mut jobs, mut left_out) = (Vec::new(), Vec::new());
    let mut programs: BTreeSet<String> = BTreeSet::from(["qnc-ai-server".to_string(), "qnc-ai-store".to_string()]);
    let mut models = BTreeSet::new();
    for (job, used) in used() {
        let used = match used {
            Ok(used) => Used { models: used.models.iter().map(|model| plain(model)).collect(), ..used },
            Err(error) => {
                left_out.push(format!("{job}: {error}"));
                continue;
            }
        };
        if let Some(why) = unchecked(&pack, &used).filter(|_| !include_unchecked) {
            left_out.push(format!("{job}: {why}"));
            continue;
        }
        jobs.push(format!("{} ({})", used.job, used.program));
        programs.insert(used.program);
        models.extend(used.models);
    }
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    // The programs and what they need beside them.
    let mut libraries = BTreeSet::new();
    for program in &programs {
        let file = format!("{program}{}", std::env::consts::EXE_SUFFIX);
        copy(&app_dir.join(&file), &out.join(APP).join(&file))?;
        let entry = pack.programs.iter().find(|entry| &entry.program == program).ok_or_else(|| format!("{program} is not in {PACK_FILE}"))?;
        if entry.status != "known" {
            return Err(format!("{program}: licence {} is not stated", entry.licence));
        }
        libraries.extend(entry.beside.iter().cloned());
    }
    for library in &libraries {
        copy(&app_dir.join(library), &out.join(APP).join(library))?;
    }
    // The models, at the same place under the root as the catalog names them, and their licences.
    let root = catalog_dir.parent().and_then(Path::parent).ok_or("the catalog is not in a QNC root")?;
    let mut licences = String::from("The AI pack of QNC: the licences of its models and libraries.\n\n");
    let share = qnc_ai_catalog::share_dir(catalog_dir).ok().map(|share| plain(&share));
    for model in &models {
        // A model of the shared AI folder goes under `share/`, one under the root at its place there.
        let place = match share.as_ref().and_then(|share| model.strip_prefix(share).ok()) {
            Some(shared) => Path::new(SHARE).join(shared),
            None => model.strip_prefix(root).map_err(|_| format!("{} is neither under the QNC root nor in the shared AI folder", model.display()))?.to_path_buf(),
        };
        copy(model, &out.join(place))?;
        if let Some(entry) = model_entry(&pack, model) {
            licences.push_str(&format!("{}: {} ({}). {}\n", entry.model, entry.licence, entry.source, entry.attribution));
        }
        for own in ["README.md", "LICENSE", "LICENSE.txt", "LICENSE.md"] {
            let file = model.join(own);
            if file.is_file() {
                copy(&file, &out.join(LICENCES).join(qnc_ai_catalog::model_name(model)).join(own))?;
            }
        }
    }
    for library in &libraries {
        if let Some(entry) = pack.libraries.iter().find(|entry| &entry.file == library) {
            licences.push_str(&format!("{}: {}\n", entry.file, entry.licence));
        }
    }
    fs::create_dir_all(out.join(LICENCES)).map_err(|e| e.to_string())?;
    fs::write(out.join(LICENCES).join("AI-LICENCES.txt"), licences).map_err(|e| e.to_string())?;
    let relative_catalog = catalog_dir.strip_prefix(root).map_err(|e| e.to_string())?;
    copy(&catalog, &out.join(relative_catalog).join(catalog.file_name().ok_or("no catalog name")?))?;
    copy(&catalog.with_file_name(PACK_FILE), &out.join(relative_catalog).join(PACK_FILE))?;
    let files = listed(out, out)?;
    let manifest = Manifest { jobs, left_out, files };
    fs::write(out.join(MANIFEST), serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let bytes: u64 = manifest.files.iter().map(|file| file.bytes).sum();
    println!("AI pack {}: {} files, {} MB", out.display(), manifest.files.len(), bytes >> 20);
    manifest.jobs.iter().for_each(|job| println!("    in: {job}"));
    manifest.left_out.iter().for_each(|job| println!("    left out: {job}"));
    Ok(())
}

fn install(pack: &Path, app: &Path, root: &Path) -> Result<(), String> {
    let text = fs::read_to_string(pack.join(MANIFEST)).map_err(|e| format!("{}: {e}", pack.join(MANIFEST).display()))?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !qnc_dev_diagnostics::is_qnc_root(root) {
        return Err(format!("{} is not a QNC root", root.display()));
    }
    // A program finds the AI catalog above the folder it is in (`qnc_ai_catalog::installed_path`).
    if !plain(app).starts_with(plain(root)) {
        return Err(format!("{} is not inside the QNC root {}: its programs would not find the AI catalog", app.display(), root.display()));
    }
    // The models of the shared AI folder go where the root's setting names it; not set, nothing installs.
    let share = qnc_ai_catalog::chosen_share(root);
    if share.is_none() && manifest.files.iter().any(|file| Path::new(&file.path).starts_with(SHARE)) {
        return Err(format!("the shared AI folder of {} is not set (choose it in Settings first)", root.display()));
    }
    // Every file is checked before any is copied: a broken pack installs nothing.
    for file in &manifest.files {
        let path = pack.join(&file.path);
        let (bytes, sha256) = digest(&path)?;
        if bytes != file.bytes || sha256 != file.sha256 {
            return Err(format!("{} is not the file the pack lists", file.path));
        }
    }
    for file in &manifest.files {
        let relative = Path::new(&file.path);
        let to = match (relative.strip_prefix(APP), relative.strip_prefix(SHARE)) {
            (Ok(program), _) => app.join(program),
            // The models of the shared AI folder go back there (QNC and QNC-AI read them both).
            (_, Ok(shared)) => share.as_ref().ok_or("the shared AI folder is not set")?.join(shared),
            _ => root.join(relative),
        };
        copy(&pack.join(relative), &to)?;
    }
    println!("AI pack installed: {} files; programs in {}, catalog and models in {}", manifest.files.len(), app.display(), root.display());
    Ok(())
}

/// Copies a file, or a folder with everything in it.
fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_dir() {
        for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::copy(from, to).map(drop).map_err(|e| format!("{} -> {}: {e}", from.display(), to.display()))
}

/// Every file under `folder` with its size and SHA-256, its path relative to `top` with `/`.
fn listed(top: &Path, folder: &Path) -> Result<Vec<PackFile>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            files.extend(listed(top, &path)?);
        } else if path != top.join(MANIFEST) {
            let (bytes, sha256) = digest(&path)?;
            let relative = path.strip_prefix(top).map_err(|e| e.to_string())?.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            files.push(PackFile { path: relative, bytes, sha256 });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn digest(path: &Path) -> Result<(u64, String), String> {
    let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mut hash, mut bytes, mut buffer) = (Sha256::new(), 0u64, vec![0u8; 1 << 20]);
    loop {
        let read = file.read(&mut buffer).map_err(|e| format!("{}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        bytes += read as u64;
    }
    Ok((bytes, hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect()))
}

/// The bytes of a file or of everything in a folder.
fn size(path: &Path) -> u64 {
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => fs::read_dir(path).map(|entries| entries.flatten().map(|entry| size(&entry.path())).sum()).unwrap_or(0),
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

/// `path` without its `.` and `..` (the catalog names its models relative to its own folder).
fn plain(path: &Path) -> PathBuf {
    let mut plain = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                plain.pop();
            }
            other => plain.push(other.as_os_str()),
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalog_path_is_plain_and_a_listed_file_is_its_size_and_sha256() {
        assert_eq!(plain(Path::new("C:/qnc/catalogs/ai/../../data/models/x")), PathBuf::from("C:/qnc/data/models/x"));
        let top = std::env::temp_dir().join(format!("qnc-ai-pack-test-{}", std::process::id()));
        fs::create_dir_all(top.join("app")).unwrap();
        fs::write(top.join("app").join("a.exe"), b"abc").unwrap();
        fs::write(top.join(MANIFEST), b"not listed").unwrap();
        let files = listed(&top, &top).unwrap();
        let _ = fs::remove_dir_all(&top);
        assert_eq!(files.len(), 1, "the manifest does not list itself");
        assert_eq!((files[0].path.as_str(), files[0].bytes), ("app/a.exe", 3));
        assert_eq!(files[0].sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
