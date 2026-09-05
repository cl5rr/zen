//! Islands wired into the real layout.
//!
//! `island.rs` tests the geometry and the container on their own. These tests are about the
//! wiring: that `FloatingSpace` keeps islands and tiles in agreement while windows are added,
//! removed, joined and split through the ordinary layout path.
//!
//! The load-bearing property is the partition -- every tile in exactly one island, no island
//! empty -- and it is checked after *every* operation in every existing layout test, because
//! `Layout::verify_invariants` reaches `FloatingSpace::verify_invariants`. These tests cover the
//! behaviour built on top of it.

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

/// Opens a floating window straight into an island, which is what `AddWindowTarget::Island` is
/// for. Not an `Op` because `Op` derives `Arbitrary` and an island id has no useful strategy.
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

/// Three windows opened without an island target are three separate islands. The first bullet of
/// PLAN.md's Phase 4 acceptance list.
#[test]
fn three_windows_are_three_islands() {
    let layout = check_ops([Op::AddOutput(1), floating(1), floating(2), floating(3)]);
    assert_eq!(layout.active_workspace().unwrap().island_count(), 3);
}

/// A one-item island *follows* its tile rather than driving it. That rule is what makes the
/// island layer invisible for a single floating window, and it is why adding islands did not
/// change a single existing layout test.
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

/// Joining tiles both windows inside one island: side by side, not stacked, and both within the
/// island's rect.
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

/// Removing a member leaves the island valid rather than stretching the survivor across a rect
/// sized for two: with one member left the rule flips back to following the tile.
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

/// Closing the last member removes the island. An empty island is never kept.
#[test]
fn emptying_an_island_removes_it() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1)]);
    let (_, island) = island_of_active(&layout);

    check_ops_on_layout(&mut layout, [Op::CloseWindow(1)]);

    let ws = layout.active_workspace().unwrap();
    assert_eq!(ws.island_count(), 0);
    assert!(!ws.has_island(island));
}

/// Pulling a window out gives it an island of its own; the one it left keeps the rest.
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

/// A single-window island has nothing to split out of, and says so rather than leaving an empty
/// island behind it.
#[test]
fn splitting_a_lone_window_is_a_no_op() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1)]);

    let ws = layout.active_workspace_mut().unwrap();
    assert!(!ws.split_island(&1));
    assert_eq!(ws.island_count(), 1);
}

/// `join_island` moves an existing window between islands -- the keyboard half of what dragging
/// a window onto a cluster will eventually do.
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

/// Direction becomes a spatial query. Two islands side by side: from the right one, "left" finds
/// the left one, and from the left one there is nothing further left.
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

/// A tabbed island stacks its members instead of tiling them.
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

/// A stale island id must never lose a window. It opens somewhere reasonable instead.
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

// ---------------------------------------------------------------------------------------------
// Spawn placement
//
// Where a new window lands. The rule is "it appears where I am looking", which is learnable in
// one use; every cleverer alternative fails because you cannot predict it, and unpredictable
// placement on an unbounded canvas is hostile.
// ---------------------------------------------------------------------------------------------

/// A window opens at the centre of the *view*, not at the centre of wherever the camera happened
/// to start. Without this, the first pan sends every subsequent window off-screen.
#[test]
fn a_new_window_opens_where_the_camera_is_looking() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    let first = render_pos(&layout, 1);

    // Pan the view left, which moves the canvas point under the viewport centre to the right.
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

/// Three windows opened in a row are three islands, even though each one opens exactly where the
/// last one is sitting. This is what `camera_is_framing` protects.
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

/// Framing an island with the camera is the deliberate act that makes the next window join it.
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

/// Panning by hand drops the framing, and with it the join. The camera is no longer holding
/// anything, so the next window is its own island again.
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

/// At map scale the viewport centre covers a screenful of nothing, so a window placed there is
/// an unreachable speck. The zoom rule joins the last-focused island instead -- and it outranks
/// the framing rule, because losing a window is worse than misplacing one.
#[test]
fn zoomed_far_out_a_new_window_joins_the_last_focused_island() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    let island = layout.active_workspace().unwrap().island_of(&1).unwrap();

    layout.set_camera_zoom(0.2);
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(layout.spawn_island_target(), Some(island));
}

// ---------------------------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------------------------

/// The rule this replaces compared centres along one axis only, so "left" would jump to whatever
/// happened to be furthest along x regardless of how far away it was in y. Here the window
/// actually beside you is much further left than the diagonal one, and it still has to win.
#[test]
fn focus_left_prefers_the_window_beside_you_over_a_nearer_diagonal() {
    let mut layout = check_ops([
        Op::AddOutput(1),
        // Beside: same row, far to the left.
        floating(1),
        move_to(100., 300.),
        // Diagonal: nearer in x, but nowhere near in y.
        floating(2),
        move_to(500., 1200.),
        // Us.
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

/// Direction walks the cluster before it leaves it: a member of your own island is simply nearer
/// than anything in the next one, so this needs no island special case.
#[test]
fn direction_walks_an_island_before_leaving_it() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), move_to(100., 100.), settle()]);
    let (first, island) = island_of_active(&layout);

    add_into_island(&mut layout, 2, island);
    check_ops_on_layout(&mut layout, [settle()]);

    // A third window, well to the right of the island.
    check_ops_on_layout(&mut layout, [floating(3), move_to(1400., 100.), settle()]);

    // From the far window, left lands on the island's rightmost member, not past it.
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

    // And left again walks within the island rather than jumping out of it.
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

/// Moving a window that is in a cluster moves the cluster. Sliding one member out would leave a
/// hole that `sync_islands` fills back in on the next frame, which reads as the move not working.
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

// ---------------------------------------------------------------------------------------------
// The canvas has to actually be unbounded
// ---------------------------------------------------------------------------------------------

#[track_caller]
fn visible_workspace_count(layout: &Layout<TestWindow>) -> usize {
    let MonitorSet::Normal { monitors, .. } = &layout.monitor_set else {
        unreachable!()
    };
    monitors[0].workspaces_with_render_geo().count()
}

/// A window left of or above the origin must still be drawn.
///
/// This is the bug that made the canvas a lie. The workspace rect's `loc` is the content
/// origin, so its `size` can only grow right and down; a window at negative coordinates fell
/// outside a rectangle with no way to express it, the cull decided the whole workspace covered
/// nothing, and *every* window vanished at once. Reported from real use, invisible to every
/// test here, because nothing had ever panned into negative space.
#[test]
fn a_window_at_negative_coordinates_is_still_drawn() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    assert_eq!(visible_workspace_count(&layout), 1);

    // Far left of, and far above, the origin.
    check_ops_on_layout(&mut layout, [move_to(-6000., -4000.), settle()]);

    // Go look at it.
    assert!(layout.camera_fit_all());
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(
        visible_workspace_count(&layout),
        1,
        "framed a window in negative space and the workspace was culled, \
         so nothing would be drawn at all"
    );
}

/// Pan to a window at negative coordinates *without* zooming out.
///
/// The distinction matters and cost me a wrong fix: `camera_fit_all` zooms out to frame things,
/// and `workspace_size` grows as zoom shrinks, so the rect becomes big enough to intersect the
/// output no matter what. Panning at zoom 1 is the case that actually breaks, and it is the one
/// a person hits by dragging a window left and following it.
#[test]
fn panning_to_negative_space_at_zoom_one_keeps_it_drawn() {
    let mut layout = check_ops([Op::AddOutput(1), floating(1), settle()]);
    check_ops_on_layout(&mut layout, [move_to(-5000., 0.), settle()]);

    layout.set_camera_zoom(1.);
    // Bring content at x = -5000 back into view.
    layout.camera_pan_immediate(Point::from((5300., 0.)));
    check_ops_on_layout(&mut layout, [settle()]);

    assert_eq!(
        visible_workspace_count(&layout),
        1,
        "panned to a window in negative space at zoom 1 and everything was culled"
    );
}

/// The same, panning by hand rather than framing, and in the positive direction too.
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
