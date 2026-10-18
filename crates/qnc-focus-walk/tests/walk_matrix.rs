//! Locks what every catalog key does on every panel of a walk (user 2026-10-18: moving the
//! row and list keys into their own blocks must not change one answer). `QNC_WRITE_GOLDEN=1`
//! writes the golden file again.

use qnc_focus_walk::*;

const ACTIONS: &str = include_str!("walk_matrix.actions");
const GOLDEN: &str = "tests/walk_matrix.golden";

fn walk() -> FocusWalk {
    FocusWalk::new((
        vec![SOURCE_KINDS, BROWSER, BROWSER_BUTTONS, CARDS, SOURCE_TIMELINE, SOURCE_BUTTONS],
        vec![
            (SOURCE_KINDS, vec!["kind_local", "kind_lan", "kind_net"]),
            (BROWSER_BUTTONS, vec!["confirm", "cancel"]),
            (POOL_TABS, vec!["tab_all", "tab_virtual"]),
            (SOURCE_BUTTONS, vec!["clear", "import", "all"]),
        ],
        vec![(CARDS, "preview_card"), (BROWSER, "open_folder")],
    ))
}

#[test]
fn every_key_on_every_panel_answers_as_before() {
    let mut actions: Vec<&str> = ACTIONS.split_whitespace().collect();
    actions.extend(["kind_lan", "confirm", "tab_virtual", "tab_all", "preview_card", "open_folder", "import"]);
    let panels = [SOURCE_KINDS, BROWSER, BROWSER_BUTTONS, CARDS, SOURCE_TIMELINE, SOURCE_BUTTONS, POOL_TABS];
    let mut lines = Vec::new();
    for panel in panels {
        let sequences: Vec<Vec<&str>> = actions
            .iter()
            .map(|action| vec![*action])
            .chain([
                vec!["step_forward_frame", "step_forward_frame", "step_forward_frame", "activate_focused_item"],
                vec!["step_back_frame", "activate_focused_item"],
                vec!["tab_virtual", "step_back_frame", "step_forward_frame"],
                vec!["activate_focused_item", "preview_card"],
                vec!["step_next_part", "focus_next", "step_next_part"],
            ])
            .collect();
        for sequence in sequences {
            let mut walk = walk();
            walk.focus(panel);
            let answers: Vec<String> = sequence.iter().map(|action| format!("{:?}/{}", walk.route(action), walk.panel())).collect();
            lines.push(format!("{panel} {} => {} | {:?}", sequence.join("+"), answers.join(" "), walk.view()));
        }
    }
    let got = lines.join("\n") + "\n";
    if std::env::var_os("QNC_WRITE_GOLDEN").is_some() {
        std::fs::write(GOLDEN, &got).expect("write golden");
        return;
    }
    let want = std::fs::read_to_string(GOLDEN).expect("golden file");
    for (number, (got, want)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(got, want, "line {}", number + 1);
    }
    assert_eq!(got.lines().count(), want.lines().count());
}
