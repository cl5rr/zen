use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned()
}

fn config() -> String {
    let path = root().join("resources/default-config.kdl");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn action_for(chord: &str) -> Option<String> {
    let text = config();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        let Some(rest) = line.strip_prefix(chord) else {
            continue;
        };
        if !rest.starts_with(' ') && !rest.starts_with('{') {
            continue;
        }
        let Some(body) = rest.split_once('{') else {
            continue;
        };
        let action = body.1.trim().trim_end_matches('}').trim().trim_end_matches(';');
        return Some(action.trim().to_owned());
    }
    None
}

#[test]
fn fullscreen_is_actually_fullscreen() {
    assert_eq!(
        action_for("Mod+F").as_deref(),
        Some("fullscreen-window"),
        "Mod+F must resize the client, not move the camera"
    );
}

#[test]
fn the_camera_move_and_the_resize_are_different_binds() {
    let m = action_for("Mod+M");
    let shift_m = action_for("Mod+Shift+M");

    assert_eq!(m.as_deref(), Some("camera-maximize"));
    assert_eq!(shift_m.as_deref(), Some("maximize-window-to-view"));
    assert_ne!(m, shift_m, "two binds doing the same thing wastes one");
}

#[test]
fn every_shipped_bind_parses() {
    zen_config::Config::load_default();
}
