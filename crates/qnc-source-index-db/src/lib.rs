//! Public source-index DB adapter; no application, scanner, parser or media workflow.
mod database;
mod transport;
pub use database::{Access, Store};
pub use qnc_source_index_contract as contract;
pub use transport::{respond, Client, Credentials, ENDPOINT};

#[cfg(test)]
mod tests;
