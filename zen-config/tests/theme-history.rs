// Every dotfile ZEN installs must have its current content listed in
// resources/theme-history.txt.
//
// setup.sh only replaces a config on disk when that file's hash is in the manifest,
// which is how it tells a copy ZEN installed from one the user wrote. A theme edited
// without regenerating the manifest is a theme that can never be updated again on any
// machine that already has it, and nothing else would report that.
//
// Regenerate with scripts/theme-history.sh.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned()
}

fn hash(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let bytes: Vec<u8> = bytes.into_iter().filter(|b| *b != b'\r').collect();
    format!("{:x}", Sha256::digest(&bytes))
}

#[test]
fn manifest_covers_every_shipped_theme() {
    let root = root();
    let manifest = std::fs::read_to_string(root.join("resources/theme-history.txt"))
        .expect("resources/theme-history.txt is missing, run scripts/theme-history.sh");

    let setup = std::fs::read_to_string(root.join("setup.sh")).unwrap();

    let mut shipped = Vec::new();
    for line in setup.lines() {
        let line = line.trim();
        if !line.starts_with("theme_file ") {
            continue;
        }
        let src = line
            .split_whitespace()
            .find(|w| w.starts_with("resources/"))
            .unwrap_or_else(|| panic!("theme_file line names no resource: {line}"));
        shipped.push(src.to_owned());
    }

    assert!(
        !shipped.is_empty(),
        "found no theme_file calls in setup.sh, this test is not checking anything"
    );

    let mut stale = Vec::new();
    for src in &shipped {
        let path = root.join(src);
        let want = format!("{src} {}", hash(&path));
        if !manifest.lines().any(|l| l.trim() == want) {
            stale.push(src.clone());
        }
    }

    assert!(
        stale.is_empty(),
        "these themes changed without regenerating the manifest: {stale:?}\n\
         anyone who already installed them could never receive the fix.\n\
         run: bash scripts/theme-history.sh"
    );
}
