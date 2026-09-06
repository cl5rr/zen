use std::fs;

// The README writes keys the way a person says them. Expand that notation into the
// literal names the config uses, so the check tests truth rather than spelling.
fn expand(key: &str) -> Vec<String> {
    let (prefix, last) = match key.rsplit_once('+') {
        Some((prefix, last)) => (format!("{prefix}+"), last),
        None => (String::new(), key),
    };

    let names: Vec<&str> = match last {
        "HJKL" => vec!["H", "J", "K", "L"],
        "arrows" => vec!["Left", "Right", "Up", "Down"],
        "±" => vec!["Equal", "Minus"],
        "," => vec!["Comma"],
        "." => vec!["Period"],
        "/" => vec!["Slash"],
        "[" => vec!["BracketLeft"],
        "]" => vec!["BracketRight"],
        "=" => vec!["Equal"],
        "-" => vec!["Minus"],
        "\\" => vec!["Backslash"],
        other => vec![other],
    };

    names.into_iter().map(|n| format!("{prefix}{n}")).collect()
}

fn documented() -> Vec<String> {
    let readme = fs::read_to_string("../README.md").expect("README.md should be readable");

    let mut out = Vec::new();
    for span in readme.split('`').skip(1).step_by(2) {
        let span = span.trim();
        let lower = span.to_ascii_lowercase();

        // Mouse and wheel gestures are not keybinds; they live in the input code.
        if lower.contains("drag") || lower.contains("wheel") || lower.contains("scroll") {
            continue;
        }
        if !(span.starts_with("Mod+") || span.starts_with("Super+")) {
            continue;
        }
        out.extend(expand(span));
    }
    out.sort();
    out.dedup();
    out
}

fn bound_in_config(config: &str, key: &str) -> bool {
    config.lines().any(|line| {
        let line = line.trim();
        if line.starts_with("//") || line.starts_with("/-") {
            return false;
        }
        match line.strip_prefix(key) {
            Some(rest) => rest.starts_with(char::is_whitespace) || rest.starts_with('{'),
            None => false,
        }
    })
}

#[test]
fn every_keybind_in_the_readme_is_actually_bound() {
    let config = fs::read_to_string("../resources/default-config.kdl")
        .expect("the shipped config should be readable");

    let missing: Vec<String> = documented()
        .into_iter()
        .filter(|key| !bound_in_config(&config, key))
        .collect();

    assert!(
        missing.is_empty(),
        "the README documents {} keybind(s) that default-config.kdl does not bind:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}
