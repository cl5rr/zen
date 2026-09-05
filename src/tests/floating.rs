use client::ClientId;
use insta::assert_snapshot;
use zen_config::Config;
use zen_ipc::SizeChange;
use smithay::utils::Point;
use wayland_client::protocol::wl_surface::WlSurface;

use super::*;

fn set_up() -> (Fixture, ClientId, WlSurface) {
    set_up_with_config(Config::default())
}

fn set_up_with_config(config: Config) -> (Fixture, ClientId, WlSurface) {
    let mut f = Fixture::with_config(config);
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
fn unfocus_preserves_current_size() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.zen_focus_output(2);

    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: []"
    );

    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen_focus_output(1);

    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    assert_snapshot!(
        window.format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_to_different_size() {
    let (mut f, id, surface) = set_up();
    let _ = f.client(id).window(&surface).recent_configures();

    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    f.zen().layout.toggle_window_floating(None);
    f.zen().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen_focus_output(2);
    f.double_roundtrip(id);
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: []"
    );

    let window = f.client(id).window(&surface);
    window.ack_last();
    f.roundtrip(id);
    f.zen_focus_output(1);
    f.double_roundtrip(id);
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.commit();
    f.double_roundtrip(id);
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    f.zen_focus_output(2);
    f.double_roundtrip(id);
    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: []"
    );
}

#[test]
fn set_window_width_uses_current_height() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.set_column_width(SizeChange::SetFixed(500));

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn set_window_height_uses_current_width() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen()
        .layout
        .set_window_height(None, SizeChange::SetFixed(500));

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 500, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_to_same_size() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.set_column_width(SizeChange::SetFixed(200));

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_to_different_then_same() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.set_column_width(SizeChange::SetFixed(500));

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen().layout.set_column_width(SizeChange::SetFixed(500));

    f.zen_focus_output(2);

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: []"
    );

    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen_focus_output(1);

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn restore_floating_size() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    let (_, configure) = window.configures_received.last().unwrap();
    window.set_size(configure.size.0 as u16, configure.size.1 as u16);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn moving_across_workspaces_doesnt_cancel_resize() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 200, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen().layout.move_to_workspace_down(true);
    f.zen_focus_output(2);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 200, bounds: 1920 × 1080, states: []"
    );

    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen_focus_output(1);
    f.zen().layout.move_to_workspace_down(true);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn moving_to_floating_doesnt_cancel_resize() {
    let (mut f, id, surface) = set_up();
    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn interactive_move_unfullscreen_to_floating_restores_size() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

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
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn interactive_move_unmaximize_to_floating_restores_size() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

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
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_during_interactive_move_propagates_to_floating() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let output = f.zen_output(1);
    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();
    zen.layout
        .interactive_move_begin(window_id.clone(), &output, Point::default());
    zen.layout.interactive_move_update(
        &window_id,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    let window = f.client(id).window(&surface);
    window.set_size(300, 300);
    window.commit();
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    f.zen().layout.interactive_move_end(&window_id);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 300, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn resize_in_steps() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    f.zen().layout.set_column_width(SizeChange::SetFixed(500));
    f.zen()
        .layout
        .set_window_height(None, SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 500, bounds: 1920 × 1080, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    let serial = window.configures_received.last().unwrap().0;

    f.zen().layout.set_column_width(SizeChange::SetFixed(600));
    f.zen_focus_output(2);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 600 × 500, bounds: 1920 × 1080, states: []"
    );

    let window = f.client(id).window(&surface);
    window.xdg_surface.ack_configure(serial);
    window.set_size(500, 500);
    window.commit();

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    f.zen()
        .layout
        .set_window_height(Some(&window), SizeChange::SetFixed(600));
    f.zen_focus_output(1);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 600 × 600, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn state_change_doesnt_break_use_window_size() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    f.client(id).window(&surface).ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.set_column_width(SizeChange::SetFixed(500));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    let serial = window.configures_received.last().unwrap().0;

    f.zen_focus_output(2);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 500 × 100, bounds: 1920 × 1080, states: []"
    );

    let window = f.client(id).window(&surface);
    window.xdg_surface.ack_configure(serial);
    window.set_size(300, 300);
    window.commit();

    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window = mapped.window.clone();
    f.zen()
        .layout
        .set_window_height(Some(&window), SizeChange::SetFixed(600));
    f.zen_focus_output(1);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 600, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn interactive_move_restores_floating_size_when_set_to_floating() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(200, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    let (_, configure) = window.configures_received.last().unwrap();
    window.set_size(configure.size.0 as u16, configure.size.1 as u16);
    window.ack_last_and_commit();
    f.roundtrip(id);

    let output = f.zen_output(1);
    let zen = f.zen();
    let mapped = zen.layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();
    zen.layout
        .interactive_move_begin(window_id.clone(), &output, Point::default());
    zen.layout.interactive_move_update(
        &window_id,
        Point::from((1000., 0.)),
        output,
        Point::default(),
    );
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 200, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen().layout.interactive_move_end(&window_id);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );
}

#[test]
fn floating_doesnt_store_fullscreen_size() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.add_output(2, (1280, 720));

    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.set_fullscreen(None);
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 0 × 0, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 100 × 100, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn floating_doesnt_store_maximized_size() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.add_output(2, (1280, 720));

    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.set_maximized();
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 0 × 0, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1048, bounds: 1888 × 1048, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 100 × 100, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn floating_respects_non_fixed_min_max_rule() {
    let config = r##"
window-rule {
    min-width 200
    max-width 300
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let (mut f, id, surface) = set_up_with_config(config);

    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    let window = f.client(id).window(&surface);
    window.set_size(400, 100);
    window.ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    f.client(id).window(&surface).ack_last_and_commit();
    f.roundtrip(id);

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 300 × 100, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmap_from_floating() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.attach_null();
    window.commit();

    f.double_roundtrip(id);
}

#[test]
fn unfullscreen_to_floating_doesnt_send_extra_configure() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.unset_fullscreen();
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmaximize_to_floating_doesnt_send_extra_configure() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.unset_maximized();
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unfullscreen_to_same_size_floating() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmaximize_to_same_size_floating() {
    let (mut f, id, surface) = set_up();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Maximized]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unfullscreen_to_same_size_windowed_fullscreen_floating() {
    let (mut f, id, surface) = set_up();

    let mapped = f.zen().layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Fullscreen]"
    );
}

#[test]
fn unmaximize_to_same_size_windowed_fullscreen_floating() {
    let (mut f, id, surface) = set_up();

    let mapped = f.zen().layout.windows().next().unwrap().1;
    let window_id = mapped.window.clone();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Maximized]"
    );

    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1888 × 1048, states: [Activated, Fullscreen]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Fullscreen]"
    );

    f.zen().layout.toggle_windowed_fullscreen(&window_id);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unfullscreen_to_same_size_same_bounds_floating() {
    let config = r##"
layout {
    gaps 0
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let (mut f, id, surface) = set_up_with_config(config);

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_fullscreen(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Fullscreen]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn unmaximize_to_same_size_same_bounds_floating() {
    let config = r##"
layout {
    gaps 0
}
"##;
    let config = Config::parse_mem(config).unwrap();
    let (mut f, id, surface) = set_up_with_config(config);

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface);
    window.set_size(1920, 1080);
    window.ack_last_and_commit();
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface).recent_configures();

    let window = f.client(id).window(&surface);
    window.set_maximized();
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated, Maximized]"
    );

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 1920 × 1080, bounds: 1920 × 1080, states: [Activated]"
    );
}

#[test]
fn repeated_size_request() {
    let (mut f, id, surface) = set_up();
    let _ = f.client(id).window(&surface).recent_configures();

    f.zen().layout.toggle_window_floating(None);
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 936 × 1048, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen()
        .layout
        .set_window_width(None, SizeChange::SetFixed(200));
    f.zen()
        .layout
        .set_window_height(None, SizeChange::SetFixed(100));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @"size: 200 × 100, bounds: 1920 × 1080, states: [Activated]"
    );

    f.zen().layout.set_column_width(SizeChange::SetFixed(200));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    let window = f.client(id).window(&surface);
    window.ack_last();
    f.double_roundtrip(id);

    f.zen().layout.set_column_width(SizeChange::SetFixed(200));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );

    let window = f.client(id).window(&surface);
    window.commit();
    f.double_roundtrip(id);

    f.zen().layout.set_column_width(SizeChange::SetFixed(200));
    f.double_roundtrip(id);

    assert_snapshot!(
        f.client(id).window(&surface).format_recent_configures(),
        @""
    );
}
