use std::fmt::Write as _;

use insta::assert_snapshot;
use zen_config::animations::{Curve, EasingParams, Kind};

use super::*;

fn format_tiles(layout: &Layout<TestWindow>) -> String {
    let mut buf = String::new();
    let ws = layout.active_workspace().unwrap();
    let mut tiles: Vec<_> = ws.tiles_with_render_positions().collect();

    tiles.sort_by_key(|(tile, _, _)| tile.window().id());
    for (tile, pos, _visible) in tiles {
        let Size { w, h, .. } = tile.animated_tile_size();
        let Point { x, y, .. } = pos;
        writeln!(&mut buf, "{w:>3.0} × {h:>3.0} at x:{x:>3.0} y:{y:>3.0}").unwrap();
    }
    buf
}

fn make_options() -> Options {
    const LINEAR: Kind = Kind::Easing(EasingParams {
        duration_ms: 1000,
        curve: Curve::Linear,
    });

    let mut options = Options {
        layout: zen_config::Layout {
            gaps: 0.0,
            ..Default::default()
        },
        ..Options::default()
    };
    options.animations.window_resize.anim.kind = LINEAR;
    options.animations.window_movement.0.kind = LINEAR;

    options
}

fn set_up_two_in_column() -> Layout<TestWindow> {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::ConsumeWindowIntoColumn,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];

    check_ops_with_options(make_options(), ops)
}

#[test]
fn height_resize_animates_next_y() {
    let mut layout = set_up_two_in_column();

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::AdjustFixed(-50),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 50)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 ×  75 at x:  0 y:  0
    200 × 200 at x:  0 y: 75
    ");

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 ×  50 at x:  0 y:  0
    200 × 200 at x:  0 y: 50
    ");
}

#[test]
fn clientside_height_change_doesnt_animate() {
    let mut layout = set_up_two_in_column();

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");

    let ops = [
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 50)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 ×  50 at x:  0 y:  0
    200 × 200 at x:  0 y: 50
    ");
}

#[test]
fn height_resize_and_back() {
    let mut layout = set_up_two_in_column();

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 500 },
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 150 at x:  0 y:  0
    200 × 200 at x:  0 y:150
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 150 at x:  0 y:  0
    200 × 200 at x:  0 y:150
    ");

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 125 at x:  0 y:  0
    200 × 200 at x:  0 y:125
    ");

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");
}

#[test]
fn height_resize_and_cancel() {
    let mut layout = set_up_two_in_column();

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 50 },
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 105 at x:  0 y:  0
    200 × 200 at x:  0 y:105
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:105
    ");

    Op::AdvanceAnimations { msec_delta: 950 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");
}

#[test]
fn height_resize_and_back_during_another_y_anim() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    let mut layout = check_ops_with_options(make_options(), ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::ConsumeWindowIntoColumn.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x: 50 y: 50
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x: 50 y: 50
    ");

    Op::AdvanceAnimations { msec_delta: 200 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 120 at x:  0 y:  0
    200 × 200 at x: 30 y: 80
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 120 at x:  0 y:  0
    200 × 200 at x: 30 y: 80
    ");

    Op::AdvanceAnimations { msec_delta: 200 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 116 at x:  0 y:  0
    200 × 200 at x: 10 y: 84
    ");

    Op::AdvanceAnimations { msec_delta: 100 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 114 at x:  0 y:  0
    200 × 200 at x:  0 y: 86
    ");

    Op::AdvanceAnimations { msec_delta: 700 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");
}

#[test]
fn height_resize_and_cancel_during_another_y_anim() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    let mut layout = check_ops_with_options(make_options(), ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::ConsumeWindowIntoColumn.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::AdvanceAnimations { msec_delta: 500 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x: 50 y: 50
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 50 },
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 105 at x:  0 y:  0
    200 × 200 at x: 45 y: 58
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x: 45 y: 58
    ");

    Op::AdvanceAnimations { msec_delta: 450 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y: 78
    ");

    Op::AdvanceAnimations { msec_delta: 550 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");
}

#[test]
fn height_resize_before_another_y_anim_then_back() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 200 },
    ];
    let mut layout = check_ops_with_options(make_options(), ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 120 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::ConsumeWindowIntoColumn.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 120 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::AdvanceAnimations { msec_delta: 600 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 180 at x:  0 y:  0
    200 × 200 at x: 40 y:120
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 180 at x:  0 y:  0
    200 × 200 at x: 40 y:120
    ");

    Op::AdvanceAnimations { msec_delta: 200 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 164 at x:  0 y:  0
    200 × 200 at x: 20 y:116
    ");

    Op::AdvanceAnimations { msec_delta: 200 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 148 at x:  0 y:  0
    200 × 200 at x:  0 y:112
    ");

    Op::AdvanceAnimations { msec_delta: 600 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");
}

#[test]
fn height_resize_before_another_y_anim_then_cancel() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 20 },
    ];
    let mut layout = check_ops_with_options(make_options(), ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 102 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::ConsumeWindowIntoColumn.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 102 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::AdvanceAnimations { msec_delta: 20 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 104 at x:  0 y:  0
    200 × 200 at x: 98 y:  4
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x: 98 y:  4
    ");

    Op::AdvanceAnimations { msec_delta: 980 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");
}

#[test]
fn clientside_height_change_during_another_y_anim() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
        Op::ConsumeWindowIntoColumn,
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 200 },
    ];
    let mut layout = check_ops_with_options(make_options(), ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x: 80 y: 20
    ");

    let ops = [
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 200 at x:  0 y:  0
    200 × 200 at x: 80 y: 20
    ");

    Op::AdvanceAnimations { msec_delta: 800 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 200 at x:  0 y:  0
    200 × 200 at x:  0 y:200
    ");
}

#[test]
fn height_resize_cancel_with_stationary_second_window() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 20 },
    ];
    let mut options = make_options();
    options.animations.window_movement.0.off = true;
    let mut layout = check_ops_with_options(options, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 102 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::ConsumeWindowIntoColumn.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 102 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::AdvanceAnimations { msec_delta: 20 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 104 at x:  0 y:  0
    200 × 200 at x:  0 y:200
    ");

    let ops = [
        Op::SetWindowHeight {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");
}

#[test]
fn width_resize_and_cancel() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::FocusColumnLeft,
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    let mut layout = check_ops_with_options(make_options(), ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    let ops = [
        Op::SetWindowWidth {
            id: None,
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(200, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 50 },
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    105 × 100 at x:  0 y:  0
    200 × 200 at x:105 y:  0
    ");

    let ops = [
        Op::SetWindowWidth {
            id: None,
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:105 y:  0
    ");

    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");
}

#[test]
fn width_resize_and_cancel_of_column_to_the_left() {
    let ops = [
        Op::AddOutput(1),
        Op::AddWindow {
            params: TestWindowParams::new(1),
        },
        Op::AddWindow {
            params: TestWindowParams::new(2),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::SetForcedSize {
            id: 2,
            size: Some(Size::new(200, 200)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::CompleteAnimations,
    ];
    let mut layout = check_ops_with_options(make_options(), ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");

    let ops = [
        Op::SetWindowWidth {
            id: Some(1),
            change: SizeChange::SetFixed(200),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(200, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
        Op::AdvanceAnimations { msec_delta: 50 },
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    105 × 100 at x: -5 y:  0
    200 × 200 at x:100 y:  0
    ");

    let ops = [
        Op::SetWindowWidth {
            id: Some(1),
            change: SizeChange::SetFixed(100),
        },
        Op::SetForcedSize {
            id: 1,
            size: Some(Size::new(100, 100)),
        },
        Op::Communicate(1),
        Op::Communicate(2),
    ];
    check_ops_on_layout(&mut layout, ops);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x: -5 y:  0
    200 × 200 at x:100 y:  0
    ");

    Op::AdvanceAnimations { msec_delta: 1000 }.apply(&mut layout);

    assert_snapshot!(format_tiles(&layout), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:100 y:  0
    ");
}
