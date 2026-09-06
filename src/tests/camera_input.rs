use smithay::utils::{Logical, Point};

use super::*;
use crate::layout::HitType;

const OUTPUT_W: i32 = 1280;
const OUTPUT_H: i32 = 720;

const ZOOM_STEPS: [f64; 4] = [0.5, 0.75, 1.0, 1.25];

fn fixture_with_window() -> Fixture {
    let mut f = Fixture::new();
    f.add_output(1, (OUTPUT_W as u16, OUTPUT_H as u16));
    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(200, 150);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    f.zen_complete_animations();
    f
}

fn set_zoom(f: &mut Fixture, zoom: f64) -> f64 {
    f.zen().layout.set_camera_zoom(zoom);
    f.zen().layout.overview_zoom()
}

fn hit_at(f: &mut Fixture, probe: Point<f64, Logical>) -> Option<(Point<f64, Logical>, f64)> {
    match f.zen().contents_under(probe).window {
        Some((_, HitType::Input { win_pos, scale })) => Some((win_pos, scale)),
        _ => None,
    }
}

fn delivered_at(f: &mut Fixture, probe: Point<f64, Logical>) -> Option<Point<f64, Logical>> {
    let (_, surface_pos) = f.zen().contents_under(probe).surface?;
    Some(probe - surface_pos)
}

fn window_bbox(f: &mut Fixture) -> Option<(Point<f64, Logical>, Point<f64, Logical>)> {
    let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
    let (mut max_x, mut max_y) = (f64::MIN, f64::MIN);
    let mut found = false;
    for y in (2..OUTPUT_H).step_by(4) {
        for x in (2..OUTPUT_W).step_by(4) {
            let p = Point::<f64, Logical>::from((f64::from(x), f64::from(y)));
            if hit_at(f, p).is_some() {
                found = true;
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
        }
    }
    found.then(|| (Point::from((min_x, min_y)), Point::from((max_x, max_y))))
}

#[test]
fn zoom_can_exceed_one() {
    let mut f = fixture_with_window();
    let zoom_in = set_zoom(&mut f, 2.5);
    assert!(
        zoom_in > 1.0,
        "expected magnification above 1.0, got {zoom_in}; \
         camera-maximize is impossible without it"
    );
    let zoom_out = set_zoom(&mut f, 0.5);
    assert!(zoom_out < 1.0, "expected zoom out below 1.0, got {zoom_out}");
}

#[test]
fn window_stays_interactive_at_every_zoom() {
    let mut f = fixture_with_window();

    for requested in ZOOM_STEPS {
        let zoom = set_zoom(&mut f, requested);
        let bbox = window_bbox(&mut f);
        assert!(
            bbox.is_some(),
            "at zoom {zoom} no point on the output produced an Input hit; \
             pointer events would not reach the client anywhere. \
             zen would have downgraded this to Activate via to_activate()."
        );
        let (min, _) = bbox.unwrap();
        let (_, scale) = hit_at(&mut f, min).unwrap();
        assert!(
            (scale - zoom).abs() < 1e-9,
            "hit reported scale {scale} but camera zoom is {zoom}"
        );
    }
}

#[test]
fn pointer_motion_scales_into_surface_coordinates() {
    let mut f = fixture_with_window();

    for requested in ZOOM_STEPS {
        let zoom = set_zoom(&mut f, requested);

        let (min, max) = window_bbox(&mut f)
            .unwrap_or_else(|| panic!("no Input hit anywhere at zoom {zoom}"));

        let p1 = Point::<f64, Logical>::from((min.x + 8., min.y + 8.));
        let step = Point::<f64, Logical>::from(((max.x - min.x) / 2., (max.y - min.y) / 3.));
        let p2 = p1 + step;

        let d1 = delivered_at(&mut f, p1)
            .unwrap_or_else(|| panic!("no surface under {p1:?} at zoom {zoom}"));
        let d2 = delivered_at(&mut f, p2)
            .unwrap_or_else(|| panic!("no surface under {p2:?} at zoom {zoom}"));

        let delivered_delta = d2 - d1;
        let expected = step.downscale(zoom);
        let err = (delivered_delta.x - expected.x)
            .abs()
            .max((delivered_delta.y - expected.y).abs());

        assert!(
            err < 1e-6,
            "at zoom {zoom}: moving the pointer by {step:?} moved the client's coordinate by \
             {delivered_delta:?}, expected {expected:?} (error {err}). \
             Cursor and client would disagree about where the pointer is."
        );
    }
}

#[test]
fn unzoomed_behaviour_is_unchanged() {
    let mut f = fixture_with_window();
    let zoom = set_zoom(&mut f, 1.0);
    assert_eq!(zoom, 1.0);

    let (min, _) = window_bbox(&mut f).expect("window not found at zoom 1");
    let probe = Point::<f64, Logical>::from((min.x + 12., min.y + 9.));

    let (win_pos, scale) = hit_at(&mut f, probe).unwrap();
    assert_eq!(scale, 1.0);

    let delivered = delivered_at(&mut f, probe).expect("no surface");
    let expected = probe - win_pos;
    let err = (delivered.x - expected.x)
        .abs()
        .max((delivered.y - expected.y).abs());
    assert!(
        err < 1e-9,
        "unzoomed delivery regressed: got {delivered:?}, expected {expected:?}"
    );
}

#[test]
fn resize_edges_are_reachable_when_zoomed() {
    let mut f = fixture_with_window();
    let output = f.zen_output(1);

    for requested in ZOOM_STEPS {
        let zoom = set_zoom(&mut f, requested);
        let (min, _) = window_bbox(&mut f)
            .unwrap_or_else(|| panic!("no Input hit anywhere at zoom {zoom}"));
        let _ = f.zen().layout.resize_edges_under(&output, min);
    }
}

#[test]
fn pan_moves_content_by_the_view_delta() {
    let mut f = fixture_with_window();
    set_zoom(&mut f, 1.0);

    let (before_min, _) = window_bbox(&mut f).expect("no window found");

    let delta = Point::<f64, Logical>::from((40., 24.));
    f.zen().layout.camera_pan_immediate(delta);
    f.zen_complete_animations();

    let (after_min, _) = window_bbox(&mut f).expect("window vanished after panning");

    let moved = after_min - before_min;
    let err = (moved.x - delta.x).abs().max((moved.y - delta.y).abs());
    assert!(
        err <= 4.0,
        "panned by {delta:?} but content moved {moved:?}"
    );
}

#[test]
fn pan_does_not_disturb_surface_coordinates() {
    let mut f = fixture_with_window();

    for zoom in [1.0, 1.25] {
        set_zoom(&mut f, zoom);
        f.zen().layout.camera_reset();
        f.zen_complete_animations();
        set_zoom(&mut f, zoom);

        let (min, max) = window_bbox(&mut f)
            .unwrap_or_else(|| panic!("no window at zoom {zoom}"));
        let probe = Point::<f64, Logical>::from(((min.x + max.x) / 2., (min.y + max.y) / 2.));

        let before = delivered_at(&mut f, probe)
            .unwrap_or_else(|| panic!("no surface under {probe:?} at zoom {zoom}"));

        let delta = Point::<f64, Logical>::from((-24., 12.));
        f.zen().layout.camera_pan_immediate(delta);

        let after = delivered_at(&mut f, probe + delta)
            .unwrap_or_else(|| panic!("no surface after panning at zoom {zoom}"));

        let err = (after.x - before.x).abs().max((after.y - before.y).abs());
        assert!(
            err < 1e-6,
            "at zoom {zoom}: panning changed the client's coordinate from {before:?} to              {after:?}; clicks would drift as you pan"
        );
    }
}

// The map is a menu of places, so a click on a bubble has to resolve to the island the
// render put under the pointer. The mapping from view to canvas goes through the
// camera's pan, the zoom, and the centring offset, and getting any of the three wrong
// still travels somewhere, just to the wrong island.
#[test]
fn clicking_a_bubble_on_the_map_travels_to_that_island() {
    let mut f = fixture_with_window();

    // Islands are the floating layer's partition, so a tiled window has none.
    f.zen().layout.set_window_floating(None, true);
    f.zen_complete_animations();

    f.zen().layout.set_camera_zoom(0.42);
    f.zen_complete_animations();

    let island = f
        .zen()
        .layout
        .active_monitor_ref()
        .unwrap()
        .active_workspace_ref()
        .floating()
        .islands()
        .islands()
        .next()
        .map(|i| i.rect())
        .expect("the window should be in an island");

    // The centre of that island, mapped forward into the view the same way the render
    // does, is where a person would actually click.
    let mon = f.zen().layout.active_monitor_ref().unwrap();
    let centre = Point::from((
        island.loc.x + island.size.w / 2.,
        island.loc.y + island.size.h / 2.,
    ));
    let zoom = mon.camera_zoom();
    let view_size = mon.camera_view_size();
    let ws_size = view_size.upscale(zoom);
    let offset = (view_size.to_point() - ws_size.to_point()).downscale(2.);
    let click = mon.camera_pan_offset_view() + offset + centre.upscale(zoom);

    let before = f.zen().layout.active_monitor_ref().unwrap().camera_zoom();
    assert!(f.zen().layout.travel_to_island_at(click), "the click missed the island");
    f.zen_complete_animations();

    let after = f.zen().layout.active_monitor_ref().unwrap().camera_zoom();
    assert!(
        after > before,
        "travelling should leave the map, zoom went {before} -> {after}"
    );

    // And the island should now be roughly centred rather than merely closer.
    let mon = f.zen().layout.active_monitor_ref().unwrap();
    let landed = mon.view_to_workspace(Point::from((
        mon.camera_view_size().w / 2.,
        mon.camera_view_size().h / 2.,
    )));
    assert!(
        (landed.x - centre.x).abs() < 2. && (landed.y - centre.y).abs() < 2.,
        "island centre {centre:?} did not land under the view centre, got {landed:?}"
    );
}

#[test]
fn clicking_empty_canvas_on_the_map_is_not_a_travel() {
    let mut f = fixture_with_window();
    f.zen().layout.set_window_floating(None, true);
    f.zen_complete_animations();
    f.zen().layout.set_camera_zoom(0.42);
    f.zen_complete_animations();

    // Far from any window, so the ordinary click handling must still get the event.
    assert!(!f.zen().layout.travel_to_island_at(Point::from((4., 4.))));
}

#[test]
fn travel_does_nothing_when_not_on_the_map() {
    let mut f = fixture_with_window();
    f.zen_complete_animations();
    assert!(!f.zen().layout.is_at_map_zoom());
    assert!(!f.zen().layout.travel_to_island_at(Point::from((640., 360.))));
}

// Zooming has to leave whatever is under the pointer under the pointer.
//
// The bug this catches: the camera pinned the anchor using pan and zoom alone, while
// the render also centres the workspace in the view by an offset that is itself a
// function of zoom. The point under the cursor therefore slid by half the view's change
// in size, and the zoom looked like it came from the middle of the screen.
#[test]
fn zooming_keeps_the_point_under_the_cursor_still() {
    let mut f = fixture_with_window();
    f.zen().layout.set_window_floating(None, true);
    f.zen_complete_animations();

    // Deliberately off centre: an anchor in the middle of the screen cannot tell a
    // correct pinning from one that always zooms about the middle.
    let anchor = Point::from((OUTPUT_W as f64 * 0.22, OUTPUT_H as f64 * 0.78));

    let before = f
        .zen()
        .layout
        .active_monitor_ref()
        .unwrap()
        .view_to_workspace(anchor);

    for _ in 0..4 {
        f.zen().layout.camera_zoom_step(true, anchor);
        f.zen_complete_animations();
    }

    let mon = f.zen().layout.active_monitor_ref().unwrap();
    assert!(mon.camera_zoom() > 1.2, "the zoom did not actually change");

    let after = mon.view_to_workspace(anchor);
    assert!(
        (after.x - before.x).abs() < 1. && (after.y - before.y).abs() < 1.,
        "the canvas drifted under the cursor: {before:?} -> {after:?}"
    );
}

#[test]
fn zooming_back_out_lands_where_it_started() {
    let mut f = fixture_with_window();
    f.zen().layout.set_window_floating(None, true);
    f.zen_complete_animations();

    let anchor = Point::from((OUTPUT_W as f64 * 0.8, OUTPUT_H as f64 * 0.3));
    let before = f
        .zen()
        .layout
        .active_monitor_ref()
        .unwrap()
        .view_to_workspace(anchor);

    for _ in 0..3 {
        f.zen().layout.camera_zoom_step(true, anchor);
        f.zen_complete_animations();
    }
    for _ in 0..3 {
        f.zen().layout.camera_zoom_step(false, anchor);
        f.zen_complete_animations();
    }

    let after = f
        .zen()
        .layout
        .active_monitor_ref()
        .unwrap()
        .view_to_workspace(anchor);
    assert!(
        (after.x - before.x).abs() < 1. && (after.y - before.y).abs() < 1.,
        "in and back out did not return: {before:?} -> {after:?}"
    );
}
