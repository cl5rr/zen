use crate::tests::fixture::Fixture;

#[test]
fn create_then_destroy() {
    let mut f = Fixture::new();
    f.add_output(1, (1920, 1080));

    let zen = f.zen();
    assert!(zen.create_virtual_output("stream", 1280, 720, 60_000).is_ok());
    assert!(zen.is_virtual_output(
        &zen.global_space
            .outputs()
            .find(|o| o.name() == "stream")
            .cloned()
            .expect("the virtual output should exist")
    ));

    let zen = f.zen();
    assert!(zen.destroy_virtual_output("stream").is_ok());
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
    assert!(f.zen().create_virtual_output(&existing, 800, 600, 60_000).is_err());
    assert!(f.zen().create_virtual_output("ok-name", 0, 600, 60_000).is_err());
    assert!(f.zen().destroy_virtual_output("never-made").is_err());
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
    f.zen().create_virtual_output("stream", 1280, 720, 60_000).unwrap();

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
