use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned()
}

fn read(rel: &str) -> String {
    let path = root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn checked_commands() -> BTreeSet<String> {
    let manifest = read("resources/packages.list");
    let mut out = BTreeSet::new();

    for line in manifest.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').map(str::trim).collect();
        let Some(id) = fields.first() else {
            continue;
        };
        let probe = fields.get(3).copied().filter(|p| !p.is_empty()).unwrap_or(id);
        for token in probe.split(',').map(str::trim) {
            if !token.is_empty() && !token.starts_with('/') && !token.starts_with('~') && !token.contains(':') {
                out.insert(token.to_owned());
            }
        }
    }

    out
}

fn spawned_by_config() -> BTreeSet<String> {
    let config = read("resources/default-config.kdl");
    let mut out = BTreeSet::new();

    for line in config.lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        let Some(rest) = line
            .strip_prefix("spawn-at-startup ")
            .or_else(|| line.split_once("spawn ").map(|(_, r)| r))
        else {
            continue;
        };
        let Some(first) = rest.split('"').nth(1) else {
            continue;
        };
        if !first.is_empty() {
            out.insert(first.to_owned());
        }
    }

    out
}

fn clicked_by_bar() -> BTreeSet<String> {
    let bar = read("resources/waybar/config.jsonc");
    let mut out = BTreeSet::new();

    for line in bar.lines() {
        let line = line.trim();
        if !line.starts_with("\"on-click") && !line.starts_with("\"on-scroll") {
            continue;
        }
        let Some(command) = line.split(':').nth(1) else {
            continue;
        };
        let command = command.trim().trim_start_matches('"');
        if let Some(first) = command.split_whitespace().next() {
            let first = first.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
            if !first.is_empty() && first.chars().any(|c| c.is_ascii_alphabetic()) {
                out.insert(first.to_owned());
            }
        }
    }

    out
}

fn ours_or_builtin(command: &str) -> bool {
    command.starts_with("zen")
        || matches!(command, "sh" | "systemctl" | "loginctl" | "pkill" | "notify-send")
}

#[test]
fn every_tool_the_shipped_setup_uses_is_one_the_installer_checks() {
    let checked = checked_commands();
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    wanted.extend(spawned_by_config());
    wanted.extend(clicked_by_bar());

    let missing: Vec<&String> = wanted
        .iter()
        .filter(|c| !ours_or_builtin(c) && !checked.contains(*c))
        .collect();

    assert!(
        missing.is_empty(),
        "the shipped config or bar runs these, but setup.sh never checks for them, so \
         nobody is ever told to install them: {missing:?}\n\
         add a row for each to resources/packages.list"
    );
}

#[test]
fn the_optional_renderers_are_offered_too() {
    let checked = checked_commands();
    for tool in ["awww", "swww", "mpvpaper"] {
        assert!(
            checked.contains(tool),
            "zen-wallpaper prefers {tool} but resources/packages.list never probes for \
             it, so it is never installed and the feature silently never happens"
        );
    }
}
