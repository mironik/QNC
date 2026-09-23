//! Public neutral input source adapters.
//!
//! This crate is the boundary between "where media can enter from" and the
//! project content database. It reads host/source transport bindings, exposes
//! local directory/card/LAN/intranet inputs in one shape, and can build the
//! read-only browser session used before Select writes anything to the project.
//! It does not scan, probe, write a database, or know any form/application.

use std::path::Path;

use qnc_dir_browser::{BrowserEntry, BrowserSource, TransportBrowserSession};
use qnc_source_bindings::{RegisteredSource, SourceBinding, TransportBindings};
use qnc_source_reader::SourceReader;

pub const MODULE_ID: &str = "qnc.module.source-input";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceInputKind {
    Computer,
    Lan,
    Intranet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInput {
    pub kind: SourceInputKind,
    pub uri: String,
    pub name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub scope: Option<String>,
    pub location: SourceBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceInputCatalog {
    inputs: Vec<SourceInput>,
}

pub trait SourceInputAdapter {
    fn kind(&self) -> SourceInputKind;

    fn inputs<'a>(&self, catalog: &'a SourceInputCatalog) -> Vec<&'a SourceInput> {
        catalog.by_kind(self.kind()).collect()
    }

    fn browser(&self, catalog: &SourceInputCatalog) -> Result<TransportBrowserSession, String> {
        browser_for(self.inputs(catalog))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ComputerInputAdapter;

#[derive(Debug, Clone, Copy, Default)]
pub struct LanInputAdapter;

#[derive(Debug, Clone, Copy, Default)]
pub struct IntranetInputAdapter;

impl SourceInputAdapter for ComputerInputAdapter {
    fn kind(&self) -> SourceInputKind {
        SourceInputKind::Computer
    }
}

impl SourceInputAdapter for LanInputAdapter {
    fn kind(&self) -> SourceInputKind {
        SourceInputKind::Lan
    }
}

impl SourceInputAdapter for IntranetInputAdapter {
    fn kind(&self) -> SourceInputKind {
        SourceInputKind::Intranet
    }
}

impl SourceInputCatalog {
    pub fn load(root: &Path) -> Result<Self, String> {
        Self::from_transport_bindings(qnc_source_bindings::load(root)?)
    }

    pub fn from_transport_bindings(bindings: TransportBindings) -> Result<Self, String> {
        let inputs = bindings
            .registered_sources
            .into_iter()
            .map(source_input)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { inputs })
    }

    pub fn inputs(&self) -> &[SourceInput] {
        &self.inputs
    }

    pub fn by_kind(&self, kind: SourceInputKind) -> impl Iterator<Item = &SourceInput> {
        self.inputs.iter().filter(move |input| input.kind == kind)
    }

    pub fn adapter(&self, kind: SourceInputKind) -> Box<dyn SourceInputAdapter> {
        match kind {
            SourceInputKind::Computer => Box::new(ComputerInputAdapter),
            SourceInputKind::Lan => Box::new(LanInputAdapter),
            SourceInputKind::Intranet => Box::new(IntranetInputAdapter),
        }
    }

    pub fn browser_for(&self, kind: SourceInputKind) -> Result<TransportBrowserSession, String> {
        self.adapter(kind).browser(self)
    }

    pub fn browser(&self) -> Result<TransportBrowserSession, String> {
        browser_for(self.inputs.iter().collect())
    }
}

fn browser_for(inputs: Vec<&SourceInput>) -> Result<TransportBrowserSession, String> {
    let sources = inputs
        .iter()
        .map(|input| {
            let source = (*input).clone();
            let mut browser_source = BrowserSource::new(
                BrowserEntry {
                    name: input.name.clone(),
                    qnc_uri: input.uri.clone(),
                    serial_number: input.serial_number.clone(),
                    volume_name: input.volume_name.clone(),
                },
                move || reader(&source.location),
            );
            if let Some(path) = &input.location.file {
                browser_source = browser_source.with_private_local_root(path.clone());
            }
            browser_source
        })
        .collect();
    TransportBrowserSession::new(sources).map_err(|error| error.to_string())
}

fn source_input(source: RegisteredSource) -> Result<SourceInput, String> {
    let kind = kind(&source.location, source.scope.as_deref())?;
    Ok(SourceInput {
        kind,
        uri: source.location.uri.clone(),
        name: required(source.name, "source name missing")?,
        serial_number: source.serial_number.unwrap_or_default(),
        volume_name: source.volume_name.unwrap_or_default(),
        scope: source.scope,
        location: source.location,
    })
}

fn kind(binding: &SourceBinding, _scope: Option<&str>) -> Result<SourceInputKind, String> {
    let parsed = qnc_contracts::parse_qnc_uri(&binding.uri)?;
    Ok(match parsed.environment.as_str() {
        "lan" => SourceInputKind::Lan,
        "intranet" => SourceInputKind::Intranet,
        "local" => SourceInputKind::Computer,
        _ => return Err("unsupported source input environment".into()),
    })
}

fn reader(binding: &SourceBinding) -> Result<SourceReader, String> {
    binding.resolver()?;
    if let Some(path) = &binding.file {
        return SourceReader::local(&binding.uri, path).map_err(|error| error.to_string());
    }
    SourceReader::remote(
        &binding.uri,
        binding
            .endpoint
            .as_deref()
            .ok_or("source endpoint missing")?,
        binding
            .token()?
            .as_deref()
            .ok_or("source credential missing")?,
    )
    .map_err(|error| error.to_string())
}

fn required(value: Option<String>, message: &'static str) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(
        uri: &str,
        file: Option<&str>,
        endpoint: Option<&str>,
        scope: Option<&str>,
    ) -> RegisteredSource {
        RegisteredSource {
            location: SourceBinding {
                uri: uri.into(),
                file: file.map(PathBuf::from),
                endpoint: endpoint.map(str::to_string),
                token_env: None,
            },
            name: Some(uri.into()),
            serial_number: Some("serial".into()),
            volume_name: Some("volume".into()),
            scope: scope.map(str::to_string),
            probe: None,
        }
    }

    #[test]
    fn normalizes_multiple_input_kinds_without_ingest_state() {
        let catalog = SourceInputCatalog::from_transport_bindings(TransportBindings {
            config_version: "0.1.0".into(),
            catalog: None,
            source_index: None,
            sources: Vec::new(),
            media_records: None,
            parallelism: None,
            registered_sources: vec![
                source(
                    "qnc://local/source/card",
                    Some("C:/card"),
                    None,
                    Some("card_relative"),
                ),
                source("qnc://local/source/folder", Some("C:/media"), None, None),
                source(
                    "qnc://lan/nas/source/news",
                    None,
                    Some("https://nas/qnc"),
                    None,
                ),
                source(
                    "qnc://intranet/mam/source/news",
                    None,
                    Some("https://mam/qnc"),
                    None,
                ),
            ],
        })
        .unwrap();

        let kinds = catalog
            .inputs()
            .iter()
            .map(|input| input.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                SourceInputKind::Computer,
                SourceInputKind::Computer,
                SourceInputKind::Lan,
                SourceInputKind::Intranet
            ]
        );
    }

    #[test]
    fn adapters_filter_browser_sources_by_ingest_browser_tab() {
        let catalog = SourceInputCatalog::from_transport_bindings(TransportBindings {
            config_version: "0.1.0".into(),
            catalog: None,
            source_index: None,
            sources: Vec::new(),
            media_records: None,
            parallelism: None,
            registered_sources: vec![
                source(
                    "qnc://local/source/card",
                    Some("C:/card"),
                    None,
                    Some("card_relative"),
                ),
                source("qnc://local/source/folder", Some("C:/media"), None, None),
                source(
                    "qnc://lan/nas/source/news",
                    None,
                    Some("https://nas/qnc"),
                    None,
                ),
                source(
                    "qnc://intranet/mam/source/news",
                    None,
                    Some("https://mam/qnc"),
                    None,
                ),
            ],
        })
        .unwrap();

        assert_eq!(ComputerInputAdapter.inputs(&catalog).len(), 2);
        assert_eq!(LanInputAdapter.inputs(&catalog).len(), 1);
        assert_eq!(IntranetInputAdapter.inputs(&catalog).len(), 1);
        assert!(catalog.browser_for(SourceInputKind::Computer).is_ok());
        assert!(catalog.browser_for(SourceInputKind::Lan).is_ok());
        assert!(catalog.browser_for(SourceInputKind::Intranet).is_ok());
    }

    #[test]
    fn browser_exposes_registered_source_uris() {
        let catalog = SourceInputCatalog::from_transport_bindings(TransportBindings {
            config_version: "0.1.0".into(),
            catalog: None,
            source_index: None,
            sources: Vec::new(),
            media_records: None,
            parallelism: None,
            registered_sources: vec![source(
                "qnc://local/source/card",
                Some("C:/card"),
                None,
                Some("card_relative"),
            )],
        })
        .unwrap();

        assert!(catalog.browser().is_ok());
    }
}
