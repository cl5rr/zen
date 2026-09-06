use std::collections::BTreeMap;

use zen_config::binds::{Key, Modifiers};
use zen_config::Config;

// On a TTY "Mod" is Super, so Mod+Alt+L and Super+Alt+L are one chord even though
// they parse as different keys. Compare what the hardware actually sends.
fn chord(key: &Key) -> String {
    let mut modifiers = key.modifiers;
    if modifiers.contains(Modifiers::COMPOSITOR) {
        modifiers.remove(Modifiers::COMPOSITOR);
        modifiers.insert(Modifiers::SUPER);
    }
    format!("{:?}+{:?}", modifiers, key.trigger)
}

#[test]
fn shipped_binds_do_not_shadow_each_other() {
    let text = std::fs::read_to_string("../resources/default-config.kdl")
        .expect("the shipped config should be readable from the crate directory");
    let config = Config::parse_mem(&text).expect("the shipped config should parse");

    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for bind in &config.binds.0 {
        seen.entry(chord(&bind.key))
            .or_default()
            .push(format!("{:?}", bind.action));
    }

    let clashes: Vec<String> = seen
        .into_iter()
        .filter(|(_, actions)| actions.len() > 1)
        .map(|(key, actions)| format!("  {key}\n    {}", actions.join("\n    ")))
        .collect();

    assert!(
        clashes.is_empty(),
        "{} chord(s) are bound twice in default-config.kdl. \
         The first wins and the rest are dead:\n{}",
        clashes.len(),
        clashes.join("\n")
    );
}
