use super::*;

fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("qnc-ai-catalog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

fn catalog(folder: &Path, selected: &str, backend: &str) -> PathBuf {
    let path = folder.join("catalog.json");
    std::fs::write(
        &path,
        format!(
            r#"{{"version":"1","transcribe":{{"selected":"{selected}","engines":[
              {{"id":"w.gpu","program":"qnc-transcribe-whisper","backend":"{backend}","model":"model.bin","vad_model":"vad.bin","threads":4}},
              {{"id":"w.cpu","program":"qnc-transcribe-whisper","backend":"cpu","model":"model.bin","threads":8}}]}}}}"#
        ),
    )
    .unwrap();
    path
}

#[test]
fn the_selected_engine_is_given_with_its_models() {
    let folder = folder("ok");
    std::fs::write(folder.join("model.bin"), b"m").unwrap();
    std::fs::write(folder.join("vad.bin"), b"v").unwrap();
    let engine = transcriber(&catalog(&folder, "w.gpu", "vulkan")).unwrap();
    assert_eq!(engine.backend, "vulkan");
    assert_eq!(engine.model, folder.join("model.bin"));
    assert_eq!(engine.vad_model, Some(folder.join("vad.bin")));
    let cpu = transcriber(&catalog(&folder, "w.cpu", "vulkan")).unwrap();
    assert_eq!((cpu.backend.as_str(), cpu.threads, cpu.vad_model), ("cpu", 8, None));
}

#[test]
fn a_missing_model_or_an_unknown_choice_is_an_error_not_another_engine() {
    let folder = folder("missing");
    let error = transcriber(&catalog(&folder, "w.gpu", "vulkan")).unwrap_err();
    assert!(error.contains("not on this host"), "{error}");
    std::fs::write(folder.join("model.bin"), b"m").unwrap();
    std::fs::write(folder.join("vad.bin"), b"v").unwrap();
    assert!(transcriber(&catalog(&folder, "w.none", "vulkan")).is_err());
    assert!(transcriber(&catalog(&folder, "w.gpu", "cuda")).unwrap_err().contains("unknown backend"));
}
