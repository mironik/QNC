//! Prints what is installed and what works with what: the plug-ins (who they are and whether the
//! application takes them), the decoders of the decoder catalog and the programs beside the
//! application. `--json` prints the report as JSON. It changes nothing.

fn main() {
    let Some(root) = qnc_dev_diagnostics::locate_qnc_root() else {
        eprintln!("QNC root not found (QNC_ROOT, or a folder of the application)");
        std::process::exit(2);
    };
    let report = qnc_installed_versions::report(&root, env!("CARGO_PKG_VERSION"));
    if std::env::args().any(|arg| arg == "--json") {
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
        return;
    }
    let mut problems = 0;
    println!("QNC {}", report.application_version);
    println!("\nPlug-ins");
    for p in &report.plugins {
        let who = match (&p.plugin_id, &p.plugin_version) {
            (Some(id), Some(version)) => format!("{id} {version}"),
            _ => String::new(),
        };
        let state = match &p.problem {
            None => "OK".to_string(),
            Some(problem) => {
                problems += 1;
                format!("NOT TAKEN: {problem}")
            }
        };
        println!("  {:<44} {:<24} {} {}  {who}  {state}", p.role, p.program, p.contract, p.contract_version);
    }
    println!("\nDecoders");
    for d in &report.decoders {
        let state = match &d.problem {
            None => "OK".to_string(),
            Some(problem) => {
                problems += 1;
                format!("NOT AVAILABLE: {problem}")
            }
        };
        println!("  {:<22} {:<8} {:<18} {:<10} {state}", d.id, d.version, d.protocol, d.use_);
        if let Some(version) = &d.executable_version {
            println!("      {version}");
        }
        if let Some(jobs) = &d.jobs {
            println!("      jobs: {jobs}");
        }
    }
    println!("\nPrograms beside the application");
    for program in &report.programs {
        println!("  {:<32} {:>12} bytes  {}", program.name, program.bytes, qnc_installed_versions::utc(program.modified));
    }
    if problems > 0 {
        println!("\n{problems} problem(s)");
        std::process::exit(1);
    }
}
