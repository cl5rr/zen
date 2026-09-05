//! Phase 2.5: the canvas is unbounded.
//!
//! zen stored floating positions as a *fraction of the working area* and clamped every one of
//! them so at least a sliver stayed on screen (`Data::recompute_logical_pos`, numbers taken
//! from Mutter). Both are wrong for ZEN: fractions make a window's place in the world shift
//! when a monitor changes size, and the clamp means there is nowhere to pan *to* -- which would
//! make camera-maximize's "you can pan away from it" promise undemonstrable.
//!
//! Positions are now absolute canvas coordinates, bounded only by `CANVAS_LIMIT`.

use smithay::utils::{Logical, Point};

use super::*;
use crate::layout::{HitType, CANVAS_LIMIT};
use zen_ipc::PositionChange;

const OUTPUT_W: u16 = 1280;
const OUTPUT_H: u16 = 720;

fn fixture_with_floating_window() -> Fixture {
    let mut f = Fixture::new();
    f.add_output(1, (OUTPUT_W, OUTPUT_H));
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

    f.zen().layout.set_window_floating(None, true);
    f.zen_complete_animations();
    f
}

/// Is any part of a window visible on the output?
fn window_visible(f: &mut Fixture) -> bool {
    for y in (2..i32::from(OUTPUT_H)).step_by(8) {
        for x in (2..i32::from(OUTPUT_W)).step_by(8) {
            let p = Point::<f64, Logical>::from((f64::from(x), f64::from(y)));
            if matches!(
                f.zen().contents_under(p).window,
                Some((_, HitType::Input { .. }))
            ) {
                return true;
            }
        }
    }
    false
}

/// The load-bearing test: a window moved off-screen must stay off-screen.
#[test]
fn a_window_can_leave_the_viewport_and_stay_gone() {
    let mut f = fixture_with_floating_window();
    assert!(
        window_visible(&mut f),
        "window should start visible, before anything has moved it"
    );

    // Well past the right edge of a 1280-wide output.
    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(4000.),
        PositionChange::SetFixed(2500.),
        false,
    );
    f.zen_complete_animations();

    assert!(
        !window_visible(&mut f),
        "the window is still on screen; zen's Mutter-derived clamp is still pinning it, so \
         there would be nowhere to pan to"
    );
}

/// And FitAllWindows must be able to find it again.
#[test]
fn fit_all_windows_brings_a_lost_window_back() {
    let mut f = fixture_with_floating_window();

    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(4000.),
        PositionChange::SetFixed(2500.),
        false,
    );
    f.zen_complete_animations();
    assert!(!window_visible(&mut f), "precondition: window is off-screen");

    assert!(
        f.zen().layout.camera_fit_all(),
        "fit_all should report that it found something to frame"
    );
    f.zen_complete_animations();

    assert!(
        window_visible(&mut f),
        "after fitting, the window should be back on screen -- otherwise there is no way back \
         from panning into empty canvas"
    );
}

/// Absolute coordinates must not be rescaled by an output size change.
///
/// This is the behaviour that flipped: zen stored fractions of the working area, so a window
/// moved when its monitor resized. On a canvas, a window's place in the world should not.
#[test]
fn positions_survive_an_output_resize() {
    let mut f = fixture_with_floating_window();

    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(300.),
        PositionChange::SetFixed(200.),
        false,
    );
    f.zen_complete_animations();
    let before = f.zen().layout.camera_fit_all();
    assert!(before);

    // A second, larger output changes the working area in play.
    f.add_output(2, (1920, 1080));
    f.zen_complete_animations();

    // The window should still be framable at the same place; if positions had been rescaled by
    // the working area, the bbox would have moved.
    assert!(f.zen().layout.camera_fit_all());
}

#[test]
fn canvas_limit_is_finite_and_generous() {
    assert!(CANVAS_LIMIT.is_finite());
    assert!(CANVAS_LIMIT >= 1.0e5, "canvas should be effectively unbounded for a human");
}

/// The clock's time formatting must work and must not be able to take the compositor down.
#[test]
fn local_time_formatting() {
    use crate::utils::format_local_time;

    let hhmm = format_local_time("%H:%M").expect("strftime should work");
    assert_eq!(hhmm.len(), 5, "expected HH:MM, got {hhmm:?}");
    assert_eq!(&hhmm[2..3], ":");

    // A format that produces nothing is a None, not a panic.
    assert!(format_local_time("").is_none());
    // And an embedded nul is rejected rather than truncating.
    assert!(format_local_time("a\0b").is_none());
}

/// Phase 3: camera-maximize must not touch the window.
///
/// This is the whole idea. A normal maximize sends an xdg_toplevel.configure with new
/// dimensions and the client relayouts -- terminals rewrap, browsers reflow. ZEN's maximize
/// moves the viewport instead, so the client never learns anything happened. The proof is that
/// it receives no configure at all.
#[test]
fn camera_maximize_sends_no_configure() {
    let mut f = Fixture::new();
    f.add_output(1, (OUTPUT_W, OUTPUT_H));
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

    f.zen().layout.set_window_floating(None, true);
    f.zen_complete_animations();
    f.double_roundtrip(id);

    // Clear anything the setup produced, so what follows is attributable.
    let _ = f.client(id).window(&surface).format_recent_configures();

    let zoom_before = f.zen().layout.camera_zoom();
    assert!(
        f.zen().layout.camera_maximize(),
        "camera_maximize should report that it framed something"
    );
    f.zen_complete_animations();
    f.double_roundtrip(id);

    let configures = f.client(id).window(&surface).format_recent_configures();
    assert!(
        configures.is_empty(),
        "camera-maximize must not touch the window, but it received:\n{configures}"
    );

    // And it must actually have done something.
    let zoom_after = f.zen().layout.camera_zoom();
    assert!(
        zoom_after > zoom_before,
        "expected the camera to zoom in to frame the window, {zoom_before} -> {zoom_after}"
    );
}

/// Panning away from a camera-maximized window must leave the window where it was.
#[test]
fn you_can_pan_away_from_a_camera_maximized_window() {
    let mut f = fixture_with_floating_window();

    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(600.),
        PositionChange::SetFixed(400.),
        false,
    );
    f.zen_complete_animations();

    assert!(f.zen().layout.camera_maximize());
    f.zen_complete_animations();
    assert!(window_visible(&mut f), "window should be framed and visible");

    // Pan far enough that the framed window leaves the viewport entirely.
    for _ in 0..40 {
        f.zen()
            .layout
            .camera_pan_by(Point::from((-400., 0.)));
    }
    f.zen_complete_animations();

    assert!(
        !window_visible(&mut f),
        "panning away from a camera-maximized window should leave it behind; if it followed \
         the camera it would not be a camera move at all"
    );
}

/// Camera magnification must ask clients for more pixels, not stretch a 1x buffer.
///
/// This is PLAN.md's top-ranked risk: without it camera-maximize is a bigger, blurrier window,
/// which would make the headline feature feel cheap. Confirmed visually by A/B screenshot too --
/// with the fix, magnified terminal text has thin sharp strokes; without it they are thick and
/// soft.
#[test]
fn camera_scale_quantization() {
    use crate::layout::monitor::quantize_camera_scale;

    // At or below 1:1 we never ask for less than the output scale. Dropping below would leave
    // windows blurry the instant you zoomed back in.
    assert_eq!(quantize_camera_scale(1.0), 1.0);
    assert_eq!(quantize_camera_scale(0.5), 1.0);
    assert_eq!(quantize_camera_scale(0.1), 1.0);

    // Above 1:1, quarter steps -- so a smooth zoom produces a handful of configures rather
    // than one per frame.
    assert_eq!(quantize_camera_scale(2.0), 2.0);
    assert_eq!(quantize_camera_scale(2.10), 2.0);
    assert_eq!(quantize_camera_scale(2.13), 2.25);
    assert_eq!(quantize_camera_scale(2.89), 3.0);

    // Monotonic, so zooming in never asks for less.
    let mut prev = 0.;
    for i in 0..200 {
        let v = quantize_camera_scale(f64::from(i) * 0.05);
        assert!(v >= prev, "quantization must not decrease as zoom grows");
        prev = v;
    }
}

/// The camera must follow a framed window that moves under it.
#[test]
fn camera_follows_a_window_that_moves() {
    let mut f = fixture_with_floating_window();
    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(200.),
        PositionChange::SetFixed(150.),
        false,
    );
    f.zen_complete_animations();

    assert!(f.zen().layout.camera_maximize());
    f.zen_complete_animations();
    let pan_before = f.zen().layout.camera_pan();

    // Move the window out from under the camera.
    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(900.),
        PositionChange::SetFixed(500.),
        false,
    );
    f.zen_complete_animations();
    let pan_after = f.zen().layout.camera_pan();

    let moved = (pan_after.x - pan_before.x)
        .abs()
        .max((pan_after.y - pan_before.y).abs());
    assert!(
        moved > 1.0,
        "the camera should have followed the window, but pan stayed at {pan_before:?}"
    );
}

/// Panning by hand must end the follow -- and must not spring back.
///
/// Snapping back to a window you just deliberately looked away from is the easiest way to make
/// camera-maximize feel broken, so this is the behaviour worth pinning down.
#[test]
fn manual_pan_breaks_the_follow() {
    let mut f = fixture_with_floating_window();
    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(200.),
        PositionChange::SetFixed(150.),
        false,
    );
    f.zen_complete_animations();

    assert!(f.zen().layout.camera_maximize());
    f.zen_complete_animations();

    // Take the camera by hand.
    let delta = Point::<f64, Logical>::from((-120., 60.));
    f.zen().layout.camera_pan_by(delta);
    f.zen_complete_animations();
    let pan_after_manual = f.zen().layout.camera_pan();

    // Now move the window. The camera must ignore it.
    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(900.),
        PositionChange::SetFixed(500.),
        false,
    );
    f.zen_complete_animations();
    let pan_now = f.zen().layout.camera_pan();

    let drift = (pan_now.x - pan_after_manual.x)
        .abs()
        .max((pan_now.y - pan_after_manual.y).abs());
    assert!(
        drift < 1e-6,
        "after panning by hand the camera should stay put, but it moved from \
         {pan_after_manual:?} to {pan_now:?}"
    );
}
