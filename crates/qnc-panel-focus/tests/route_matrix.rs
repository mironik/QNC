//! Locks what every catalog key does on every panel (user 2026-10-18: splitting the
//! router into blocks must not change one answer). The golden file was written by the
//! router before the split; `QNC_WRITE_GOLDEN=1` writes it again.

use qnc_panel_focus::{FocusLayout, Panel, PanelFocus};
use qnc_program_segments::ProgramSegments;
use qnc_source_preview::SourcePreview;
use qnc_timeline::TimelineProjection;
use qnc_wrap_session::WrapSession;

const ACTIONS: &str = include_str!("route_matrix.actions");
const GOLDEN: &str = "tests/route_matrix.golden";

fn layout() -> FocusLayout {
    FocusLayout::new((
        vec!["pool_tabs", "pool", "source_timeline", "source_buttons", "segments"],
        vec!["editorial_tab_all", "editorial_tab_virtual", "editorial_tab_broll"],
        vec!["save_virtual_shot", "add_ton_segment", "add_off_segment"],
        vec!["segment_panel"],
    ))
}

fn timelines() -> [(&'static str, TimelineProjection); 2] {
    let marked = TimelineProjection {
        duration_frames: 100,
        playhead_frame: Some(20),
        source_in_frame: Some(10),
        source_out_frame: Some(50),
        ..Default::default()
    };
    [("empty", TimelineProjection::default()), ("marked", marked)]
}

fn row(panel: Panel, name: &str, timeline: &TimelineProjection, actions: &[&str]) -> String {
    let mut focus = PanelFocus::with_layout(layout());
    focus.set(panel);
    let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
    let mut timeline = timeline.clone();
    let mut answers = Vec::new();
    for action in actions {
        let routed = focus.route(*action, (&mut preview, &mut wrap, &mut segments, &mut timeline));
        let pick = focus.take_pick();
        answers.push(format!("{routed:?}/{:?}/{pick:?}", focus.panel()));
    }
    let view = focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline, &mut []));
    format!(
        "{panel:?} {name} {} => {} | button={:?} pool={:?} in={:?} out={:?} playhead={:?} wrap={}",
        actions.join("+"),
        answers.join(" "),
        view.button,
        view.pool_step,
        timeline.source_in_frame,
        timeline.source_out_frame,
        timeline.playhead_frame,
        wrap.is_active(),
    )
}

#[test]
fn every_key_on_every_panel_answers_as_before() {
    let actions: Vec<&str> = ACTIONS.split_whitespace().collect();
    let panels = [Panel::PoolTabs, Panel::Pool, Panel::SourceTimeline, Panel::SourceButtons, Panel::Segments];
    let mut lines = Vec::new();
    for (name, timeline) in timelines() {
        for panel in panels {
            for action in &actions {
                lines.push(row(panel, name, &timeline, &[action]));
            }
            // Pairs that depend on what the first key left: a taken mark, a moved tab or button.
            for pair in [
                ["select_mark_in", "step_forward_frame"],
                ["select_mark_out", "step_back_frame"],
                ["select_mark_in", "clear_focus"],
                ["step_forward_frame", "activate_focused_item"],
                ["step_back_frame", "activate_focused_item"],
                ["select_audio_a1", "step_next_part"],
                ["select_audio_a2", "clear_focus"],
                ["mark_in", "play_pause"],
                ["wheel_source_forward", "wheel_segments_back"],
                ["focus_next", "activate_focused_item"],
                ["editorial_tab_virtual", "step_forward_frame"],
            ] {
                lines.push(row(panel, name, &timeline, &pair));
            }
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
