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

#[test]
fn a_window_can_leave_the_viewport_and_stay_gone() {
    let mut f = fixture_with_floating_window();
    assert!(
        window_visible(&mut f),
        "window should start visible, before anything has moved it"
    );

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

    f.add_output(2, (1920, 1080));
    f.zen_complete_animations();

    assert!(f.zen().layout.camera_fit_all());
}

#[test]
fn canvas_limit_is_finite_and_generous() {
    assert!(CANVAS_LIMIT.is_finite());
    assert!(CANVAS_LIMIT >= 1.0e5, "canvas should be effectively unbounded for a human");
}

#[test]
fn local_time_formatting() {
    use crate::utils::format_local_time;

    let hhmm = format_local_time("%H:%M").expect("strftime should work");
    assert_eq!(hhmm.len(), 5, "expected HH:MM, got {hhmm:?}");
    assert_eq!(&hhmm[2..3], ":");

    assert!(format_local_time("").is_none());
    assert!(format_local_time("a\0b").is_none());
}

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

    let zoom_after = f.zen().layout.camera_zoom();
    assert!(
        zoom_after > zoom_before,
        "expected the camera to zoom in to frame the window, {zoom_before} -> {zoom_after}"
    );
}

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

    f.zen()
        .layout
        .camera_pan_immediate(Point::from((-16000., 0.)));
    f.zen_complete_animations();

    assert!(
        !window_visible(&mut f),
        "panning away from a camera-maximized window should leave it behind; if it followed \
         the camera it would not be a camera move at all"
    );
}

#[test]
fn camera_scale_quantization() {
    use crate::layout::monitor::quantize_camera_scale;

    assert_eq!(quantize_camera_scale(1.0), 1.0);
    assert_eq!(quantize_camera_scale(0.5), 1.0);
    assert_eq!(quantize_camera_scale(0.1), 1.0);

    assert_eq!(quantize_camera_scale(2.0), 2.0);
    assert_eq!(quantize_camera_scale(2.10), 2.0);
    assert_eq!(quantize_camera_scale(2.13), 2.25);
    assert_eq!(quantize_camera_scale(2.89), 3.0);

    let mut prev = 0.;
    for i in 0..200 {
        let v = quantize_camera_scale(f64::from(i) * 0.05);
        assert!(v >= prev, "quantization must not decrease as zoom grows");
        prev = v;
    }
}

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

    let delta = Point::<f64, Logical>::from((-120., 60.));
    f.zen().layout.camera_pan_immediate(delta);
    f.zen_complete_animations();
    let pan_after_manual = f.zen().layout.camera_pan();

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

// fly
fn far_away_window() -> Fixture {
    let mut f = fixture_with_floating_window();
    f.zen().layout.move_floating_window(
        None,
        PositionChange::SetFixed(4000.),
        PositionChange::SetFixed(2500.),
        false,
    );
    f.zen_complete_animations();
    f
}

fn camera_zoom(f: &mut Fixture) -> f64 {
    f.zen().layout.active_monitor_ref().unwrap().camera_zoom()
}

#[test]
fn a_nearby_target_flies_straight() {
    use crate::layout::monitor::fly_needs_arc;
    use smithay::utils::Rectangle;

    let view = Rectangle::<f64, Logical>::new((0., 0.).into(), (1280., 720.).into());
    let near = Rectangle::new((100., 100.).into(), (200., 150.).into());
    let just_off = Rectangle::new((1400., 100.).into(), (200., 150.).into());
    let far = Rectangle::new((4000., 2500.).into(), (200., 150.).into());

    assert!(!fly_needs_arc(view, near), "on screen already");
    assert!(!fly_needs_arc(view, just_off), "a little off screen is a pan, not a flight");
    assert!(fly_needs_arc(view, far), "three screens away needs the overview on the way");
}

#[test]
fn flying_to_a_far_window_rises_first() {
    let mut f = far_away_window();
    assert!(!window_visible(&mut f), "precondition: off screen");

    assert!(f.zen().layout.fly_to_active_window());
    assert!(
        f.zen().layout.active_monitor_ref().unwrap().is_flying(),
        "a far target should go by way of a wider view"
    );

    f.zen_complete_animations();
    assert!(
        camera_zoom(&mut f) < 0.6,
        "the first leg should zoom out to show where it is going, zoom is {}",
        camera_zoom(&mut f)
    );
}

#[test]
fn flying_to_a_far_window_lands_on_it() {
    let mut f = far_away_window();
    assert!(f.zen().layout.fly_to_active_window());

    f.zen_complete_animations();
    f.zen_complete_animations();

    assert!(
        !f.zen().layout.active_monitor_ref().unwrap().is_flying(),
        "the flight never finished its second leg"
    );
    assert!(
        (camera_zoom(&mut f) - 1.).abs() < 0.01,
        "it should land at normal size, zoom is {}",
        camera_zoom(&mut f)
    );
    assert!(window_visible(&mut f), "landed, but the window is not under the camera");
}

#[test]
fn panning_mid_flight_cancels_it() {
    let mut f = far_away_window();
    assert!(f.zen().layout.fly_to_active_window());
    f.zen().layout.camera_pan_by(Point::from((40., 0.)));
    assert!(!f.zen().layout.active_monitor_ref().unwrap().is_flying());
}

#[test]
fn the_canvas_snapshot_lists_islands_and_cameras() {
    let mut f = fixture_with_floating_window();
    let zen = f.zen();
    let layout = &zen.layout;
    let (islands, cameras) = layout.canvas_snapshot(|w| {
        layout
            .windows()
            .find(|(_, m)| &m.window == w)
            .map(|(_, m)| m.id().get())
    });
    let window_id = layout.windows().next().map(|(_, m)| m.id().get()).unwrap();

    assert_eq!(islands.len(), 1, "one window, one island: {islands:?}");
    assert_eq!(islands[0].windows, vec![window_id]);
    assert_eq!(islands[0].active_window_id, Some(window_id));
    assert!(islands[0].rect.width > 0. && islands[0].rect.height > 0.);

    assert_eq!(cameras.len(), 1);
    assert!((cameras[0].zoom - 1.).abs() < 0.001);
    assert!((cameras[0].visible.width - f64::from(OUTPUT_W)).abs() < 1.);
    assert!((cameras[0].visible.height - f64::from(OUTPUT_H)).abs() < 1.);
    assert!(!cameras[0].on_map);
}
