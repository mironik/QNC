use std::{
    io::{self, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !args.is_empty() && (args.len() != 1 || args[0] != "--json") {
        eprintln!("Usage: qnc-workstation-identity [--json]");
        return ExitCode::from(2);
    }
    let snapshot = qnc_workstation_identity::read_local_identity();
    let mut output = io::stdout().lock();
    if serde_json::to_writer(&mut output, &snapshot).is_err() || writeln!(output).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
