use super::*;

fn sample(candidates: Vec<SerialCandidate>) -> IdentitySnapshot {
    collect(Ok("Edit-01".into()), Ok("editor".into()), candidates)
}

#[test]
fn device_is_preferred_to_processor() {
    let result = sample(vec![
        (
            SerialKind::Processor,
            IdentitySource::SmbiosProcessorSerial,
            Ok("CPU-02".into()),
        ),
        (
            SerialKind::Device,
            IdentitySource::SmbiosSystemSerial,
            Ok("Unit-AbC-01".into()),
        ),
    ]);
    assert_eq!(result.hardware_serial.unwrap().value, "Unit-AbC-01");
}

#[test]
fn missing_device_can_use_real_processor_serial() {
    let result = sample(vec![
        (
            SerialKind::Device,
            IdentitySource::SmbiosSystemSerial,
            Err(ReadFailure::Unavailable),
        ),
        (
            SerialKind::Processor,
            IdentitySource::SmbiosProcessorSerial,
            Ok("CPU-02".into()),
        ),
    ]);
    assert_eq!(result.hardware_serial.unwrap().kind, SerialKind::Processor);
    assert_eq!(result.issues.len(), 1);
}

#[test]
fn oem_placeholders_are_not_serials() {
    for value in [
        "",
        "\0",
        "To Be Filled By O.E.M.",
        "Default string",
        "System Serial Number",
        "None",
        "Unknown",
        "00000000",
        "FFFFFFFF",
        "N/A",
        "abc\n123",
    ] {
        assert!(normalize_serial(value.into()).is_err(), "{value:?}");
    }
    assert_eq!(normalize_serial("  AbC-0139\0".into()).unwrap(), "AbC-0139");
}

#[test]
fn missing_metadata_is_explicit_and_never_synthesized() {
    let result = collect(
        Err(ReadFailure::ReadFailed),
        Err(ReadFailure::PermissionDenied),
        vec![(
            SerialKind::Device,
            IdentitySource::LinuxDmiSystemSerial,
            Err(ReadFailure::PermissionDenied),
        )],
    );
    assert!(result.workstation_name.is_none());
    assert!(result.user_name.is_none());
    assert!(result.hardware_serial.is_none());
    assert_eq!(result.issues.len(), 3);
    assert!(result
        .issues
        .iter()
        .any(|v| v.reason == ReadFailure::PermissionDenied));
}

#[test]
fn json_round_trip_is_portable_without_paths_or_project_state() {
    let result = sample(vec![(
        SerialKind::Device,
        IdentitySource::IokitPlatformSerial,
        Ok("Mac-123".into()),
    )]);
    let json = serde_json::to_string(&result).unwrap();
    assert_eq!(
        serde_json::from_str::<IdentitySnapshot>(&json).unwrap(),
        result
    );
    for private in [
        "local_path",
        "project_id",
        "active_project",
        "workspace",
        "password",
    ] {
        assert!(!json.contains(private));
    }
}

#[test]
fn module_contract_has_no_consumer_restrictions() {
    let report = qnc_contracts::validate_module_manifest_json("workstation-identity", MANIFEST);
    assert!(report.is_ok(), "{:?}", report.errors);
    let cargo = include_str!("../Cargo.toml");
    for dependency in [
        "qnc-project",
        "qnc-ingest",
        "qnc-shell",
        "rusqlite",
        "eframe",
        "reqwest",
    ] {
        assert!(!cargo.contains(dependency));
    }
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
fn smbios_reads_serial_field_not_processor_id() {
    let mut system = vec![1, 8, 0, 0, 0, 0, 0, 1];
    system.extend_from_slice(b"To Be Filled By O.E.M.\0\0");
    let mut cpu = vec![0u8; 33];
    cpu[0] = 4;
    cpu[1] = 33;
    cpu[8..16].copy_from_slice(b"FAKECPID");
    cpu[32] = 1;
    cpu.extend_from_slice(b"RealCpuSerial\0\0");
    system.extend(cpu);
    let data = smbioslib::SMBiosData::from_vec_and_version(system, None);
    let result = sample(crate::smbios::candidates(&data));
    let serial = result.hardware_serial.unwrap();
    assert_eq!(serial.kind, SerialKind::Processor);
    assert_eq!(serial.value, "RealCpuSerial");
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
fn smbios_short_record_does_not_invent_a_serial() {
    let data = smbioslib::SMBiosData::from_vec_and_version(vec![4, 4, 0, 0, 0, 0], None);
    assert!(sample(crate::smbios::candidates(&data))
        .hardware_serial
        .is_none());
}
