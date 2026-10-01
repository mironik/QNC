use super::*;

const NAMES: BoardNames<'static> =
    BoardNames { monitor: "monitor", head: "head", body: "body", right: "right", dock: "dock" };

fn sizes() -> BoardSizes {
    BoardSizes {
        left_ratio: 0.31,
        divider_width: 5.0,
        left_min_width: 280.0,
        right_min_width: 200.0,
        monitor_height: 1.0,
        head_height: 1.0,
        dock_height: 1.0,
        monitor_aspect: None,
    }
}

fn solved(width: f32, height: f32, sizes: BoardSizes) -> Solved {
    let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(width, height));
    solve(&sizes.standard_layout(&NAMES), rect)
}

fn face(solved: &Solved, name: &str) -> Rect {
    solved.faces.iter().find(|(face, _)| face == name).map(|(_, rect)| *rect).unwrap()
}

#[test]
fn project_places_fill_the_board_around_one_pixel_places() {
    let s = solved(1000.0, 700.0, sizes());
    let block = |name| s.block(name).unwrap();
    assert_eq!(block("dock").height(), 1.0);
    assert_eq!(block("monitor").height(), 1.0);
    assert_eq!(block("head").height(), 1.0);
    assert_eq!(block("body").height(), 700.0 - 3.0);
    assert_eq!(face(&s, "left").width(), 310.0);
    assert_eq!(face(&s, "divider").width(), 5.0);
    assert_eq!(block("right").left(), 315.0);
    assert_eq!(block("right").right(), 1000.0);
}

#[test]
fn faces_paint_board_left_divider_right_and_blocks_draw_in_board_order() {
    let s = solved(1000.0, 700.0, sizes());
    let faces: Vec<&str> = s.faces.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(faces, ["bg", "left", "divider", "right"]);
    let blocks: Vec<&str> = s.blocks.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(blocks, ["monitor", "head", "body", "right", "dock"]);
}

#[test]
fn the_dock_never_takes_more_than_its_share_and_columns_keep_their_minimum() {
    let s = solved(400.0, 300.0, BoardSizes { dock_height: 500.0, ..sizes() });
    assert!((s.block("dock").unwrap().height() - 300.0 * DOCK_MAX_SHARE).abs() < 0.01);
    assert_eq!(face(&s, "left").width(), 280.0);
}

#[test]
fn a_monitor_with_a_picture_shape_follows_the_column_width() {
    let aspect = MonitorAspect {
        ratio: 16.0 / 9.0,
        reserve_below: 190.0,
        min_height: 160.0,
        width_inset: 32.0,
        min_width: 240.0,
    };
    let sizes = BoardSizes {
        left_ratio: 0.365,
        monitor_aspect: Some(aspect),
        head_height: 34.0,
        ..sizes()
    };
    let s = solved(1000.0, 900.0, sizes);
    // Column 365 px: picture 333 px wide, 187.3 px high.
    assert!((s.block("monitor").unwrap().height() - 333.0 * 9.0 / 16.0).abs() < 0.01);
    assert_eq!(s.block("head").unwrap().top(), s.block("monitor").unwrap().bottom());
}

#[test]
fn the_monitor_never_grows_past_the_column() {
    let s = solved(1000.0, 400.0, BoardSizes { monitor_height: 900.0, head_height: 30.0, ..sizes() });
    assert_eq!(s.block("monitor").unwrap().bottom(), face(&s, "left").bottom());
    assert_eq!(s.block("body").unwrap().height(), 0.0);
}

#[test]
fn an_empty_name_leaves_a_place_empty() {
    let names = BoardNames { monitor: "", head: "", body: "list", right: "settings", dock: "" };
    let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1000.0, 700.0));
    let s = solve(&sizes().standard_layout(&names), rect);
    let blocks: Vec<&str> = s.blocks.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(blocks, ["list", "settings"]);
}

#[test]
fn a_layout_is_data_that_round_trips_and_can_place_a_monitor_on_the_right() {
    let layout = sizes().standard_layout(&NAMES);
    let json = serde_json::to_string(&layout).unwrap();
    assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), layout);

    // Another arrangement of the same blocks: the monitor on the right.
    let other: Layout = serde_json::from_str(
        r#"{"root":{"split":{"axis":"horizontal","parts":[
            {"size":"rest","node":{"block":{"name":"body"}}},
            {"size":{"px":{"px":400.0,"max_share":null}},"node":{"block":{"name":"monitor"}}}
        ]}}}"#,
    )
    .unwrap();
    let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1000.0, 500.0));
    let s = solve(&other, rect);
    assert_eq!(s.block("monitor").unwrap().left(), 600.0);
    assert_eq!(s.block("body").unwrap().width(), 600.0);
}

#[test]
fn a_board_in_the_desktop_frame_is_one_tree_over_the_footer() {
    let board = sizes().standard_layout(&NAMES);
    let whole = surface_with_footer(30.0).nest("surface", &board);
    assert_eq!(whole.block_names(), ["monitor", "head", "body", "right", "dock", "footer"]);
    let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1000.0, 730.0));
    let s = solve(&whole, rect);
    assert_eq!(s.block("footer").unwrap().top(), 700.0);
    assert_eq!(s.block("body").unwrap().height(), 700.0 - 3.0);
    // The board's own face paints the place it takes in the frame.
    assert_eq!(face(&s, "bg"), Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1000.0, 700.0)));
}

#[test]
fn a_bare_frame_leaves_the_board_as_it_is() {
    let board = sizes().standard_layout(&NAMES);
    let frame = Frame::bare();
    assert_eq!(frame.layout.nest(&frame.slot, &board), board);
}
