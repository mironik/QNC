use qnc_source_index_contract::*;

fn read_request() -> Request {
    Request {
        version: VERSION.into(),
        db_uri: "qnc://local/db/source_index".into(),
        operation: Operation::Read {
            record_id: "record-1".into(),
        },
    }
}
#[test]
fn strict_envelope_and_neutral_identity() {
    let request = read_request();
    request.validate().unwrap();
    let encoded = serde_json::to_string(&request).unwrap();
    assert_eq!(serde_json::from_str::<Request>(&encoded).unwrap(), request);
    let mut json = serde_json::to_value(request).unwrap();
    json["local_path"] = "C:/private/index.sqlite".into();
    assert!(serde_json::from_value::<Request>(json).is_err());
    for id in [
        "",
        "..",
        "/tmp/index",
        "C:\\index",
        "record?sql=x",
        "hello\n",
    ] {
        assert!(valid_id(id).is_err());
    }
}
#[test]
fn rejects_wrong_reply_database_version_and_operation() {
    let request = read_request();
    let valid = Reply {
        version: VERSION.into(),
        db_uri: request.db_uri.clone(),
        result: Ok(Data::Record(None)),
    };
    assert_eq!(
        valid.clone().validate(&request).unwrap(),
        Data::Record(None)
    );
    let mut wrong = valid.clone();
    wrong.version = "999".into();
    assert_eq!(wrong.validate(&request), Err(Error::Protocol));
    let mut wrong = valid.clone();
    wrong.db_uri = "qnc://lan/other/db/source_index".into();
    assert_eq!(wrong.validate(&request), Err(Error::Protocol));
    let mut wrong = valid;
    wrong.result = Ok(Data::Written(Receipt {
        batch_id: "one".into(),
        record_ids: vec![],
    }));
    assert_eq!(wrong.validate(&request), Err(Error::Protocol));
}
#[test]
fn empty_and_oversized_batches_are_not_accepted() {
    let mut input = Batch {
        batch_id: "one".into(),
        source_uri: "qnc://local/source/card".into(),
        proposals: vec![],
        file_facts: vec![],
    };
    assert_eq!(input.validate(), Err(Error::InvalidRequest));
    let root = SourceReference::new(&input.source_uri, ".").unwrap();
    let p = GroupProposal {
        root: root.clone(),
        recording_identity: "A".into(),
        original: root.descendant("A.MXF").unwrap(),
        proxies: vec![],
        related: vec![],
        evidence: qnc_source_groups::GroupEvidence {
            reader_id: "test".into(),
            document: root.descendant("INDEX.XML").unwrap(),
            locator: "A".into(),
        },
    };
    input.proposals = vec![p.clone(); MAX_GROUPS + 1];
    assert_eq!(input.validate(), Err(Error::TooLarge));
    input.proposals = vec![p];
    assert_eq!(input.validate(), Err(Error::InvalidRequest));
    input.file_facts = vec![
        FileFact {
            reference: root.descendant("A.MXF").unwrap(),
            state: FileState::File
        };
        MAX_FACTS + 1
    ];
    assert_eq!(input.validate(), Err(Error::TooLarge));
}
#[test]
fn contract_has_no_transport_database_or_application_dependency() {
    let cargo = include_str!("../Cargo.toml");
    for forbidden in [
        "rusqlite",
        "ureq",
        "qnc-transport-resolver",
        "qnc-ingest",
        "qnc-project",
        "qnc-scanner",
        "qnc-sony-metadata",
    ] {
        assert!(
            !cargo.contains(forbidden),
            "forbidden dependency {forbidden}"
        );
    }
}
