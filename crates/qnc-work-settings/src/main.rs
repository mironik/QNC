use qnc_work_settings::SettingsReader;
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut root = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut bind = None;
    let mut registry_uri = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("Missing --root value")?),
            "--serve" => bind = Some(args.next().ok_or("Missing --serve address")?),
            "--registry-uri" => registry_uri = Some(args.next().ok_or("Missing registry URI")?),
            _ => {
                return Err(
                    "Usage: qnc-work-settings [--root DIR] [--serve ADDRESS --registry-uri URI]"
                        .into(),
                )
            }
        }
    }
    if let Some(bind) = bind {
        let address: std::net::SocketAddr = bind
            .parse()
            .map_err(|_| "--serve requires an IP address and port")?;
        if !address.ip().is_loopback() {
            return Err("Bind the storage helper to loopback and use an authenticated HTTPS reverse proxy for LAN/Intranet".into());
        }
        let token = std::env::var("QNC_WORK_SETTINGS_TOKEN")
            .ok()
            .filter(|t| !t.is_empty())
            .ok_or("QNC_WORK_SETTINGS_TOKEN is required")?;
        let uri = registry_uri.ok_or("--registry-uri is required for the storage endpoint")?;
        let server = tiny_http::Server::http(address).map_err(|e| e.to_string())?;
        eprintln!(
            "Read-only work-settings endpoint listening on {}",
            server.server_addr()
        );
        for request in server.incoming_requests() {
            qnc_work_settings::server::respond(
                request,
                &root.join("data").join("project_store.db"),
                &uri,
                &token,
            );
        }
    } else {
        if registry_uri.is_some() {
            return Err(
                "--registry-uri requires --serve; reader transport comes from configuration".into(),
            );
        }
        let result = SettingsReader::from_root(&root)
            .and_then(|r| r.read())
            .map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
