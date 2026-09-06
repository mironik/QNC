use qnc_workstation_identity::{IdentitySnapshot, CONTRACT_VERSION};
use std::process::Command;

#[test]
fn helper_runs_outside_qnc_and_returns_one_json_snapshot() {
    let output = Command::new(env!("CARGO_BIN_EXE_qnc-workstation-identity"))
        .current_dir(std::env::temp_dir())
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let result: IdentitySnapshot = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result.contract_version, CONTRACT_VERSION);
}

#[test]
fn helper_rejects_remote_target_and_extra_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_qnc-workstation-identity"))
        .args(["--host", "server"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
