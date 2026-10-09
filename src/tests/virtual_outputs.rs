use crate::tests::fixture::Fixture;

#[test]
fn create_then_destroy() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));

    assert!(f
        .zen_state()
        .create_virtual_output("stream", 1280, 720, 60_000, false)
        .is_ok());

    let zen = f.zen();
    assert!(zen.is_virtual_output(
        &zen.global_space
            .outputs()
            .find(|o| o.name() == "stream")
            .cloned()
            .expect("the virtual output should exist")
    ));

    assert!(f.zen_state().destroy_virtual_output("stream").is_ok());
    let zen = f.zen();
    assert!(
        !zen.global_space.outputs().any(|o| o.name() == "stream"),
        "destroying it should take the output away too"
    );
}

#[test]
fn a_name_that_is_taken_is_refused() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));

    let existing = f.zen().global_space.outputs().next().unwrap().name();
    assert!(f.zen_state().create_virtual_output(&existing, 800, 600, 60_000, false).is_err());
    assert!(f.zen_state().create_virtual_output("ok-name", 0, 600, 60_000, false).is_err());
    assert!(f.zen_state().destroy_virtual_output("never-made").is_err());
}

#[test]
fn redrawing_a_virtual_output_terminates() {
    use crate::state::RedrawState;

    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state().create_virtual_output("stream", 1280, 720, 60_000, false).unwrap();

    let output = f
        .zen()
        .global_space
        .outputs()
        .find(|o| o.name() == "stream")
        .cloned()
        .unwrap();

    f.zen().queue_redraw(&output);

    let state = f.zen_state();
    state.zen.redraw_queued_outputs(&mut state.backend);

    let redraw_state = &f.zen().output_state.get(&output).unwrap().redraw_state;
    assert!(
        !matches!(redraw_state, RedrawState::Queued),
        "left Queued, redraw_queued_outputs would loop forever"
    );
}

#[test]
fn a_virtual_output_is_listed_over_the_ipc() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));

    let listed = |f: &mut Fixture| -> bool {
        f.zen_state()
            .backend
            .ipc_outputs()
            .lock()
            .unwrap()
            .values()
            .any(|o| o.name == "stream")
    };

    assert!(!listed(&mut f));
    f.zen_state()
        .create_virtual_output("stream", 1280, 720, 60_000, false)
        .unwrap();
    assert!(listed(&mut f), "created but absent from zen msg outputs");

    f.zen_state().destroy_virtual_output("stream").unwrap();
    assert!(!listed(&mut f), "destroyed but still listed");
}

#[test]
fn a_virtual_output_identifies_itself_as_one() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state()
        .create_virtual_output("stream", 800, 600, 60_000, false)
        .unwrap();

    let outputs = f.zen_state().backend.ipc_outputs();
    let outputs = outputs.lock().unwrap();
    let entry = outputs.values().find(|o| o.name == "stream").unwrap();
    assert_eq!(entry.make, "ZEN");
    assert_eq!(entry.model, "Virtual");
    assert_eq!(entry.modes[0].width, 800);
    assert_eq!(entry.modes[0].height, 600);
}

#[test]
fn a_placeable_output_takes_a_position_like_any_other() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state()
        .create_virtual_output("stream", 1280, 720, 60_000, false)
        .unwrap();

    let zen = f.zen();
    let output = zen
        .global_space
        .outputs()
        .find(|o| o.name() == "stream")
        .cloned()
        .expect("a placeable output should be in the global space");

    assert!(!zen.is_hidden_output(&output));
    assert!(
        zen.global_space.output_geometry(&output).is_some(),
        "a placeable output should have somewhere to be"
    );
}

#[test]
fn a_hidden_output_says_so() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state()
        .create_virtual_output("secret", 1280, 720, 60_000, true)
        .unwrap();
    f.zen_state()
        .create_virtual_output("shown", 1280, 720, 60_000, false)
        .unwrap();

    let zen = f.zen();
    let by_name = |name: &str| {
        zen.global_space
            .outputs()
            .find(|o| o.name() == name)
            .cloned()
            .unwrap()
    };

    assert!(zen.is_hidden_output(&by_name("secret")));
    assert!(!zen.is_hidden_output(&by_name("shown")));
}

#[test]
fn destroying_a_hidden_output_clears_the_flag_with_it() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state()
        .create_virtual_output("secret", 800, 600, 60_000, true)
        .unwrap();
    f.zen_state().destroy_virtual_output("secret").unwrap();

    assert!(
        !f.zen().global_space.outputs().any(|o| o.name() == "secret"),
        "the output should be gone"
    );
    f.zen_state()
        .create_virtual_output("secret", 800, 600, 60_000, false)
        .expect("the name should be free again");
    let zen = f.zen();
    let output = zen
        .global_space
        .outputs()
        .find(|o| o.name() == "secret")
        .cloned()
        .unwrap();
    assert!(!zen.is_hidden_output(&output), "the old flag came back");
}

#[test]
fn a_virtual_output_is_castable() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state()
        .create_virtual_output("stream", 1280, 720, 60_000, false)
        .unwrap();

    f.zen_state().refresh_ipc_outputs();

    let outputs = f.zen_state().backend.ipc_outputs();
    let outputs = outputs.lock().unwrap();
    let entry = outputs
        .values()
        .find(|o| o.name == "stream")
        .expect("the virtual output should be listed");

    assert!(
        entry.logical.is_some(),
        "record_monitor refuses an output with no logical geometry as disabled, so a \
         virtual one could never be cast"
    );
}

// preview
#[test]
fn a_virtual_output_can_be_shown_on_a_real_one() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state()
        .create_virtual_output("stream", 1280, 720, 60_000, false)
        .unwrap();

    assert!(
        !f.zen().is_previewed("stream"),
        "a new one must not start drawing over the screen"
    );

    assert!(f.zen_state().set_virtual_output_preview("stream", true).is_ok());
    assert!(f.zen().is_previewed("stream"));

    assert!(f.zen_state().set_virtual_output_preview("stream", false).is_ok());
    assert!(!f.zen().is_previewed("stream"));

    assert!(
        f.zen_state().set_virtual_output_preview("eDP-1", true).is_err(),
        "a real output has nothing to preview"
    );
    assert!(f.zen_state().set_virtual_output_preview("never-made", true).is_err());
}

#[test]
fn the_preview_keeps_its_shape_and_stays_on_screen() {
    use crate::backend::virtual_output::{preview_rect, PREVIEW_MARGIN};
    use smithay::utils::Size;

    let host = Size::from((1920., 1080.));
    let rect = preview_rect(host, Size::from((1280, 720))).expect("16:9 into 16:9");

    assert!(
        (rect.size.w / rect.size.h - 1280. / 720.).abs() < 0.001,
        "a squashed preview is worse than none: {:?}",
        rect.size
    );
    assert!((rect.size.w - 1920. * 0.25).abs() < 0.001);
    assert!((rect.loc.x + rect.size.w - (host.w - PREVIEW_MARGIN)).abs() < 0.001);
    assert!((rect.loc.y + rect.size.h - (host.h - PREVIEW_MARGIN)).abs() < 0.001);
    assert!(rect.loc.x >= 0. && rect.loc.y >= 0.);
}

#[test]
fn a_tall_virtual_output_cannot_take_over_the_screen() {
    use crate::backend::virtual_output::{preview_rect, PREVIEW_MAX_HEIGHT};
    use smithay::utils::Size;

    let host = Size::from((1920., 1080.));
    let rect = preview_rect(host, Size::from((600, 3000))).expect("a very tall one");

    assert!(
        rect.size.h <= host.h * PREVIEW_MAX_HEIGHT + 0.001,
        "height ran away: {}",
        rect.size.h
    );
    assert!(
        (rect.size.w / rect.size.h - 600. / 3000.).abs() < 0.001,
        "capping the height must not stretch it"
    );
    assert!(rect.loc.y >= 0.);
}

#[test]
fn nothing_is_drawn_for_a_size_that_makes_no_sense() {
    use crate::backend::virtual_output::preview_rect;
    use smithay::utils::Size;

    assert!(preview_rect(Size::from((1920., 1080.)), Size::from((0, 720))).is_none());
    assert!(preview_rect(Size::from((1920., 1080.)), Size::from((1280, 0))).is_none());
    assert!(preview_rect(Size::from((0., 0.)), Size::from((1280, 720))).is_none());
}

#[test]
fn a_backend_refresh_does_not_take_the_virtual_outputs_with_it() {
    use crate::backend::{keep_virtual_outputs, IpcOutputMap, OutputId};

    let entry = |name: &str, preview: Option<bool>| zen_ipc::Output {
        name: name.to_owned(),
        make: "ZEN".to_owned(),
        model: "Virtual".to_owned(),
        serial: None,
        physical_size: None,
        modes: Vec::new(),
        current_mode: None,
        is_custom_mode: true,
        vrr_supported: false,
        vrr_enabled: false,
        logical: None,
        max_bpc: None,
        preview,
    };

    let real = OutputId::next();
    let virt = OutputId::next();

    let mut existing = IpcOutputMap::new();
    existing.insert(real, entry("eDP-1", None));
    existing.insert(virt, entry("stream", Some(true)));

    let mut rebuilt = IpcOutputMap::new();
    rebuilt.insert(real, entry("eDP-1", None));

    keep_virtual_outputs(&existing, &mut rebuilt);

    assert!(
        rebuilt.values().any(|o| o.name == "stream"),
        "plugging in a monitor must not delete a virtual one"
    );
    assert_eq!(
        rebuilt.get(&virt).and_then(|o| o.preview),
        Some(true),
        "and it has to keep being previewed"
    );
    assert_eq!(rebuilt.len(), 2);
}

#[test]
fn the_tty_backend_puts_them_back_after_a_refresh() {
    let src = include_str!("../backend/tty.rs");
    let replace = src
        .find("*guard = ipc_outputs;")
        .expect("refresh_ipc_outputs no longer replaces the map; check this test");

    assert!(
        src[..replace].contains("keep_virtual_outputs(&guard, &mut ipc_outputs);"),
        "the map is replaced without putting the virtual outputs back into it"
    );
}

// view
fn pointer_at(f: &mut Fixture) -> smithay::utils::Point<f64, smithay::utils::Logical> {
    f.zen().seat.get_pointer().unwrap().current_location()
}

fn geometry(f: &mut Fixture, name: &str) -> smithay::utils::Rectangle<f64, smithay::utils::Logical> {
    let zen = f.zen();
    let output = zen.global_space.outputs().find(|o| o.name() == name).cloned().unwrap();
    zen.global_space.output_geometry(&output).unwrap().to_f64()
}

#[test]
fn viewing_moves_you_into_the_virtual_monitor_and_back() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state().create_virtual_output("stream", 1280, 720, 60_000, false).unwrap();
    let real = f.zen().global_space.outputs().find(|o| o.name() != "stream").unwrap().name();
    let home = geometry(&mut f, &real).loc + smithay::utils::Point::from((300., 200.));

    f.zen_state().move_cursor(home);
    assert!(f.zen_state().view_output(Some("stream")).is_ok());

    assert_eq!(f.zen().viewer_of("stream"), Some(real.as_str()));
    let inside = geometry(&mut f, "stream");
    assert!(inside.contains(pointer_at(&mut f)), "the pointer should be on the virtual monitor");
    let at = pointer_at(&mut f);
    assert!(f.zen().viewed_rect(at).is_some());

    assert!(f.zen_state().view_output(None).is_ok());
    assert!(f.zen().viewer_of("stream").is_none());
    assert_eq!(pointer_at(&mut f), home, "and it comes back where it was");
}

#[test]
fn cycling_goes_through_every_virtual_monitor_then_home() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state().create_virtual_output("a", 1280, 720, 60_000, false).unwrap();
    f.zen_state().create_virtual_output("b", 800, 600, 60_000, false).unwrap();

    f.zen_state().cycle_view().unwrap();
    assert!(f.zen().viewer_of("a").is_some());
    f.zen_state().cycle_view().unwrap();
    assert!(f.zen().viewer_of("a").is_none());
    assert!(f.zen().viewer_of("b").is_some());
    assert!(geometry(&mut f, "b").contains(pointer_at(&mut f)));
    f.zen_state().cycle_view().unwrap();
    assert!(f.zen().viewing.is_empty(), "the last one hands the screen back");
}

#[test]
fn switching_between_virtual_monitors_still_returns_home() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state().create_virtual_output("a", 1280, 720, 60_000, false).unwrap();
    f.zen_state().create_virtual_output("b", 800, 600, 60_000, false).unwrap();

    let real = f.zen().global_space.outputs().find(|o| !o.name().starts_with(['a', 'b'])).unwrap().name();
    let home = geometry(&mut f, &real).loc + smithay::utils::Point::from((300., 200.));
    f.zen_state().move_cursor(home);
    f.zen_state().view_output(Some("a")).unwrap();
    f.zen_state().view_output(Some("b")).unwrap();
    assert!(f.zen().viewer_of("b").is_some());
    f.zen_state().view_output(Some("b")).unwrap();
    assert!(f.zen().viewing.is_empty(), "asking for the one you are on switches back");
    assert_eq!(pointer_at(&mut f), home);
}

#[test]
fn cycling_with_nothing_to_view_says_so() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    assert!(f.zen_state().cycle_view().is_err());
    assert!(f.zen_state().view_output(Some("never-made")).is_err());
}

#[test]
fn destroying_the_viewed_monitor_gives_the_screen_back() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state().create_virtual_output("stream", 1280, 720, 60_000, false).unwrap();
    f.zen_state().view_output(Some("stream")).unwrap();

    f.zen_state().destroy_virtual_output("stream").unwrap();
    assert!(f.zen().viewing.is_empty());
    let at = pointer_at(&mut f);
    assert!(
        f.zen().global_space.output_under(at).next().is_some(),
        "the pointer was left in a hole: {at:?}"
    );
}

#[test]
fn shares_of_the_real_screen_never_show_the_virtual_one() {
    use crate::backend::virtual_output::shows_view;
    use crate::render_helpers::RenderTarget;

    assert!(shows_view(RenderTarget::Output, false));
    assert!(!shows_view(RenderTarget::Screencast, false));
    assert!(!shows_view(RenderTarget::ScreenCapture, false));
    assert!(!shows_view(RenderTarget::Output, true), "the lock screen always wins");
}

#[test]
fn the_view_keeps_its_shape() {
    use crate::backend::virtual_output::view_rect;
    use smithay::utils::Size;

    let full = view_rect(Size::from((1920., 1080.)), Size::from((1280, 720))).unwrap();
    assert_eq!(full.loc, (0., 0.).into());
    assert_eq!(full.size, (1920., 1080.).into());

    let boxed = view_rect(Size::from((1920., 1080.)), Size::from((1080, 1080))).unwrap();
    assert_eq!(boxed.size, (1080., 1080.).into());
    assert_eq!(boxed.loc, (420., 0.).into());

    assert!(view_rect(Size::from((1920., 1080.)), Size::from((0, 720))).is_none());
}

#[test]
fn the_pointer_stays_inside_while_viewing() {
    use crate::backend::virtual_output::confine;
    use smithay::utils::Rectangle;

    let rect = Rectangle::new((2000., 0.).into(), (1280., 720.).into());
    assert_eq!(confine((10., 10.).into(), rect), (2000., 10.).into());
    assert_eq!(confine((5000., 900.).into(), rect), (3279., 719.).into());
    assert_eq!(confine((2500., 300.).into(), rect), (2500., 300.).into());
}

#[test]
fn an_absolute_pointer_lands_where_you_see_it() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state().create_virtual_output("stream", 1280, 720, 60_000, false).unwrap();
    let real = f.zen().global_space.outputs().find(|o| o.name() != "stream").unwrap().name();
    let screen = geometry(&mut f, &real);
    let virt = geometry(&mut f, "stream");

    use smithay::utils::Point;
    let middle = screen.loc + Point::from((screen.size.w / 2., screen.size.h / 2.));
    assert_eq!(f.zen().through_view(middle), middle, "nothing changes until you switch");

    f.zen_state().view_output(Some("stream")).unwrap();
    let landed = f.zen().through_view(middle);
    assert_eq!(landed, virt.loc + Point::from((virt.size.w / 2., virt.size.h / 2.)));
    let corner = f.zen().through_view(screen.loc);
    assert_eq!(corner, virt.loc);
}

#[test]
fn the_screen_draws_the_view_only_for_itself() {
    let src = include_str!("../state.rs");
    let inner = src.find("fn render_inner<").expect("render_inner moved; check this test");
    let body = &src[inner..inner + 2000];
    assert!(
        body.contains("if shows_view(ctx.target, self.is_locked())"),
        "render_inner draws the view without asking who it is drawing for"
    );
}

#[test]
fn pointer_motion_is_kept_on_the_view() {
    let src = include_str!("../input/mod.rs");
    assert!(src.contains("if let Some(rect) = self.zen.viewed_rect(pos) {"));
    assert!(src.contains("let pos = self.zen.through_view(pos);"));
}
