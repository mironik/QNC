use super::*;
use qnc_transport_resolver::ResolverConfig;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

fn fixture(uri: &str) -> Catalog {
    let applications = [("first", "a"), ("lite", "c"), ("full", "c"), ("last", "d")]
        .into_iter()
        .map(|(id, group)| Application {
            application_id: format!("qnc.{id}"),
            tab_id: id.into(),
            label: id.into(),
            priority_group: group.into(),
            system: false,
            removable: true,
            host_mode: HostMode::ExternalComponent,
            desktop_entry: id.into(),
            standalone_executable: id.into(),
        })
        .collect();
    Catalog {
        catalog_id: CATALOG_ID.into(),
        schema_version: 1,
        catalog_uri: uri.into(),
        selection_policy: "one_per_priority_group".into(),
        observed_at_unix_ms: 1,
        target_os: "linux".into(),
        target_cpu: "aarch64".into(),
        applications,
        unavailable: vec![],
    }
}

#[test]
fn radio_groups_replace_one_variant_without_affecting_another_template() {
    let c = fixture(DEFAULT_URI);
    let mut one = vec![];
    let mut two = vec![];
    choose_group(&c, &mut one, "c", Some("qnc.lite")).unwrap();
    choose_group(&c, &mut two, "c", Some("qnc.lite")).unwrap();
    choose_group(&c, &mut one, "c", Some("qnc.full")).unwrap();
    assert_eq!(one, ["qnc.full"]);
    assert_eq!(two, ["qnc.lite"]);
    let g = groups(&c, &one);
    assert_eq!(g[1].choices.iter().filter(|c| c.selected).count(), 1);
    assert!(choose_group(&c, &mut one, "c", Some("qnc.first")).is_err());
    assert!(choose_group(&c, &mut one, "c", Some("qnc.missing")).is_err());
    assert_eq!(one, ["qnc.full"]);
    choose_group(&c, &mut one, "c", None).unwrap();
    assert!(one.is_empty());
    assert!(groups(&c, &one)[1].no_selection);
}

#[test]
fn snapshot_sorts_gaps_and_rejects_forged_group_order() {
    let c = fixture(DEFAULT_URI);
    let ids = ["qnc.last", "qnc.first", "qnc.full"].map(str::to_owned);
    let mut selection = SelectionSnapshot::from_catalog(&c, &ids).unwrap();
    assert_eq!(
        selection
            .applications
            .iter()
            .map(|app| app.priority_group.as_str())
            .collect::<Vec<_>>(),
        ["a", "c", "d"]
    );
    selection.validate().unwrap();
    selection.applications.swap(0, 2);
    assert!(selection.validate().is_err());
    selection.applications.swap(0, 2);
    selection.applications[2].priority_group = "c".into();
    assert!(selection.validate().is_err());
}

#[test]
fn missing_duplicate_and_unavailable_selections_are_rejected() {
    let c = fixture(DEFAULT_URI);
    for ids in [
        vec!["qnc.missing"],
        vec!["qnc.first", "qnc.first"],
        vec!["qnc.lite", "qnc.full"],
    ] {
        assert!(SelectionSnapshot::from_catalog(
            &c,
            &ids.into_iter().map(str::to_owned).collect::<Vec<_>>()
        )
        .is_err());
    }
}

#[test]
fn local_binding_is_read_only_and_missing_does_not_create_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("catalog.json");
    let resolver = ResolverConfig::new(tmp.path()).with_local_binding(DEFAULT_URI, &path);
    assert!(read_uri(&resolver, DEFAULT_URI).is_err());
    assert!(!path.exists());
    let bytes = serde_json::to_vec(&fixture(DEFAULT_URI)).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        read_uri(&resolver, DEFAULT_URI).unwrap().applications.len(),
        4
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

fn http_response(status: &str, body: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/proxy", listener.local_addr().unwrap());
    let status = status.to_owned();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while request.len() < 16384 && !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        let header = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        stream.write_all(header.as_bytes()).unwrap();
        let _ = stream.write_all(&body);
        String::from_utf8(request).unwrap()
    });
    (base, handle)
}

#[test]
fn lan_and_intranet_read_json_through_http_proxy_mapping() {
    for environment in ["lan", "intranet"] {
        let uri = format!("qnc://{environment}/studio/catalog/applications");
        let (base, server) = http_response("200 OK", serde_json::to_vec(&fixture(&uri)).unwrap());
        let resolver = ResolverConfig::new("unused")
            .with_lan_authority("studio", &base)
            .with_intranet_authority("studio", &base);
        let catalog = read_uri(&resolver, &uri).unwrap();
        assert_eq!(catalog.catalog_uri, uri);
        assert!(server
            .join()
            .unwrap()
            .starts_with("GET /proxy/catalog/applications HTTP/1.1"));
    }
}

#[test]
fn network_rejects_status_redirect_wrong_identity_and_oversize() {
    let uri = "qnc://lan/studio/catalog/applications";
    let bodies = [
        ("404 Not Found", vec![]),
        ("302 Found", vec![]),
        ("200 OK", serde_json::to_vec(&fixture(DEFAULT_URI)).unwrap()),
        ("200 OK", vec![b' '; CATALOG_LIMIT as usize + 1]),
    ];
    for (status, body) in bodies {
        let (base, server) = http_response(status, body);
        let resolver = ResolverConfig::new("unused").with_lan_authority("studio", base);
        assert!(read_uri(&resolver, uri).is_err());
        server.join().unwrap();
    }
}

#[test]
fn unknown_network_authority_does_not_fall_back_to_local() {
    let resolver = ResolverConfig::new("unused");
    assert!(read_uri(&resolver, "qnc://lan/missing/catalog/applications").is_err());
}
