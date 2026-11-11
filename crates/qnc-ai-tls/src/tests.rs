use super::*;
use std::net::TcpListener;

#[test]
fn a_client_talks_only_to_the_server_of_the_fingerprint_it_names() {
    let folder = tempfile::tempdir().unwrap();
    let (config, print) = server_config(folder.path()).unwrap();
    assert_eq!(server_config(folder.path()).unwrap().1, print, "the certificate is made once and kept");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let Ok(mut tls) = accept(stream.unwrap(), config.clone()) else { continue };
            let line = read_line(&mut tls, 100).unwrap();
            tls.write_all(&[line.as_slice(), b" back\n"].concat()).unwrap();
            tls.flush().unwrap();
        }
    });
    let mut tls = connect(TcpStream::connect(address).unwrap(), client_config(&print).unwrap()).unwrap();
    tls.write_all(b"hello\n").unwrap();
    tls.flush().unwrap();
    assert_eq!(read_line(&mut tls, 100).unwrap(), b"hello back");
    let other = "0".repeat(64);
    assert!(connect(TcpStream::connect(address).unwrap(), client_config(&other).unwrap()).is_err(), "another certificate is refused");
    assert!(client_config("abc").is_err());
    server.join().unwrap();
}
