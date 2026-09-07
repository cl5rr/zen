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

    // What a rebuild from the connectors alone produces: no virtual output in sight.
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
    // The merge itself is tested above. This is about it being called: the tty backend
    // cannot be driven from a headless fixture, so nothing else in the suite would
    // notice the call going missing, and the symptom on a real machine is a virtual
    // output quietly disappearing from `zen msg outputs`.
    let src = include_str!("../backend/tty.rs");
    let replace = src
        .find("*guard = ipc_outputs;")
        .expect("refresh_ipc_outputs no longer replaces the map; check this test");

    assert!(
        src[..replace].contains("keep_virtual_outputs(&guard, &mut ipc_outputs);"),
        "the map is replaced without putting the virtual outputs back into it"
    );
}
