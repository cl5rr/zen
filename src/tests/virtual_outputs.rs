use crate::tests::fixture::Fixture;

#[test]
fn create_then_destroy() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));

    assert!(f
        .zen_state()
        .create_virtual_output("stream", 1280, 720, 60_000)
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
    assert!(f.zen_state().create_virtual_output(&existing, 800, 600, 60_000).is_err());
    assert!(f.zen_state().create_virtual_output("ok-name", 0, 600, 60_000).is_err());
    assert!(f.zen_state().destroy_virtual_output("never-made").is_err());
}

// The freeze, and the limits of testing it here.
//
// redraw_queued_outputs is a `while let` over every output still marked Queued, so an
// output whose redraw state is never cleared does not merely spin: that loop never
// returns, inside one dispatch, and the session stops. A virtual output has no vblank
// to clear it, so it clears its own, and the loop now carries a bound as well.
//
// This checks the loop terminates and leaves the output idle. It does NOT prove the
// pacing itself, because the fixture has no GPU: the render returns Skipped and the
// existing Skipped branch clears the state for its own reasons. Verified by A/B, which
// passed with the fix removed.
#[test]
fn redrawing_a_virtual_output_terminates() {
    use crate::state::RedrawState;

    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state().create_virtual_output("stream", 1280, 720, 60_000).unwrap();

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

// The bug this catches: a virtual output was added to the layout but never to the
// backend's IPC output map, so it was a real monitor to clients and to the compositor
// while `zen msg outputs` denied it existed. Everything that lists monitors reads that
// map, so the settings app could create one and then never show it.
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
        .create_virtual_output("stream", 1280, 720, 60_000)
        .unwrap();
    assert!(listed(&mut f), "created but absent from zen msg outputs");

    f.zen_state().destroy_virtual_output("stream").unwrap();
    assert!(!listed(&mut f), "destroyed but still listed");
}

// The settings page tells a virtual output from a real one by these two fields, so they
// are load-bearing rather than decoration.
#[test]
fn a_virtual_output_identifies_itself_as_one() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));
    f.zen_state()
        .create_virtual_output("stream", 800, 600, 60_000)
        .unwrap();

    let outputs = f.zen_state().backend.ipc_outputs();
    let outputs = outputs.lock().unwrap();
    let entry = outputs.values().find(|o| o.name == "stream").unwrap();
    assert_eq!(entry.make, "ZEN");
    assert_eq!(entry.model, "Virtual");
    assert_eq!(entry.modes[0].width, 800);
    assert_eq!(entry.modes[0].height, 600);
}
