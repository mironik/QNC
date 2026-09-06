use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;

use qnc_application_catalog::*;
const MANIFEST_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Debug, Deserialize)]
struct Registration {
    application_id: String,
    tab_id: String,
    label: String,
    enabled: bool,
    system: bool,
    removable: bool,
    priority_group: String,
    host_mode: HostMode,
    desktop_entry: String,
    standalone_executable: String,
}

impl From<Registration> for Application {
    fn from(r: Registration) -> Self {
        Self {
            application_id: r.application_id,
            tab_id: r.tab_id,
            label: r.label,
            priority_group: r.priority_group,
            system: r.system,
            removable: r.removable,
            host_mode: r.host_mode,
            desktop_entry: r.desktop_entry,
            standalone_executable: r.standalone_executable,
        }
    }
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(format!("Not a regular file: {}", path.display()).into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(format!("File exceeds {limit} byte limit: {}", path.display()).into());
    }
    Ok(bytes)
}

fn executable_path(directory: &Path, basename: &str) -> PathBuf {
    directory.join(format!("{basename}{}", std::env::consts::EXE_SUFFIX))
}

fn executable_status(path: &Path) -> Result<Option<UnavailableReason>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(UnavailableReason::MissingExecutable));
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() == 0 {
        return Ok(Some(UnavailableReason::NotExecutable));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Ok(Some(UnavailableReason::NotExecutable));
        }
    }
    Ok(None)
}

fn discover(registrations: &Path, executables: &Path, uri: &str) -> Result<Catalog> {
    validate_uri(uri)?;
    if !registrations.is_dir() || !executables.is_dir() {
        return Err("Registration and executable directories must both exist".into());
    }
    let mut catalog = Catalog {
        catalog_id: CATALOG_ID.into(),
        schema_version: SCHEMA_VERSION,
        catalog_uri: uri.into(),
        selection_policy: "one_per_priority_group".into(),
        observed_at_unix_ms: u64::try_from(
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
        )?,
        target_os: std::env::consts::OS.into(),
        target_cpu: std::env::consts::ARCH.into(),
        applications: Vec::new(),
        unavailable: Vec::new(),
    };
    let mut ids = HashSet::new();
    let mut tabs = HashSet::new();
    for child in fs::read_dir(registrations)? {
        let path = child?.path();
        if !fs::metadata(&path)?.is_dir() {
            continue;
        }
        let manifest_path = path.join("qnc-app.json");
        match fs::metadata(&manifest_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let registration: Registration =
            serde_json::from_slice(&read_limited(&manifest_path, MANIFEST_LIMIT)?)
                .map_err(|error| format!("Invalid {}: {error}", manifest_path.display()))?;
        let enabled = registration.enabled;
        let app = Application::from(registration);
        validate_application(&app)?;
        if !ids.insert(app.application_id.clone()) || !tabs.insert(app.tab_id.clone()) {
            return Err(format!(
                "Duplicate registered application ID/tab: {}",
                manifest_path.display()
            )
            .into());
        }
        let reason = if enabled {
            executable_status(&executable_path(executables, &app.standalone_executable))?
        } else {
            Some(UnavailableReason::Disabled)
        };
        if let Some(reason) = reason {
            catalog.unavailable.push(UnavailableApplication {
                application_id: app.application_id,
                label: app.label,
                priority_group: app.priority_group,
                reason,
            });
        } else {
            catalog.applications.push(app);
        }
    }
    catalog.applications.sort_by(|a, b| {
        a.priority_group
            .cmp(&b.priority_group)
            .then_with(|| a.label.cmp(&b.label))
            .then_with(|| a.application_id.cmp(&b.application_id))
    });
    catalog.unavailable.sort_by(|a, b| {
        a.priority_group
            .cmp(&b.priority_group)
            .then_with(|| a.application_id.cmp(&b.application_id))
    });
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn publish(catalog: &Catalog, output: &Path) -> Result<()> {
    validate_catalog(catalog)?;
    let replace = match fs::symlink_metadata(output) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err("Refusing to replace a catalog symlink or non-file".into());
            }
            let previous = read_catalog(output)?;
            if previous.catalog_uri != catalog.catalog_uri {
                return Err("Refusing to replace a catalog for a different QNC URI".into());
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    let mut bytes = serde_json::to_vec_pretty(catalog)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > CATALOG_LIMIT {
        return Err("Generated catalog exceeds size limit".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    // Readers see the old complete snapshot or the new one, never a partially written JSON.
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    read_catalog(temporary.path())?;
    if replace {
        temporary.persist(output).map_err(|error| error.error)?;
    } else {
        temporary
            .persist_noclobber(output)
            .map_err(|error| error.error)?;
    }
    Ok(())
}

fn refresh(registrations: &Path, executables: &Path, output: &Path, uri: &str) -> Result<Catalog> {
    let catalog = discover(registrations, executables, uri)?;
    publish(&catalog, output)?;
    Ok(catalog)
}

fn summary(catalog: &Catalog) {
    println!(
        "{}: {} available, {} unavailable",
        catalog.catalog_uri,
        catalog.applications.len(),
        catalog.unavailable.len()
    );
    for app in &catalog.applications {
        println!(
            "{}\t{}\t{}",
            app.priority_group, app.application_id, app.label
        );
    }
    for app in &catalog.unavailable {
        eprintln!("unavailable: {} ({:?})", app.application_id, app.reason);
    }
}

const USAGE: &str = "Usage: qnc-app-catalog refresh REGISTRATIONS_DIR EXECUTABLES_DIR OUTPUT.json [QNC_URI]\n       qnc-app-catalog check CATALOG.json\n       qnc-app-catalog show CATALOG.json\n       qnc-app-catalog select CATALOG.json [APPLICATION_ID ...]";

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|value| value.to_str()) {
        Some("refresh") if args.len() == 4 || args.len() == 5 => {
            let uri = match args.get(4) {
                Some(value) => value.to_str().ok_or("QNC URI must be valid Unicode")?,
                None => DEFAULT_URI,
            };
            summary(&refresh(
                Path::new(&args[1]),
                Path::new(&args[2]),
                Path::new(&args[3]),
                uri,
            )?);
        }
        Some("check") if args.len() == 2 => summary(&read_catalog(Path::new(&args[1]))?),
        Some("show") if args.len() == 2 => println!(
            "{}",
            serde_json::to_string_pretty(&read_catalog(Path::new(&args[1]))?)?
        ),
        Some("select") if args.len() >= 2 => {
            let catalog = read_catalog(Path::new(&args[1]))?;
            let ids = args[2..]
                .iter()
                .map(|value| value.to_str().ok_or("Application ID must be Unicode"))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&select(&catalog, &ids)?)?
            );
        }
        Some("--help" | "-h") if args.len() == 1 => println!("{USAGE}"),
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("app-catalog: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
