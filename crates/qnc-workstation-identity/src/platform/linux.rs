use crate::{
    io_failure, normalize_serial, IdentitySource, ReadFailure, Reading, SerialCandidate, SerialKind,
};
use std::{fs::File, io::Read};

fn read_serial(path: &str) -> Reading {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(io_failure)?
        .take(1025)
        .read_to_end(&mut bytes)
        .map_err(io_failure)?;
    if bytes.len() > 1024 {
        return Err(ReadFailure::InvalidValue);
    }
    normalize_serial(String::from_utf8(bytes).map_err(|_| ReadFailure::InvalidValue)?)
}

pub(crate) fn serial_candidates() -> Vec<SerialCandidate> {
    let mut output = Vec::new();
    for (path, source) in [
        (
            "/sys/class/dmi/id/product_serial",
            IdentitySource::LinuxDmiSystemSerial,
        ),
        (
            "/sys/firmware/devicetree/base/serial-number",
            IdentitySource::DeviceTreeSerial,
        ),
    ] {
        let reading = read_serial(path);
        let available = reading.is_ok();
        output.push((SerialKind::Device, source, reading));
        if available {
            return output;
        }
    }
    // Firmware access may be denied. Never invoke sudo or replace a serial with machine-id.
    match smbioslib::table_load_from_device() {
        Ok(data) => output.extend(crate::smbios::candidates(&data)),
        Err(error) => output.push((
            SerialKind::Processor,
            IdentitySource::SmbiosProcessorSerial,
            Err(io_failure(error)),
        )),
    }
    output
}
