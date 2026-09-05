use std::fmt::Write as _;
use std::time::Duration;

use insta::assert_snapshot;
use zen_config::animations::{Curve, EasingParams, Kind};
use zen_config::Config;
use zen_ipc::SizeChange;
use smithay::utils::{Point, Size};
use wayland_client::protocol::wl_surface::WlSurface;

use super::client::ClientId;
use super::*;
use crate::state::Zen;

fn format_tiles(zen: &Zen) -> String {
    let mut buf = String::new();
    let ws = zen.layout.active_workspace().unwrap();
    let mut tiles: Vec<_> = ws.tiles_with_render_positions().collect();

    tiles.sort_by_key(|(tile, _, _)| tile.window().id().get());
    for (tile, pos, _visible) in tiles {
        let Size { w, h, .. } = tile.animated_tile_size();
        let Point { x, y, .. } = pos;
        writeln!(&mut buf, "{w:>3.0} × {h:>3.0} at x:{x:>3.0} y:{y:>3.0}").unwrap();
    }
    buf
}

fn create_window(f: &mut Fixture, id: ClientId, w: u16, h: u16) -> WlSurface {
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);

    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(w, h);
    window.ack_last_and_commit();
    f.roundtrip(id);

    surface
}

fn set_time(zen: &mut Zen, time: Duration) {
    let now = zen.clock.now();
    zen.clock.set_unadjusted(now);
    let _ = zen.clock.now();
    zen.clock.set_unadjusted(Duration::ZERO);
    zen.clock.set_rate(1.0);
    let _ = zen.clock.now();

    zen.clock.set_unadjusted(time);
    let _ = zen.clock.now();

    zen.clock.set_rate(0.0);
}

fn set_up() -> Fixture {
    const LINEAR: Kind = Kind::Easing(EasingParams {
        duration_ms: 1000,
        curve: Curve::Linear,
    });

    let mut config = Config::default();
    config.layout.gaps = 0.0;
    config.animations.window_resize.anim.kind = LINEAR;
    config.animations.window_movement.0.kind = LINEAR;

    let mut f = Fixture::with_config(config);
    f.zen_state().backend.headless().add_renderer().unwrap();
    f.add_output(1, (1920, 1080));

    f
}

fn set_up_two_in_column() -> (Fixture, ClientId, WlSurface, WlSurface) {
    let mut f = set_up();

    let id = f.add_client();

    let surface1 = create_window(&mut f, id, 100, 100);
    let surface2 = create_window(&mut f, id, 200, 200);
    f.double_roundtrip(id);

    let _ = f.client(id).window(&surface1).recent_configures();
    let _ = f.client(id).window(&surface2).recent_configures();

    f.zen().layout.focus_left();
    f.zen().layout.consume_into_column();
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface1);
    window.ack_last_and_commit();

    let window = f.client(id).window(&surface2);
    window.ack_last_and_commit();

    f.double_roundtrip(id);

    set_time(f.zen(), Duration::ZERO);
    f.zen_complete_animations();

    (f, id, surface1, surface2)
}

#[test]
fn egl_height_resize_animates_next_y() {
    let (mut f, id, surface1, surface2) = set_up_two_in_column();

    f.zen()
        .layout
        .set_window_height(None, SizeChange::AdjustFixed(-50));
    f.double_roundtrip(id);

    let window = f.client(id).window(&surface1);
    window.set_size(100, 50);
    window.ack_last_and_commit();
    let window = f.client(id).window(&surface2);
    window.ack_last_and_commit();

    f.roundtrip(id);

    assert_snapshot!(format_tiles(f.zen()), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");

    set_time(f.zen(), Duration::from_millis(500));
    f.zen().advance_animations();

    assert_snapshot!(format_tiles(f.zen()), @r"
    100 ×  75 at x:  0 y:  0
    200 × 200 at x:  0 y: 75
    ");

    set_time(f.zen(), Duration::from_millis(1000));
    f.zen().advance_animations();

    assert_snapshot!(format_tiles(f.zen()), @r"
    100 ×  50 at x:  0 y:  0
    200 × 200 at x:  0 y: 50
    ");
}

#[test]
fn egl_clientside_height_change_doesnt_animate() {
    let (mut f, id, surface1, _surface2) = set_up_two_in_column();

    assert_snapshot!(format_tiles(f.zen()), @r"
    100 × 100 at x:  0 y:  0
    200 × 200 at x:  0 y:100
    ");

    let window = f.client(id).window(&surface1);
    window.set_size(100, 50);
    window.commit();

    f.roundtrip(id);

    assert_snapshot!(format_tiles(f.zen()), @r"
    100 ×  50 at x:  0 y:  0
    200 × 200 at x:  0 y: 50
    ");
}
