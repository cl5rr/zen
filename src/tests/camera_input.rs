//! Phase 1: pointer input delivery under camera zoom.
//!
//! zen only ever zoomed as a transient overview mode, and deliberately refused to send pointer
//! events to a scaled window: `Monitor::window_under` called `HitType::to_activate()`, which
//! throws away the surface-local position, so a zoomed window could be clicked to focus but
//! never actually received pointer events. ZEN's camera-maximize needs the opposite.
//!
//! The property under test is scale-independent and non-circular:
//!
//! > Moving the pointer N pixels across the screen must move the coordinate delivered to the
//! > client by N/zoom pixels.
//!
//! If that holds, the client's idea of where the cursor is agrees with the user's, at any zoom.
//!
//! Note on range: these tests only exercise modest magnification. zen's overview render
//! geometry assumes zoom <= 1, and above roughly 1.25 it pushes the workspace off-screen
//! entirely (the window's origin goes negative). That is a framing problem for the real
//! per-output Camera in Phase 2, not an input problem -- the input math below is scale-generic.

use smithay::utils::{Logical, Point};

use super::*;
use crate::layout::HitType;

const OUTPUT_W: i32 = 1280;
const OUTPUT_H: i32 = 720;

/// Camera zoom levels to test. Above 1.0 magnifies.
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

/// The surface-local coordinate the client would receive for a pointer at `probe`.
///
/// Smithay delivers `event.location - surface_pos`, so this reproduces exactly what the client
/// sees, rather than trusting the formula the compositor used to get there.
fn delivered_at(f: &mut Fixture, probe: Point<f64, Logical>) -> Option<Point<f64, Logical>> {
    let (_, surface_pos) = f.zen().contents_under(probe).surface?;
    Some(probe - surface_pos)
}

/// On-screen bounding box of the window's input region, found by scanning.
///
/// The window's position depends on layout and camera, so the tests locate it rather than
/// assuming. Returns `None` if nothing on the output is hittable.
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

/// The load-bearing test: pointer motion must scale correctly on its way to the client.
#[test]
fn pointer_motion_scales_into_surface_coordinates() {
    let mut f = fixture_with_window();

    for requested in ZOOM_STEPS {
        let zoom = set_zoom(&mut f, requested);

        let (min, max) = window_bbox(&mut f)
            .unwrap_or_else(|| panic!("no Input hit anywhere at zoom {zoom}"));

        // Two points comfortably inside the window's input region.
        let p1 = Point::<f64, Logical>::from((min.x + 8., min.y + 8.));
        let step = Point::<f64, Logical>::from(((max.x - min.x) / 2., (max.y - min.y) / 3.));
        let p2 = p1 + step;

        let d1 = delivered_at(&mut f, p1)
            .unwrap_or_else(|| panic!("no surface under {p1:?} at zoom {zoom}"));
        let d2 = delivered_at(&mut f, p2)
            .unwrap_or_else(|| panic!("no surface under {p2:?} at zoom {zoom}"));

        // Moving `step` across the screen must move the delivered coordinate by `step / zoom`.
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

/// At zoom 1.0 the delivered coordinate must be exactly what it was before Phase 1.
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

/// Interactive resize must be reachable while zoomed; zen returned `None` outright.
#[test]
fn resize_edges_are_reachable_when_zoomed() {
    let mut f = fixture_with_window();
    let output = f.zen_output(1);

    for requested in ZOOM_STEPS {
        let zoom = set_zoom(&mut f, requested);
        let (min, _) = window_bbox(&mut f)
            .unwrap_or_else(|| panic!("no Input hit anywhere at zoom {zoom}"));
        // Only asserting the lookup is attempted rather than short-circuited to None the
        // moment any zoom is active.
        let _ = f.zen().layout.resize_edges_under(&output, min);
    }
}

/// Panning must move content on screen by exactly the requested view-space delta.
///
/// This goes through `workspaces_render_geo`, the single place render geometry is computed, so
/// it also pins the property that hit-testing and rendering agree about where things are.
///
/// Measured at zoom 1.0 only: past that the workspace overflows the output, so the scanned
/// bounding box is clamped by the screen edge rather than describing the window, and a clipped
/// edge does not move when you pan.
#[test]
fn pan_moves_content_by_the_view_delta() {
    let mut f = fixture_with_window();
    set_zoom(&mut f, 1.0);

    let (before_min, _) = window_bbox(&mut f).expect("no window found");

    let delta = Point::<f64, Logical>::from((40., 24.));
    f.zen().layout.camera_pan_by(delta);

    let (after_min, _) = window_bbox(&mut f).expect("window vanished after panning");

    let moved = after_min - before_min;
    // The scan step is 4px, so allow that much slack in locating the edge.
    let err = (moved.x - delta.x).abs().max((moved.y - delta.y).abs());
    assert!(
        err <= 4.0,
        "panned by {delta:?} but content moved {moved:?}"
    );
}

/// Pan must not disturb the coordinates delivered to the client.
///
/// The window moves on screen, but the same physical point *on the window* must still map to
/// the same surface-local coordinate -- otherwise clicks would drift as you pan.
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
        // Probe the centre, which stays on the window even when the edges are clipped.
        let probe = Point::<f64, Logical>::from(((min.x + max.x) / 2., (min.y + max.y) / 2.));

        let before = delivered_at(&mut f, probe)
            .unwrap_or_else(|| panic!("no surface under {probe:?} at zoom {zoom}"));

        let delta = Point::<f64, Logical>::from((-24., 12.));
        f.zen().layout.camera_pan_by(delta);

        let after = delivered_at(&mut f, probe + delta)
            .unwrap_or_else(|| panic!("no surface after panning at zoom {zoom}"));

        let err = (after.x - before.x).abs().max((after.y - before.y).abs());
        assert!(
            err < 1e-6,
            "at zoom {zoom}: panning changed the client's coordinate from {before:?} to              {after:?}; clicks would drift as you pan"
        );
    }
}
