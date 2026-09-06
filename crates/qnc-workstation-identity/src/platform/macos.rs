use crate::{IdentitySource, ReadFailure, Reading, SerialCandidate, SerialKind};
use core_foundation::{
    base::{CFType, TCFType},
    string::CFString,
};
use io_kit_sys::{
    IOObjectRelease, IORegistryEntryCreateCFProperty, IOServiceGetMatchingService,
    IOServiceMatching,
};

fn platform_serial() -> Reading {
    // IOKit exposes the same platform serial on Intel and Apple Silicon.
    // MatchingService consumes the matching dictionary; CreateCFProperty returns ownership.
    unsafe {
        let matching = IOServiceMatching(c"IOPlatformExpertDevice".as_ptr());
        if matching.is_null() {
            return Err(ReadFailure::Unavailable);
        }
        let service = IOServiceGetMatchingService(0, matching);
        if service == 0 {
            return Err(ReadFailure::Unavailable);
        }
        let key = CFString::new("IOPlatformSerialNumber");
        let value = IORegistryEntryCreateCFProperty(
            service,
            key.as_concrete_TypeRef(),
            std::ptr::null(),
            0,
        );
        IOObjectRelease(service);
        if value.is_null() {
            return Err(ReadFailure::Unavailable);
        }
        let owned = CFType::wrap_under_create_rule(value);
        owned
            .downcast::<CFString>()
            .map(|s| s.to_string())
            .ok_or(ReadFailure::InvalidValue)
    }
}

pub(crate) fn serial_candidates() -> Vec<SerialCandidate> {
    vec![(
        SerialKind::Device,
        IdentitySource::IokitPlatformSerial,
        platform_serial(),
    )]
}
