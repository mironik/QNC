use super::*;
use serde_json::json;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn codes(list: &[&str]) -> Vec<String> {
    list.iter().map(|code| code.to_string()).collect()
}

#[test]
fn turning_a_feature_on_turns_on_what_it_needs_and_what_is_needed_stays() {
    let features = Features::load(&root()).expect("features");
    let mut chosen = Vec::new();
    features.turn_on(&mut chosen, BROLL_SUGGEST);
    assert_eq!(chosen, codes(&[SHOT_ANALYSIS, SHOT_CONTENT, BROLL_SUGGEST]));
    let refused = features.turn_off(&mut chosen, SHOT_ANALYSIS).expect_err("shot analysis is needed");
    assert_eq!(refused, codes(&["ai.feature.shot_content", "ai.feature.broll_suggest"]));
    assert_eq!(chosen.len(), 3, "nothing turned off");
    features.turn_off(&mut chosen, BROLL_SUGGEST).expect("nothing needs it");
    features.turn_off(&mut chosen, SHOT_CONTENT).expect("nothing needs it now");
    features.turn_off(&mut chosen, SHOT_ANALYSIS).expect("nothing needs it now");
    assert!(chosen.is_empty());
    assert!(features.check(&codes(&[TRANSLATE])).is_err(), "a choice without what it needs is an error");
    assert!(features.check(&codes(&["nothing"])).is_err());
}

#[test]
fn a_list_that_needs_an_unknown_or_itself_is_refused() {
    assert!(Features::parse(r#"{"features":[{"code":"a","name":"x","needs":["b"]}]}"#).is_err());
    assert!(Features::parse(r#"{"features":[{"code":"a","name":"x","needs":["b"]},{"code":"b","name":"y","needs":["a"]}]}"#).is_err());
}

#[test]
fn the_prepared_templates_keep_the_rule_and_an_own_one_is_kept_chosen_and_deleted() {
    let features = Features::load(&root()).expect("features");
    let place = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(place.path().join("catalogs").join("ai")).unwrap();
    for file in ["features.json", "templates.json"] {
        std::fs::copy(root().join("catalogs").join("ai").join(file), place.path().join("catalogs").join("ai").join(file)).unwrap();
    }
    let (prepared, errors) = templates(place.path(), &features);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(prepared[0].id, NO_AI);
    assert!(prepared.iter().all(|template| !template.own));

    let mut choice = Choice::load(place.path());
    let mut settings = json!({"ai": {"language": "hr"}});
    choice.choose_template(&mut settings, "news_full");
    choice.switch(&mut settings, SHOT_CONTENT, false);
    assert!(!choice.message.is_empty(), "refused: B-roll suggestions need it");
    choice.switch(&mut settings, TRANSLATE, false);
    choice.switch(&mut settings, BROLL_SUGGEST, false);
    choice.name = "Bez pokrivalica".into();
    choice.keep_template(&mut settings);
    let kept = choice.templates.iter().find(|template| template.own).expect("kept").clone();
    assert_eq!(template_of(&settings), kept.id);
    assert!(!chosen_of(&settings).contains(&BROLL_SUGGEST.to_string()));
    assert_eq!(settings["ai"]["language"], json!("hr"), "nothing else changes");

    choice.delete_template(&prepared[0].id);
    assert!(choice.templates.iter().any(|template| template.id == NO_AI), "a prepared template is never deleted");
    choice.delete_template(&kept.id);
    assert!(choice.templates.iter().all(|template| !template.own));
}

#[test]
fn an_action_of_a_feature_the_project_has_off_is_grey() {
    let features = Features::load(&root()).expect("features");
    let on = [TRANSCRIPTION, SHOT_ANALYSIS, SHOT_CONTENT];
    let gate = Gate::new(&features, |code| on.contains(&code));
    assert!(gate.allows("transcribe_marks") && gate.allows("editorial_tab_ai_broll"));
    assert!(!gate.allows("script_suggest_broll") && !gate.allows("subsegment_join_smooth"));
    assert!(gate.allows("play_pause"), "an action no feature starts");
    assert!(Gate::default().allows("script_suggest_broll"), "without a project every action is allowed");
    let twice = r#"{"features": [{"code": "a", "name": "n", "needs": [], "actions": ["x"]}, {"code": "b", "name": "n", "needs": [], "actions": ["x"]}]}"#;
    assert!(Features::parse(twice).is_err());
}
