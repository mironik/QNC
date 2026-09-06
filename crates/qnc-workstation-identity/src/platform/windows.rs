use crate::{io_failure, IdentitySource, SerialCandidate, SerialKind};

pub(crate) fn serial_candidates() -> Vec<SerialCandidate> {
    match smbioslib::table_load_from_device() {
        Ok(data) => crate::smbios::candidates(&data),
        Err(error) => vec![(
            SerialKind::Device,
            IdentitySource::SmbiosSystemSerial,
            Err(io_failure(error)),
        )],
    }
}
