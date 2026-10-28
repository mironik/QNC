use super::*;
use std::fs;

fn root_with_a_prepared_sheet() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("catalogs").join("story-sheets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("news.json"),
        r#"{ "sheet_id": "news", "name": "News", "fields": [
            { "kind": "text", "label": "Slug", "id": "slug", "path": "slug" },
            { "kind": "duration", "label": "Length", "id": "target_seconds", "path": "target_seconds" } ] }"#,
    )
    .unwrap();
    root
}

#[test]
fn the_sheets_qnc_brings_all_load_with_their_choices() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (sheets, errors) = Sheets::load(&root);
    assert!(errors.is_empty(), "{errors:?}");
    let prepared: Vec<_> = sheets.list.iter().filter(|(_, origin)| *origin == Origin::Prepared).map(|(sheet, _)| sheet).collect();
    assert_eq!(prepared.len(), 6);
    for sheet in prepared {
        assert!(!sheet.name.starts_with('@'), "{} has a text code left", sheet.sheet_id);
        for field in &sheet.fields {
            if let Some(set) = &field.options {
                assert!(sheet.options.contains_key(set), "{}: {} has no choices {set}", sheet.sheet_id, field.path);
            }
        }
    }
}

#[test]
fn a_typed_name_becomes_a_code() {
    assert_eq!(code_of("Snimatelj / kamera"), "snimatelj_kamera");
    assert_eq!(code_of("Čitač vijesti"), "citac_vijesti");
    assert_eq!(code_of("2. urednik"), "f_2_urednik");
    assert!(valid_code(&code_of("Režija!")));
}

#[test]
fn a_prepared_sheet_is_never_written_over_its_copy_is_kept_and_chosen() {
    let root = root_with_a_prepared_sheet();
    let (mut sheets, errors) = Sheets::load(root.path());
    assert!(errors.is_empty(), "{errors:?}");
    let mut copy = sheets.current().unwrap().clone();
    copy.name = "Naša redakcija".into();
    let field = new_field(&copy, "Slug");
    assert_eq!(field.path, "slug_2", "a new field never takes a code the sheet has");
    copy.fields.push(field);
    sheets.keep(copy).unwrap();
    let (again, _) = Sheets::load(root.path());
    assert_eq!(again.list.len(), 2);
    let chosen = again.current().unwrap();
    assert_eq!((chosen.sheet_id.as_str(), chosen.fields.len()), ("nasa_redakcija", 3));
    assert_eq!(again.list[0].0.fields.len(), 2, "the prepared sheet stays as QNC brought it");
}

#[test]
fn the_overlay_hands_back_only_the_values_of_the_sheets_fields() {
    let root = root_with_a_prepared_sheet();
    let values = serde_json::json!({ "slug": "PROSVJED", "gone": "x" });
    let overlay = SheetOverlay::new(root.path(), "Prilog".into(), true, values, None, "Create".into());
    let (sheet, values) = overlay.filled().unwrap();
    assert_eq!(sheet.sheet_id, "news");
    assert_eq!(values, serde_json::json!({ "slug": "PROSVJED" }));
}
