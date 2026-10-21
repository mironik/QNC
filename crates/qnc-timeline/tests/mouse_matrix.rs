//! Locks what every pointer does on every timeline panel (user rule 2026-10-20: the mouse
//! map is one rule, written once, locked). Each line: panel, what the pointer did, the
//! catalog action with its value and target. `QNC_WRITE_GOLDEN=1` writes the file again;
//! a change of the answer is a change of the rule and needs the user's decision.

use qnc_timeline::{AudioLane, TimelineIntent};
use qnc_wheel_jog::Wheel;

const GOLDEN: &str = "tests/mouse_matrix.golden";

fn intents() -> Vec<TimelineIntent> {
    let id = || "x".to_string();
    vec![
        TimelineIntent::CueFrame(5),
        TimelineIntent::SelectVirtual { id: id(), frame: 5 },
        TimelineIntent::OpenVirtual { id: id(), frame: 5 },
        TimelineIntent::OpenSlot { id: id(), frame: 5 },
        TimelineIntent::Keep(Some(5)),
        TimelineIntent::Keep(None),
        TimelineIntent::SelectMarkerSlot { id: id(), frame: 5 },
        TimelineIntent::SelectCover { id: id(), frame: 5 },
        TimelineIntent::TakeCoverOnStrip { id: id(), frame: 5 },
        TimelineIntent::AddMarkerAt(5),
        TimelineIntent::GoHome,
        TimelineIntent::ToggleVideoMode,
        TimelineIntent::LaneLabel { lane: AudioLane::A1, frame: None },
        TimelineIntent::LaneLabel { lane: AudioLane::A2, frame: Some(5) },
        TimelineIntent::ToggleAudioExpand(AudioLane::A1),
        TimelineIntent::ChooseA1Channel(2),
        TimelineIntent::ChooseA2Channel(2),
        TimelineIntent::TakeMark(true),
        TimelineIntent::TakeMark(false),
        TimelineIntent::TakeMarker { id: id(), frame: 5 },
        TimelineIntent::TakeSlot { id: id(), frame: 5 },
        TimelineIntent::TakeCover { id: id(), frame: 5 },
        TimelineIntent::TakeAudioLane(AudioLane::A1),
        TimelineIntent::TakeAudioLane(AudioLane::A2),
        TimelineIntent::TakeSound { lane: AudioLane::A1, frame: 5 },
        TimelineIntent::TakeSound { lane: AudioLane::A2, frame: 5 },
        TimelineIntent::TakeMarkerSound { id: id() },
        TimelineIntent::DragMark { in_mark: true, frame: 5 },
        TimelineIntent::DragMark { in_mark: false, frame: 5 },
        TimelineIntent::Cut,
        TimelineIntent::Wheel(Wheel { up: false, ctrl: false }),
        TimelineIntent::Wheel(Wheel { up: true, ctrl: false }),
        TimelineIntent::Wheel(Wheel { up: false, ctrl: true }),
        TimelineIntent::Wheel(Wheel { up: true, ctrl: true }),
        TimelineIntent::None,
    ]
}

fn table() -> String {
    let mut out = String::new();
    for panel in ["source_timeline", "segments"] {
        for intent in intents() {
            // A Wrap row's frames stand 100 frames into the program.
            let answer = qnc_timeline::event(&intent, panel, &|frame| frame + 100)
                .map_or("-".to_string(), |event| format!("{} value={:?} target={:?}", event.action_id, event.value, event.target));
            out.push_str(&format!("{panel} | {intent:?} | {answer}\n"));
        }
    }
    out
}

#[test]
fn every_pointer_on_every_panel_is_the_locked_action() {
    let now = table();
    if std::env::var_os("QNC_WRITE_GOLDEN").is_some() {
        std::fs::write(GOLDEN, &now).unwrap();
    }
    let golden = std::fs::read_to_string(GOLDEN).expect("the golden mouse matrix");
    for (line, (want, got)) in golden.lines().zip(now.lines()).enumerate() {
        assert_eq!(want, got, "line {}", line + 1);
    }
    assert_eq!(golden.lines().count(), now.lines().count());
}
