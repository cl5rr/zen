use client::ClientId;
use insta::assert_snapshot;
use smithay::utils::Point;
use wayland_client::protocol::wl_surface::WlSurface;

use super::*;
use crate::layout::LayoutElement as _;

fn set_up() -> (Fixture, ClientId, WlSurface) {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.add_output(2, (1280, 720));

    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    (f, id, surface)
}

#[test]
fn windowed_fullscreen() {
    let (mut f, id, surface) = set_up();

    let _ = f.client(id).window(&surface).recent_configures();

    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();

    zen.layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 936 × 1048, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    let mapped = f.zen().layout.windows().next().unwrap().1;
    assert!(!mapped.is_windowed_fullscreen());

    let window = f.client(id).window(&surface);
    window.ack_last_and_commit();
    f.roundtrip(id);

    let mapped = f.zen().layout.windows().next().unwrap().1;
    assert!(mapped.is_windowed_fullscreen());

    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 936 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    let mapped = f.zen().layout.windows().next().unwrap().1;
    assert!(mapped.is_windowed_fullscreen());

    let window = f.client(id).window(&surface);
    window.ack_last_and_commit();
    f.roundtrip(id);

    let mapped = f.zen().layout.windows().next().unwrap().1;
    assert!(!mapped.is_windowed_fullscreen());
}

#[test]
fn windowed_fullscreen_chain() {
    let (mut f, id, surface) = set_up();

    let _ = f.client(id).window(&surface).recent_configures();

    let mapped = f.zen().layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();

    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.roundtrip(id);
    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.roundtrip(id);
    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.roundtrip(id);
    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @r"
    size: 936 × 1048, bounds: 1888 × 1048, states: [Activated, Fullscreen]
    size: 936 × 1048, bounds: 1888 × 1048, states: [Activated]
    size: 936 × 1048, bounds: 1888 × 1048, states: [Activated, Fullscreen]
    size: 936 × 1048, bounds: 1888 × 1048, states: [Activated]
    "
    );

    let window = f.client(id).window(&surface);
    let serials = Vec::from_iter(
        window.configures_received[window.configures_received.len() - 4..]
            .iter()
            .map(|(s, _c)| *s),
    );

    let get_state = |f: &mut Fixture| {
        let mapped = f.zen().layout.windows().next().unwrap().1;
        format!(
            "fs {}, wfs {}",
            mapped.sizing_mode().is_fullscreen(),
            mapped.is_windowed_fullscreen()
        )
    };

    let mut states = vec![get_state(&mut f)];
    for serial in serials {
        let window = f.client(id).window(&surface);
        window.xdg_surface.ack_configure(serial);
        window.commit();
        f.roundtrip(id);
        states.push(get_state(&mut f));
    }

    assert_snapshot!(
        states.join("\n"),
        @r"
    fs false, wfs false
    fs false, wfs true
    fs false, wfs false
    fs false, wfs true
    fs false, wfs false
    "
    );
}

#[test]
fn unfullscreen_before_fullscreen_ack_doesnt_prevent_view_offset_save_restore() {
    let (mut f, id, _surface) = set_up();

    let window2 = f.client(id).create_window();
    let surface2 = window2.surface.clone();
    window2.commit();
    f.roundtrip(id);

    let window2 = f.client(id).window(&surface2);
    window2.attach_new_buffer();
    window2.set_size(200, 200);
    window2.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface2).recent_configures();

    let zen = f.zen();
    let mapped2 = zen.layout.windows().last().unwrap().1;
    let window2_id = mapped2.window.clone();

    assert_snapshot!(zen.layout.active_workspace().unwrap().scrolling().view_pos(), @"-16");

    zen.layout.set_fullscreen(&window2_id, true);
    f.double_roundtrip(id);

    f.zen().layout.set_fullscreen(&window2_id, false);

    let window2 = f.client(id).window(&surface2);
    assert_snapshot!(
        window2.format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );
    let (_, configure) = window2.configures_received.last().unwrap();
    window2.set_size(configure.size.0 as u16, configure.size.1 as u16);
    window2.ack_last_and_commit();
    f.double_roundtrip(id);
    f.zen_complete_animations();

    assert_snapshot!(f.zen().layout.active_workspace().unwrap().scrolling().view_pos(), @"116");

    let window2 = f.client(id).window(&surface2);
    assert_snapshot!(
        window2.format_recent_configures(),
        @"size: 936 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );
    window2.set_size(200, 200);
    window2.ack_last_and_commit();
    f.roundtrip(id);
    f.zen_complete_animations();

    assert_snapshot!(f.zen().layout.active_workspace().unwrap().scrolling().view_pos(), @"-16");
}

#[test]
fn interactive_move_unfullscreen_to_scrolling_restores_size() {
    let (mut f, id, surface) = set_up();

    let _ = f.client(id).window(&surface).recent_configures();

    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    zen.layout.set_fullscreen(&window, true);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    let output = f.zen_output(1);
    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    zen.layout
        .interactive_move_begin(window.clone(), &output, Point::default());
    zen.layout.interactive_move_update(
        &window,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn interactive_move_unmaximize_to_scrolling_restores_size() {
    let (mut f, id, surface) = set_up();

    let _ = f.client(id).window(&surface).recent_configures();

    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    zen.layout.set_maximized(&window, true);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Maximized]"
    );

    let output = f.zen_output(1);
    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    zen.layout
        .interactive_move_begin(window.clone(), &output, Point::default());
    zen.layout.interactive_move_update(
        &window,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}
