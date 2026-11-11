//! TLS between QNC and its AI server (docs/102 step 6, user 2026-11-11: the AI server on the LAN
//! or the intranet carries pictures and sound of a story not yet aired, and a token). The server
//! makes its own certificate the first time it starts and says its fingerprint; a QNC trusts that
//! one certificate only, by the fingerprint its setting names, so no other computer can stand in
//! for the server and nothing goes over the network readable. No certificate authority is
//! needed.
//!
//! A TLS stream cannot be read and written from two threads, so one thread runs it ([`pump`]):
//! what it reads goes to the caller, what the caller sends is written.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ConnectionCommon, DigitallySignedStruct, SideData, SignatureScheme};
use sha2::{Digest, Sha256};

pub use rustls::{ClientConfig, ClientConnection, ServerConfig, ServerConnection, StreamOwned};

pub const MODULE_ID: &str = "qnc.module.ai-tls";
/// The name the server's certificate is made for and a client asks for.
pub const SERVER_NAME: &str = "qnc-ai-server";
/// The server's certificate and key, beside the settings of its computer.
pub const CERT_FILE: &str = "ai-server-cert.pem";
pub const KEY_FILE: &str = "ai-server-key.pem";

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// The SHA-256 of a certificate, as lowercase hex: what a client's setting names.
pub fn fingerprint(certificate: &[u8]) -> String {
    Sha256::digest(certificate).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The server's TLS: its certificate and key from `folder`, made there the first time; and the
/// fingerprint a client names.
pub fn server_config(folder: &Path) -> Result<(Arc<ServerConfig>, String), String> {
    let (cert_file, key_file) = (folder.join(CERT_FILE), folder.join(KEY_FILE));
    if !cert_file.is_file() || !key_file.is_file() {
        let key = rcgen::KeyPair::generate().map_err(|e| e.to_string())?;
        let params = rcgen::CertificateParams::new(vec![SERVER_NAME.to_string()]).map_err(|e| e.to_string())?;
        let certificate = params.self_signed(&key).map_err(|e| e.to_string())?;
        std::fs::write(&key_file, key.serialize_pem()).map_err(|e| format!("{}: {e}", key_file.display()))?;
        std::fs::write(&cert_file, certificate.pem()).map_err(|e| format!("{}: {e}", cert_file.display()))?;
    }
    let certificate = CertificateDer::from_pem_file(&cert_file).map_err(|e| format!("{}: {e}", cert_file.display()))?;
    let key = PrivateKeyDer::from_pem_file(&key_file).map_err(|e| format!("{}: {e}", key_file.display()))?;
    let fingerprint = fingerprint(&certificate);
    let config = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .map_err(|e| e.to_string())?;
    Ok((Arc::new(config), fingerprint))
}

/// A client's TLS: it trusts only the certificate whose fingerprint is `fingerprint`.
pub fn client_config(fingerprint: &str) -> Result<Arc<ClientConfig>, String> {
    let fingerprint = fingerprint.trim().to_ascii_lowercase();
    if fingerprint.len() != 64 || !fingerprint.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("the fingerprint of the AI server is 64 hex digits (SHA-256 of its certificate)".into());
    }
    let provider = provider();
    let config = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { fingerprint, provider }))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// A TLS connection to the AI server at `stream`, its certificate the one of `config`.
pub fn connect(stream: TcpStream, config: Arc<ClientConfig>) -> Result<StreamOwned<ClientConnection, TcpStream>, String> {
    let name = ServerName::try_from(SERVER_NAME).map_err(|e| e.to_string())?;
    let connection = ClientConnection::new(config, name).map_err(|e| e.to_string())?;
    let mut stream = StreamOwned::new(connection, stream);
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock).map_err(|e| format!("TLS with the AI server: {e}"))?;
    }
    Ok(stream)
}

/// A TLS connection of a client accepted at `stream`.
pub fn accept(stream: TcpStream, config: Arc<ServerConfig>) -> Result<StreamOwned<ServerConnection, TcpStream>, String> {
    let connection = ServerConnection::new(config).map_err(|e| e.to_string())?;
    let mut stream = StreamOwned::new(connection, stream);
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock).map_err(|e| format!("TLS: {e}"))?;
    }
    Ok(stream)
}

/// Reads one line (without its end) from `stream`, a byte at a time so nothing after it is taken.
pub fn read_line<C, S>(stream: &mut StreamOwned<C, TcpStream>, longest: usize) -> Result<Vec<u8>, String>
where
    C: DerefMut + Deref<Target = ConnectionCommon<S>>,
    S: SideData,
{
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Err("the connection closed".into()),
            Ok(_) if byte[0] == b'\n' => return Ok(line),
            Ok(_) if line.len() >= longest => return Err("a line too long".into()),
            Ok(_) => line.push(byte[0]),
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Runs `stream` until either side ends: what it reads goes to `on_read` (false ends it), what
/// comes on `outgoing` is written. One thread, so the TLS stream is never shared.
pub fn pump<C, S>(mut stream: StreamOwned<C, TcpStream>, mut on_read: impl FnMut(&[u8]) -> bool, outgoing: Receiver<Vec<u8>>)
where
    C: DerefMut + Deref<Target = ConnectionCommon<S>>,
    S: SideData,
{
    let _ = stream.sock.set_read_timeout(Some(Duration::from_millis(20)));
    let mut buffer = vec![0u8; 64 << 10];
    'running: loop {
        loop {
            match outgoing.try_recv() {
                Ok(bytes) => {
                    if stream.write_all(&bytes).and_then(|()| stream.flush()).is_err() {
                        break 'running;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'running,
            }
        }
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if !on_read(&buffer[..read]) {
                    break;
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }
    stream.conn.send_close_notify();
    let _ = stream.conn.complete_io(&mut stream.sock);
    let _ = stream.sock.shutdown(std::net::Shutdown::Both);
}

/// Trusts the one certificate of the fingerprint the setting names; its signatures are checked
/// as any TLS checks them.
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("the AI server's certificate is not the one its setting names".into()))
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests;
