//! The Sony MediaProfile record format as a plug-in program (contract `qnc.record-format` 1).
//! The application starts it, gives it texts and references, and reads its answers on stdout.

fn main() {
    let served = qnc_record_format_plugin::serve(&qnc_sony_metadata::SonyMediaProfile, "qnc.format.sony-mediaprofile", env!("CARGO_PKG_VERSION"));
    if let Err(error) = served {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
