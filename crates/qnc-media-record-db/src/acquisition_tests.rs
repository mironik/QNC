use super::*;

fn begin() -> BeginAcquisition {
    BeginAcquisition {
        attempt_id: "probe-A".into(),
        clip_id: "clip-A".into(),
        expected_revision: 1,
        media_uri: input("A").metadata.original.media_uri,
        document_uri: "qnc://local/artifact/probe-A".into(),
    }
}
fn stored() -> FinishAcquisition {
    let begin = begin();
    FinishAcquisition {
        attempt_id: begin.attempt_id,
        outcome: AcquisitionOutcome::Stored {
            document_uri: begin.document_uri.clone(),
        },
        document: Some(Document {
            document_uri: begin.document_uri,
            media_type: DocumentType::Json,
            text: serde_json::json!({"format":{"filename":begin.media_uri},"streams":[]})
                .to_string(),
        }),
    }
}

#[test]
fn only_first_claim_grants_even_after_reopen_and_new_attempt_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut db = local(&path, true, Access::ReadWrite);
    db.write(input("A")).unwrap();
    let first = db.begin_acquisition(begin()).unwrap();
    assert!(first.granted);
    assert!(first.acquisition.outcome.is_none());
    assert!(!db.begin_acquisition(begin()).unwrap().granted);
    drop(db);
    let mut db = local(&path, false, Access::ReadWrite);
    let mut different = begin();
    different.attempt_id = "second-process".into();
    different.document_uri = "qnc://local/artifact/another".into();
    let denied = db.begin_acquisition(different).unwrap();
    assert!(!denied.granted);
    assert_eq!(denied.acquisition, first.acquisition);
    assert_eq!(
        db.acquisition(&begin().media_uri).unwrap(),
        Some(first.acquisition)
    );
    assert_eq!(count(&path, "media_acquisitions"), 1);
}

#[test]
fn independent_connections_grant_only_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    local(&path, true, Access::ReadWrite)
        .write(input("A"))
        .unwrap();
    let barrier = Arc::new(Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let path = path.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let mut db = local(&path, false, Access::ReadWrite);
                let mut claim = begin();
                claim.attempt_id = format!("competitor-{i}");
                barrier.wait();
                db.begin_acquisition(claim).unwrap().granted
            })
        })
        .collect();
    let winners = handles
        .into_iter()
        .filter_map(|h| h.join().unwrap().then_some(()))
        .count();
    assert_eq!(winners, 1);
}

#[test]
fn success_commits_raw_evidence_and_replaying_finish_does_not_change_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut db = local(&path, true, Access::ReadWrite);
    db.write(input("A")).unwrap();
    db.begin_acquisition(begin()).unwrap();
    let saved = db.finish_acquisition(stored()).unwrap();
    assert!(saved.finished_at_unix_ms.is_some());
    assert_eq!(db.finish_acquisition(stored()).unwrap(), saved);
    assert_eq!(
        db.document(&begin().document_uri).unwrap(),
        stored().document
    );
    let mut changed = stored();
    changed.document.as_mut().unwrap().text.push(' ');
    assert_eq!(db.finish_acquisition(changed), Err(Error::Conflict));
    assert!(!db.begin_acquisition(begin()).unwrap().granted);
    drop(db);
    let mut reader = local(&path, false, Access::ReadOnly);
    assert_eq!(reader.acquisition(&begin().media_uri).unwrap(), Some(saved));
    assert_eq!(
        reader.document(&begin().document_uri).unwrap(),
        stored().document
    );
}

#[test]
fn failure_and_uncertainty_never_reopen_acquisition() {
    for outcome in [
        AcquisitionOutcome::Failed {
            code: "failed".into(),
        },
        AcquisitionOutcome::Uncertain {
            code: "timeout".into(),
        },
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("media.sqlite");
        let mut db = local(&path, true, Access::ReadWrite);
        db.write(input("A")).unwrap();
        db.begin_acquisition(begin()).unwrap();
        let finish = FinishAcquisition {
            attempt_id: begin().attempt_id,
            outcome,
            document: None,
        };
        let result = db.finish_acquisition(finish.clone()).unwrap();
        assert_eq!(db.finish_acquisition(finish).unwrap(), result);
        assert_eq!(db.finish_acquisition(stored()), Err(Error::Conflict));
        assert!(!db.begin_acquisition(begin()).unwrap().granted);
        assert!(db.document(&begin().document_uri).unwrap().is_none());
    }
}

#[test]
fn absent_wrong_or_final_snapshot_cannot_start_probe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut db = local(&path, true, Access::ReadWrite);
    assert_eq!(db.begin_acquisition(begin()), Err(Error::InvalidRequest));
    db.write(input("A")).unwrap();
    let mut wrong = begin();
    wrong.media_uri = "qnc://local/media/unrelated".into();
    assert_eq!(db.begin_acquisition(wrong), Err(Error::InvalidRequest));
    let mut final_write = input("A");
    final_write.request_id = "final-A".into();
    final_write.expected_revision = 1;
    final_write.phase = Phase::Final;
    db.write(final_write).unwrap();
    assert_eq!(db.begin_acquisition(begin()), Err(Error::Finalized));
    assert_eq!(count(&path, "media_acquisitions"), 0);
}

#[test]
fn invalid_finish_does_not_persist_document_or_finish_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut db = local(&path, true, Access::ReadWrite);
    db.write(input("A")).unwrap();
    db.begin_acquisition(begin()).unwrap();
    let mut wrong = stored();
    wrong.document.as_mut().unwrap().text =
        "{\"format\":{\"filename\":\"wrong\"},\"streams\":[]}".into();
    assert_eq!(db.finish_acquisition(wrong), Err(Error::InvalidMetadata));
    assert!(db
        .acquisition(&begin().media_uri)
        .unwrap()
        .unwrap()
        .outcome
        .is_none());
    assert!(db.document(&begin().document_uri).unwrap().is_none());
    assert_eq!(count(&path, "evidence_documents"), 1);
}

#[test]
fn claim_does_not_hold_write_lock_during_media_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut first = local(&path, true, Access::ReadWrite);
    first.write(input("A")).unwrap();
    first.begin_acquisition(begin()).unwrap();
    let mut second = local(&path, false, Access::ReadWrite);
    second.write(input("B")).unwrap();
    first.finish_acquisition(stored()).unwrap();
}

#[test]
fn local_lan_and_intranet_enforce_write_grants_and_replay_denial() {
    for env in ["lan", "intranet"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("media.sqlite");
        let uri = format!("qnc://{env}/test/db/media_records");
        let host = Host::start(&path, &uri);
        let mut writer = host.client(&uri, WRITE);
        let mut reader = host.client(&uri, READ);
        writer.write(input("A")).unwrap();
        assert_eq!(reader.begin_acquisition(begin()), Err(Error::AccessDenied));
        assert!(writer.begin_acquisition(begin()).unwrap().granted);
        assert!(
            !host
                .client(&uri, WRITE)
                .begin_acquisition(begin())
                .unwrap()
                .granted
        );
        assert_eq!(
            reader.finish_acquisition(stored()),
            Err(Error::AccessDenied)
        );
        let finished = writer.finish_acquisition(stored()).unwrap();
        assert_eq!(
            reader.acquisition(&begin().media_uri).unwrap(),
            Some(finished)
        );
        let mut local_reader = local(&path, false, Access::ReadOnly);
        assert_eq!(
            local_reader.begin_acquisition(begin()),
            Err(Error::AccessDenied)
        );
        assert_eq!(
            local_reader.finish_acquisition(stored()),
            Err(Error::AccessDenied)
        );
    }
}

#[test]
fn claim_crash_child() {
    let Some(path) = std::env::var_os("QNC_CLAIM_CRASH_FIXTURE") else {
        return;
    };
    let mut db = local(Path::new(&path), false, Access::ReadWrite);
    assert!(db.begin_acquisition(begin()).unwrap().granted);
    std::process::exit(23);
}

#[test]
fn committed_claim_survives_child_exit_without_destructors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    local(&path, true, Access::ReadWrite)
        .write(input("A"))
        .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::acquisition_tests::claim_crash_child",
            "--nocapture",
        ])
        .env("QNC_CLAIM_CRASH_FIXTURE", &path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(23), "{output:?}");
    let mut db = local(&path, false, Access::ReadWrite);
    let denied = db.begin_acquisition(begin()).unwrap();
    assert!(!denied.granted);
    assert!(denied.acquisition.outcome.is_none());
}

#[test]
fn a_new_clip_id_cannot_bypass_a_claim_for_the_same_media() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    let mut db = local(&path, true, Access::ReadWrite);
    db.write(input("A")).unwrap();
    db.begin_acquisition(begin()).unwrap();
    let mut claim = begin();
    claim.clip_id = "different-project-clip".into();
    claim.attempt_id = "different-request".into();
    assert!(!db.begin_acquisition(claim).unwrap().granted);
    let mut proxy = begin();
    proxy.media_uri = input("A").metadata.proxy.unwrap().media_uri;
    assert_eq!(db.begin_acquisition(proxy.clone()), Err(Error::Conflict));
    proxy.attempt_id = "proxy-request".into();
    proxy.document_uri = "qnc://local/artifact/proxy-result".into();
    assert!(db.begin_acquisition(proxy).unwrap().granted);
    assert_eq!(count(&path, "media_acquisitions"), 2);
}

#[test]
fn committed_begin_with_lost_reply_cannot_grant_on_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("media.sqlite");
    local(&path, true, Access::ReadWrite)
        .write(input("A"))
        .unwrap();
    let uri = "qnc://lan/test/db/media_records";
    let mut store = Store::open_owner_binding(&path, Access::ReadWrite, false).unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let resolver = ResolverConfig::new("")
        .with_lan_authority("test", format!("http://{}", server.server_addr()));
    let host = thread::spawn(move || {
        let mut request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        let mut text = String::new();
        request.as_reader().read_to_string(&mut text).unwrap();
        let body: Request = serde_json::from_str(&text).unwrap();
        let Data::AcquisitionClaim(claim) = store.execute(&body).unwrap() else {
            panic!()
        };
        assert!(claim.granted);
        request.respond(tiny_http::Response::empty(503)).unwrap();
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        respond(
            request,
            &mut store,
            uri,
            &Credentials::new(READ, WRITE).unwrap(),
        );
    });
    let mut client = Client::open(&resolver, uri, Access::ReadWrite, Some(WRITE)).unwrap();
    assert!(client.begin_acquisition(begin()).is_err());
    let denied = client.begin_acquisition(begin()).unwrap();
    assert!(!denied.granted);
    assert!(denied.acquisition.outcome.is_none());
    host.join().unwrap();
}
