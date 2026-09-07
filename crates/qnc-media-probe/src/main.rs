//! One request per helper invocation; private binding config never crosses stdout.
use qnc_media_probe::*;
use std::io::Read;
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err(
            "usage: qnc-media-probe <private-owner-config.json>; request JSON on stdin".into(),
        );
    }
    let mut config = Vec::new();
    std::fs::File::open(&args[0])?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut config)?;
    if config.len() > MAX_BYTES {
        return Err(Error::Configuration.into());
    }
    let executor = Executor::new(serde_json::from_slice(&config)?)?;
    let mut input = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut input)?;
    if input.len() > 65536 {
        return Err(Error::InvalidRequest.into());
    }
    let request: Request = serde_json::from_slice(&input)?;
    let result = executor.execute(&request);
    serde_json::to_writer(
        std::io::stdout(),
        &Reply {
            version: VERSION.into(),
            request_id: request.request_id,
            result,
        },
    )?;
    Ok(())
}
