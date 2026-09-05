use super::*;
use crate::layout::island::{Direction, IslandId, IslandLayout};

fn floating(id: usize) -> Op {
    Op::AddWindow {
        params: TestWindowParams {
            is_floating: true,
            ..TestWindowParams::new(id)
        },
    }
}

fn move_to(x: f64, y: f64) -> Op {
    Op::MoveFloatingWindow {
        id: None,
        x: PositionChange::SetFixed(x),
        y: PositionChange::SetFixed(y),
        animate: false,
    }
}

fn settle() -> Op {
    Op::AdvanceAnimations { msec_delta: 1000 }
}

#[track_caller]
fn add_into_island(layout: &mut Layout<TestWindow>, id: usize, island: IslandId) {
    let win = TestWindow::new(TestWindowParams {
        is_floating: true,
        ..TestWindowParams::new(id)
    });
    layout.add_window(
        win,
        AddWindowTarget::Island(island),
        None,
        None,
        false,
        true,
        ActivateWindow::default(),
    );
    layout.verify_invariants();
}

#[track_caller]
fn island_of_active(layout: &Layout<TestWindow>) -> (usize, IslandId) {
    let ws = layout.active_workspace().unwrap();
    let win = *ws.active_window().unwrap().id();
    (win, ws.island_of(&win).unwrap())
}

#[track_caller]
fn render_pos(layout: &Layout<TestWindow>, id: usize) -> Point<f64, Logical> {
    layout
        .active_workspace()
        .unwrap()
        .tiles_with_render_positions()
        .find(|(tile, _, _)| *tile.window().id() == id)
        .map(|(_, pos, _)| pos)
        .unwrap()
}

#[test]
fn a_lone_window_is_an_island_of_its_own() {
    let layout = check_ops([Op::AddOutput(1), floating(1)]);

    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.island_count(), 1);

    let (win, island) = island_of_active(&layout);
    assert_eq!(ws.island_windows(island), &[win]);
}

#[test]
fn three_windows_are_three_islands() {
    let layout = check_ops([Op::AddOutput(1), floating(1), floating(2), floating(3)]);
    assert_eq!(layout.active_workspace().unwrap().island_count(), 3);
}

#[test]
fn a_one_item_island_follows_its_window() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);

    let (_, island) = island_of_active(&layout);
    let before = layout
        .active_workspace()
        .unwrap()
        .island_rect(island)
        .unwrap();

    check_ops_on_layout(&mut layout, [move_to(400., 300.), settle()]);

    let after = layout
        .active_workspace()
        .unwrap()
        .island_rect(island)
        .unwrap();

    assert_ne!(
        (before.loc.x, before.loc.y),
        (after.loc.x, after.loc.y),
        "the island must move with the window it holds"
    );
}

#[test]
fn joining_an_island_tiles_both_windows() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);
    let (first, island) = island_of_active(&layout);

    add_into_island(&mut layout, 2, island);
    check_ops_on_layout(&mut layout, [settle()]);

    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.island_count(), 1, "the second window joined, not spawned");
    assert_eq!(ws.island_windows(island).len(), 2);
    let rect = ws.island_rect(island).unwrap();

    let a = render_pos(&layout, first);
    let b = render_pos(&layout, 2);

    assert_ne!(a.x, b.x, "columns must be side by side, not stacked");
    for pos in [a, b] {
        assert!(
            pos.x + 0.5 >= rect.loc.x && pos.x <= rect.loc.x + rect.size.w + 0.5,
            "member at {pos:?} is outside its island {rect:?}"
        );
    }
}

#[test]
fn removing_a_member_leaves_the_island_consistent() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);
    let (first, island) = island_of_active(&layout);

    add_into_island(&mut layout, 2, island);
    check_ops_on_layout(&mut layout, [settle(), Op::CloseWindow(2), settle()]);

    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.island_count(), 1);
    assert_eq!(ws.island_windows(island), &[first]);
}

#[test]
fn emptying_an_island_removes_it() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1)]);
    let (_, island) = island_of_active(&layout);

    check_ops_on_layout(&mut layout, [Op::CloseWindow(1)]);

    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.island_count(), 0);
    assert!(!ws.has_island(island));
}

#[test]
fn splitting_makes_a_new_island() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);
    let (first, island) = island_of_active(&layout);

    add_into_island(&mut layout, 2, island);
    check_ops_on_layout(&mut layout, [settle()]);

    let ws = layout.active_workspace_mut().unwrap();
    assert!(ws.split_island(&2));

    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.island_count(), 2);
    assert_eq!(ws.island_windows(island), &[first]);
    assert_ne!(ws.island_of(&2).unwrap(), island);
}

#[test]
fn splitting_a_lone_window_is_a_no_op() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1)]);

    let ws = layout.active_workspace_mut().unwrap();
    assert!(!ws.split_island(&1));
    assert_eq!(ws.island_count(), 1);
}

#[test]
fn joining_moves_a_window_between_islands() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        floating(1),
        move_to(100., 100.),
        floating(2),
        move_to(700., 100.),
        settle(),
    ]);

    let target = layout.active_workspace().unwrap().island_of(&1).unwrap();

    let ws = layout.active_workspace_mut().unwrap();
    assert!(ws.join_island(&2, target));

    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.island_count(), 1);
    assert_eq!(ws.island_windows(target).len(), 2);
}

#[test]
fn direction_finds_the_island_beside_you() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        floating(1),
        move_to(100., 100.),
        floating(2),
        move_to(700., 100.),
        settle(),
    ]);

    let ws = layout.active_workspace_mut().unwrap();
    assert!(
        ws.focus_island_in_direction(Direction::Left),
        "there is an island to the left"
    );

    let ws = layout.active_workspace().unwrap();
    assert_eq!(
        *ws.active_window().unwrap().id(),
        1,
        "focus must have moved to the left island"
    );

    let ws = layout.active_workspace_mut().unwrap();
    assert!(
        !ws.focus_island_in_direction(Direction::Left),
        "nothing further left, so nothing to focus"
    );
}

#[test]
fn a_tabbed_island_stacks_its_members() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);
    let (first, island) = island_of_active(&layout);

    add_into_island(&mut layout, 2, island);
    check_ops_on_layout(&mut layout, [settle()]);

    let ws = layout.active_workspace_mut().unwrap();
    assert!(ws.set_island_layout(island, IslandLayout::Tabbed));
    check_ops_on_layout(&mut layout, [settle()]);

    let a = render_pos(&layout, first);
    let b = render_pos(&layout, 2);
    assert!(
        (a.x - b.x).abs() < 1. && (a.y - b.y).abs() < 1.,
        "tabbed members must sit on top of each other, got {a:?} and {b:?}"
    );
}

#[test]
fn a_stale_island_target_still_opens_the_window() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1)]);
    let (_, island) = island_of_active(&layout);

    check_ops_on_layout(&mut layout, [Op::CloseWindow(1)]);
    add_into_island(&mut layout, 2, island);

    let ws = layout.active_workspace().unwrap();
    assert!(ws.has_window(&2), "the window must still be mapped");
    assert_eq!(ws.island_count(), 1);
}

// -
// -

#[test]
fn a_new_window_opens_where_the_camera_is_looking() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    let first = render_pos(&layout, 1);

    layout.camera_pan_immediate(Point::from((-600., 0.)));
    check_ops_on_layout(&mut layout, [settle(), floating(2), settle()]);

    let ws = layout.active_workspace().unwrap();
    let a = ws.island_rect(ws.island_of(&1).unwrap()).unwrap();
    let b = ws.island_rect(ws.island_of(&2).unwrap()).unwrap();

    assert!(
        b.loc.x - a.loc.x > 400.,
        "the second window should open ~600px along the canvas from the first, \
         got {} -> {} (first tile rendered at {first:?})",
        a.loc.x,
        b.loc.x
    );
}

#[test]
fn opening_windows_in_a_row_does_not_collapse_into_one_island() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);

    assert_eq!(
        layout.spawn_island_target(),
        None,
        "an island under the viewport by accident is not one you are looking at"
    );

    check_ops_on_layout(&mut layout, [floating(2), settle(), floating(3), settle()]);
    assert_eq!(layout.active_workspace().unwrap().island_count(), 3);
}

#[test]
fn a_framed_island_takes_the_next_window() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    let island = layout.active_workspace().unwrap().island_of(&1).unwrap();

    assert!(layout.camera_maximize());
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(
        layout.spawn_island_target(),
        Some(island),
        "a window opened while an island is framed belongs in that island"
    );
}

#[test]
fn panning_away_drops_the_join() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);

    assert!(layout.camera_maximize());
    check_ops_on_layout(&mut layout, [settle()]);
    assert!(layout.spawn_island_target().is_some());

    layout.camera_pan_immediate(Point::from((-600., 0.)));
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(layout.spawn_island_target(), None);
}

#[test]
fn zoomed_far_out_a_new_window_joins_the_last_focused_island() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    let island = layout.active_workspace().unwrap().island_of(&1).unwrap();

    layout.set_camera_zoom(0.2);
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(layout.spawn_island_target(), Some(island));
}

// -
// -

#[test]
fn focus_left_prefers_the_window_beside_you_over_a_nearer_diagonal() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        floating(1),
        move_to(100., 300.),
        floating(2),
        move_to(500., 1200.),
        floating(3),
        move_to(800., 300.),
        settle(),
    ]);

    let ws = layout.active_workspace_mut().unwrap();
    assert!(ws.focus_left());

    assert_eq!(
        *layout
            .active_workspace()
            .unwrap()
            .active_window()
            .unwrap()
            .id(),
        1,
        "the window in the same row wins over a nearer one that is nowhere near it"
    );
}

#[test]
fn direction_walks_an_island_before_leaving_it() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);
    let (first, island) = island_of_active(&layout);

    add_into_island(&mut layout, 2, island);
    check_ops_on_layout(&mut layout, [settle()]);

    check_ops_on_layout(&mut layout, [floating(3), move_to(1400., 100.), settle()]);

    let ws = layout.active_workspace_mut().unwrap();
    assert!(ws.focus_left());
    let landed = *layout
        .active_workspace()
        .unwrap()
        .active_window()
        .unwrap()
        .id();
    assert!(
        landed == first || landed == 2,
        "left from outside must land in the island, got {landed}"
    );

    let ws = layout.active_workspace_mut().unwrap();
    assert!(ws.focus_left());
    let inner = *layout
        .active_workspace()
        .unwrap()
        .active_window()
        .unwrap()
        .id();
    assert_ne!(inner, landed);
    assert_eq!(
        layout.active_workspace().unwrap().island_of(&inner),
        Some(island),
        "the second step stays inside the cluster"
    );
}

#[test]
fn moving_a_clustered_window_moves_the_island() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);
    let (_, island) = island_of_active(&layout);

    add_into_island(&mut layout, 2, island);
    check_ops_on_layout(&mut layout, [settle()]);

    let before = layout
        .active_workspace()
        .unwrap()
        .island_rect(island)
        .unwrap();

    let ws = layout.active_workspace_mut().unwrap();
    ws.move_right();
    check_ops_on_layout(&mut layout, [settle()]);

    let ws = layout.active_workspace().unwrap();
    let after = ws.island_rect(island).unwrap();

    assert!(after.loc.x > before.loc.x, "the island should have moved");
    assert_eq!(ws.island_windows(island).len(), 2, "and kept both members");
}

// -
// -

#[track_caller]
fn visible_workspace_count(layout: &Layout<TestWindow>) -> usize {
    let MonitorSet::Normal { monitors, .. } = &layout.monitor_set else {
        unreachable!()
    };
    monitors[0].workspaces_with_render_geo().count()
}

#[test]
fn a_window_at_negative_coordinates_is_still_drawn() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    assert_eq!(visible_workspace_count(&layout), 1);

    check_ops_on_layout(&mut layout, [move_to(-6000., -4000.), settle()]);

    assert!(layout.camera_fit_all());
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(
        visible_workspace_count(&layout),
        1,
        "framed a window in negative space and the workspace was culled, \
         so nothing would be drawn at all"
    );
}

#[test]
fn panning_to_negative_space_at_zoom_one_keeps_it_drawn() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    check_ops_on_layout(&mut layout, [move_to(-5000., 0.), settle()]);

    layout.set_camera_zoom(1.);
    layout.camera_pan_immediate(Point::from((5300., 0.)));
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(
        visible_workspace_count(&layout),
        1,
        "panned to a window in negative space at zoom 1 and everything was culled"
    );
}

#[test]
fn panning_far_in_any_direction_keeps_content_drawn() {
    for (dx, dy) in [(-9000., 0.), (9000., 0.), (0., -7000.), (0., 7000.)] {
        let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
        check_ops_on_layout(&mut layout, [move_to(dx, dy), settle()]);

        assert!(layout.camera_fit_all());
        check_ops_on_layout(&mut layout, [settle()]);

        assert_eq!(
            visible_workspace_count(&layout),
            1,
            "window at ({dx}, {dy}) got culled away"
        );
    }
}
