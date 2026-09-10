//! Test-only protocol peer. Emits synthetic packets, never a production decoder.
use qnc_media_decode::*;
use std::io::{self, Read, Write};

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin()
        .take(MAX_OPEN_BYTES as u64 + 1)
        .read_to_end(&mut input)?;
    let open: ProcessOpen = serde_json::from_slice(&input)?;
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "hang" {
        std::thread::sleep(std::time::Duration::from_secs(30));
        return Ok(());
    }
    let mut err = io::stderr().lock();
    let mut out = io::stdout().lock();
    writeln!(
        err,
        "{}",
        serde_json::to_string(&ProcessRecord::Ready {
            protocol_version: if mode == "bad-version" {
                "2".into()
            } else {
                PROCESS_PROTOCOL_VERSION.into()
            },
            adapter_id: open.adapter_id,
            request_id: open.request_id,
            format: open.expected_format,
        })?
    )?;
    err.flush()?;
    let count = if mode == "wrong-count" { 4 } else { 5 };
    for ordinal in 0..count {
        writeln!(
            err,
            "{}",
            serde_json::to_string(&ProcessRecord::Packet {
                header: PacketHeader {
                    ordinal,
                    pts: ordinal as i64 * 40,
                    time_base: qnc_media_metadata::Rational {
                        numerator: 1,
                        denominator: 1000
                    },
                    size: 384,
                }
            })?
        )?;
        err.flush()?;
        out.write_all(&vec![ordinal as u8; 384])?;
        out.flush()?;
    }
    if mode == "exit-failure" {
        std::process::exit(2);
    }
    Ok(())
}
