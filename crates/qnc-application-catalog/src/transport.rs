use crate::*;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use std::{fs::File, io::Read, path::Path, time::Duration};

fn decode(mut reader: impl Read) -> Result<Catalog> {
    let mut bytes = Vec::new();
    (&mut reader)
        .take(CATALOG_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CATALOG_LIMIT {
        return Err("Catalog exceeds size limit".into());
    }
    let catalog = serde_json::from_slice(&bytes)?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

pub fn read_catalog(path: &Path) -> Result<Catalog> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err("Catalog is not a regular file".into());
    }
    decode(file)
}

/// Read-only transport adapter. Call outside the UI thread.
pub fn read_uri(resolver: &ResolverConfig, uri: &str) -> Result<Catalog> {
    validate_uri(uri)?;
    let catalog = match resolver.resolve(uri)?.endpoint {
        ResolvedEndpoint::LocalPath(path) => read_catalog(&path)?,
        ResolvedEndpoint::NetworkEndpoint {
            base_url,
            resource_kind,
            resource_id,
        } => {
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(10))
                .redirects(0)
                .build();
            let response = agent
                .get(&format!("{base_url}/{resource_kind}/{resource_id}"))
                .set("Accept", "application/json")
                .call()?;
            if response.status() != 200 {
                return Err("Catalog endpoint did not return 200".into());
            }
            decode(response.into_reader())?
        }
    };
    if catalog.catalog_uri != uri {
        return Err("Catalog URI does not match requested resource".into());
    }
    Ok(catalog)
}
