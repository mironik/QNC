//! Owner-bound media record persistence, without media I/O or producer workflow.
pub use qnc_json_transport::{Access, Credentials};
pub use qnc_media_records as contract;
mod acquisition;
mod database;
mod transport;
pub use database::Store;
pub use transport::{respond, Client, ENDPOINT};

#[cfg(test)]
mod tests;
