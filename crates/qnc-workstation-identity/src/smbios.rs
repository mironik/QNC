use crate::{IdentitySource, ReadFailure, SerialCandidate, SerialKind};
use smbioslib::{SMBiosData, SMBiosProcessorInformation, SMBiosSystemInformation};

pub(crate) fn candidates(data: &SMBiosData) -> Vec<SerialCandidate> {
    let mut output = Vec::new();
    for record in data.defined_struct_iter::<SMBiosSystemInformation>() {
        output.push((
            SerialKind::Device,
            IdentitySource::SmbiosSystemSerial,
            record.serial_number().ok().ok_or(ReadFailure::Unavailable),
        ));
    }
    for record in data.defined_struct_iter::<SMBiosProcessorInformation>() {
        // Type 4 Serial Number, never ProcessorId/CPUID or the CPU model.
        output.push((
            SerialKind::Processor,
            IdentitySource::SmbiosProcessorSerial,
            record.serial_number().ok().ok_or(ReadFailure::Unavailable),
        ));
    }
    if output.is_empty() {
        output.push((
            SerialKind::Device,
            IdentitySource::SmbiosSystemSerial,
            Err(ReadFailure::Unavailable),
        ));
    }
    output
}
