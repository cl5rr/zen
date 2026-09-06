use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context as _};
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

// paths
pub fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    base.join("zen").join("config.kdl")
}

pub fn wallpaper_dir() -> PathBuf {
    config_path()
        .parent()
        .map(|p| p.join("wallpapers"))
        .unwrap_or_default()
}

// document
pub struct Config {
    path: PathBuf,
    doc: KdlDocument,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = config_path();
        let text = fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let doc: KdlDocument = text.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Self { path, doc })
    }

    // Checked in process rather than by shelling out to `zen validate`. Spawning the
    // compositor binary per keystroke cost more than everything else here put together,
    // and it made the safety net depend on zen being on PATH.
    pub fn save(&self) -> anyhow::Result<()> {
        let text = self.doc.to_string();

        if let Err(err) = zen_config::Config::parse_mem(&text) {
            let why = format!("{err}");
            let why = why
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("no reason given")
                .to_owned();
            bail!("rejected: {why}");
        }

        let tmp = self.path.with_extension("kdl.new");
        fs::write(&tmp, &text)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    // read
    //
    // A config has several window-rule nodes and only one of them applies to every
    // window: the one with no match. Taking the first by name would land on whichever
    // app-specific rule happens to be written earliest, so settings that mean "all
    // windows" would silently edit the wezterm rule.
    fn pick<'a>(doc: &'a KdlDocument, name: &str) -> Option<&'a KdlNode> {
        let idx = pick_index(doc, name)?;
        Some(&doc.nodes()[idx])
    }

    fn section(&self, path: &[&str]) -> Option<&KdlNode> {
        let mut doc = &self.doc;
        let mut found = None;
        for name in path {
            let node = Self::pick(doc, name)?;
            found = Some(node);
            doc = node.children()?;
        }
        found
    }

    pub fn boolean(&self, path: &[&str], key: &str) -> Option<bool> {
        self.leaf(path, key)?.entries().first()?.value().as_bool()
    }

    fn leaf(&self, path: &[&str], key: &str) -> Option<&KdlNode> {
        self.section(path)?.children()?.get(key)
    }

    pub fn number(&self, path: &[&str], key: &str) -> Option<f64> {
        let value = self.leaf(path, key)?.entries().first()?.value();
        value.as_f64().or_else(|| value.as_i64().map(|i| i as f64))
    }

    pub fn flag(&self, path: &[&str], key: &str) -> bool {
        self.section(path)
            .and_then(|n| n.children())
            .is_some_and(|c| c.get(key).is_some())
    }

    pub fn string(&self, path: &[&str], key: &str) -> Option<String> {
        self.leaf(path, key)?
            .entries()
            .first()?
            .value()
            .as_string()
            .map(str::to_owned)
    }

    pub fn prop(&self, path: &[&str], key: &str, prop: &str) -> Option<f64> {
        let value = self.leaf(path, key)?.get(prop)?.value();
        value.as_f64().or_else(|| value.as_i64().map(|i| i as f64))
    }

    // write
    pub fn set_number(&mut self, path: &[&str], key: &str, value: f64, digits: usize) {
        let entry = if digits == 0 {
            number_entry(KdlValue::Base10(value.round() as i64), format(value, 0))
        } else {
            number_entry(KdlValue::Base10Float(value), format(value, digits))
        };
        self.set_argument(path, key, entry);
    }

    pub fn set_string(&mut self, path: &[&str], key: &str, value: &str) {
        let mut entry = KdlEntry::new(KdlValue::String(value.to_owned()));
        entry.set_value_repr(format!("{value:?}"));
        self.set_argument(path, key, entry);
    }

    pub fn set_boolean(&mut self, path: &[&str], key: &str, value: bool) {
        let mut entry = KdlEntry::new(KdlValue::Bool(value));
        entry.set_value_repr(value.to_string());
        self.set_argument(path, key, entry);
    }

    pub fn set_flag(&mut self, path: &[&str], key: &str, on: bool) {
        let depth = path.len();
        let children = self.children_mut(path);

        let present = children.get(key).is_some();
        if on && !present {
            children.nodes_mut().push(fresh(key, depth));
        } else if !on && present {
            children.nodes_mut().retain(|n| n.name().value() != key);
        }
    }

    pub fn set_prop(&mut self, path: &[&str], key: &str, prop: &str, value: f64) {
        let depth = path.len();
        let node = self.leaf_mut(path, key, depth);

        let mut entry = KdlEntry::new_prop(prop, KdlValue::Base10(value.round() as i64));
        entry.set_value_repr(format(value, 0));

        let at = node
            .entries()
            .iter()
            .position(|e| e.name().is_some_and(|n| n.value() == prop));
        match at {
            Some(i) => node.entries_mut()[i] = entry,
            None => node.entries_mut().push(entry),
        }
    }

    fn set_argument(&mut self, path: &[&str], key: &str, entry: KdlEntry) {
        let depth = path.len();
        let node = self.leaf_mut(path, key, depth);

        let at = node.entries().iter().position(|e| e.name().is_none());
        match at {
            Some(i) => node.entries_mut()[i] = entry,
            None => node.entries_mut().push(entry),
        }
    }

    fn leaf_mut(&mut self, path: &[&str], key: &str, depth: usize) -> &mut KdlNode {
        let children = self.children_mut(path);
        if children.get(key).is_none() {
            children.nodes_mut().push(fresh(key, depth));
        }
        children.get_mut(key).unwrap()
    }

    fn children_mut(&mut self, path: &[&str]) -> &mut KdlDocument {
        let mut doc = &mut self.doc;
        for (depth, name) in path.iter().enumerate() {
            let idx = match pick_index(doc, name) {
                Some(idx) => idx,
                None => {
                    doc.nodes_mut().push(fresh(node_name(name), depth));
                    let idx = doc.nodes().len() - 1;
                    // A rule created for a selector must carry the match it was
                    // selected by, or it would apply to every window and never be
                    // found again.
                    if let Some(selector) = name.split_once('@').map(|(_, sel)| sel) {
                        add_match(&mut doc.nodes_mut()[idx], selector, depth + 1);
                    }
                    idx
                }
            };
            let node = &mut doc.nodes_mut()[idx];
            if node.children().is_none() {
                node.set_children(empty(depth));
            }
            doc = node.children_mut().as_mut().unwrap();
        }
        doc
    }
}

// util
//
// A path segment may carry a selector after '@' so a spec row can name one node out of
// several with the same name. "window-rule" alone means the rule with no match at all,
// the one that applies to every window; "window-rule@is-active=true" means the rule
// whose match says exactly that. Without this, every rule row would edit whichever
// window-rule happened to be written first.
fn node_name(segment: &str) -> &str {
    segment.split_once('@').map_or(segment, |(name, _)| name)
}

fn match_nodes(node: &KdlNode) -> impl Iterator<Item = &KdlNode> {
    node.children()
        .into_iter()
        .flat_map(|c| c.nodes().iter())
        .filter(|n| n.name().value() == "match")
}

fn selects(node: &KdlNode, selector: Option<&str>) -> bool {
    let Some(selector) = selector else {
        return match_nodes(node).next().is_none();
    };
    let Some((prop, want)) = selector.split_once('=') else {
        return false;
    };
    match_nodes(node).any(|m| {
        m.get(prop)
            .is_some_and(|e| e.value().as_bool().map(|b| b.to_string()) == Some(want.to_owned()))
    })
}

fn add_match(node: &mut KdlNode, selector: &str, depth: usize) {
    let Some((prop, want)) = selector.split_once('=') else {
        return;
    };
    let Ok(want) = want.parse::<bool>() else {
        return;
    };
    if node.children().is_none() {
        node.set_children(empty(depth - 1));
    }
    let mut m = fresh("match", depth);
    let mut entry = KdlEntry::new_prop(prop, KdlValue::Bool(want));
    entry.set_value_repr(want.to_string());
    m.entries_mut().push(entry);
    node.children_mut().as_mut().unwrap().nodes_mut().push(m);
}

fn pick_index(doc: &KdlDocument, segment: &str) -> Option<usize> {
    let (name, selector) = match segment.split_once('@') {
        Some((name, sel)) => (name, Some(sel)),
        None => (segment, None),
    };

    // Only rule nodes come in several copies; everything else is a plain lookup, and
    // treating it as selectable would make an absent selector mean "has no children".
    let selectable = matches!(name, "window-rule" | "layer-rule");
    doc.nodes().iter().position(|n| {
        n.name().value() == name && (!selectable || selects(n, selector))
    })
}

fn fresh(name: &str, depth: usize) -> KdlNode {
    let mut node = KdlNode::new(name);
    node.set_leading("    ".repeat(depth));
    node.set_trailing("\n");
    node
}

fn empty(depth: usize) -> KdlDocument {
    let mut doc = KdlDocument::new();
    doc.set_leading("\n");
    doc.set_trailing("    ".repeat(depth));
    doc
}

fn number_entry(value: KdlValue, repr: String) -> KdlEntry {
    let mut entry = KdlEntry::new(value);
    entry.set_value_repr(repr);
    entry
}

fn format(value: f64, digits: usize) -> String {
    if digits == 0 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.digits$}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from(text: &str) -> Config {
        Config {
            path: PathBuf::new(),
            doc: text.parse().unwrap(),
        }
    }

    fn shipped() -> Config {
        from(include_str!("../../resources/default-config.kdl"))
    }

    // The shipped config has four window-rule nodes and only the third applies to
    // every window. A path lookup by name alone lands on the wezterm rule, so the
    // tint toggle would silently edit that instead, and nothing else would notice.
    #[test]
    fn the_tint_toggle_finds_the_rule_that_has_no_match() {
        let mut c = shipped();
        assert_eq!(c.boolean(&["window-rule"], "draw-border-with-background"), Some(false));

        c.set_boolean(&["window-rule"], "draw-border-with-background", true);
        let text = c.doc.to_string();

        // Counted as nodes, not as text: the config explains the setting in a comment
        // further up, so a substring count is 2 before anything is even written.
        let carriers: Vec<bool> = c
            .doc
            .nodes()
            .iter()
            .filter(|n| n.name().value() == "window-rule")
            .map(|n| {
                n.children().is_some_and(|c| {
                    c.nodes()
                        .iter()
                        .any(|n| n.name().value() == "draw-border-with-background")
                })
            })
            .collect();
        assert_eq!(
            carriers.iter().filter(|c| **c).count(),
            1,
            "the setting landed in {} of {} rules",
            carriers.iter().filter(|c| **c).count(),
            carriers.len()
        );
        assert!(text.contains("draw-border-with-background true"));
        assert!(
            text.contains("wezterm"),
            "the app-specific rules must survive the edit"
        );

        let reread = from(&text);
        assert_eq!(reread.boolean(&["window-rule"], "draw-border-with-background"), Some(true));
        zen_config::Config::parse_mem(&text).expect("the edited config must still parse");
    }

    // The two opacity rules are told apart only by their match, and creating one
    // without its match would produce a rule that applies to every window.
    #[test]
    fn opacity_rows_address_the_active_and_inactive_rules_separately() {
        let mut c = shipped();
        assert_eq!(c.number(&["window-rule@is-active=true"], "opacity"), Some(0.92));
        assert_eq!(c.number(&["window-rule@is-active=false"], "opacity"), Some(0.82));

        c.set_number(&["window-rule@is-active=true"], "opacity", 0.5, 2);
        let text = c.doc.to_string();
        let reread = from(&text);
        assert_eq!(reread.number(&["window-rule@is-active=true"], "opacity"), Some(0.5));
        assert_eq!(
            reread.number(&["window-rule@is-active=false"], "opacity"),
            Some(0.82),
            "editing the focused rule must not touch the unfocused one"
        );
        zen_config::Config::parse_mem(&text).expect("the edited config must still parse");
    }

    #[test]
    fn a_rule_created_for_a_selector_carries_that_match() {
        let mut c = from("layout {
    gaps 20
}
");
        c.set_number(&["window-rule@is-active=true"], "opacity", 0.5, 2);
        let text = c.doc.to_string();

        assert!(text.contains("is-active=true"), "created rule has no match: {text}");
        let reread = from(&text);
        assert_eq!(reread.number(&["window-rule@is-active=true"], "opacity"), Some(0.5));
        assert_eq!(
            reread.number(&["window-rule"], "opacity"),
            None,
            "the new rule must not read as the global one"
        );
        zen_config::Config::parse_mem(&text).expect("the created config must still parse");
    }

    #[test]
    fn reads_the_shipped_config() {
        let c = shipped();
        assert_eq!(c.number(&["glass"], "opacity"), Some(0.30));
        assert_eq!(c.number(&["layout"], "gaps"), Some(20.));
        assert!(c.flag(&["layout", "shadow"], "on"));
        assert!(c.flag(&["layout", "border"], "off"));
        assert_eq!(c.prop(&["layout", "shadow"], "offset", "y"), Some(10.));
    }

    #[test]
    fn edits_survive_a_round_trip() {
        let mut c = shipped();
        c.set_number(&["glass"], "opacity", 0.42, 2);
        c.set_number(&["layout"], "gaps", 31., 0);
        c.set_number(&["layout", "shadow"], "softness", 12., 0);
        c.set_prop(&["layout", "shadow"], "offset", "y", 7.);
        c.set_string(&["layout", "focus-ring"], "active-color", "#abcdefff");
        c.set_flag(&["layout", "border"], "off", false);
        c.set_flag(&["welcome"], "off", true);

        let back = from(&c.doc.to_string());
        assert_eq!(back.number(&["glass"], "opacity"), Some(0.42));
        assert_eq!(back.number(&["layout"], "gaps"), Some(31.));
        assert_eq!(back.number(&["layout", "shadow"], "softness"), Some(12.));
        assert_eq!(back.prop(&["layout", "shadow"], "offset", "y"), Some(7.));
        assert_eq!(
            back.string(&["layout", "focus-ring"], "active-color").as_deref(),
            Some("#abcdefff")
        );
        assert!(!back.flag(&["layout", "border"], "off"));
        assert!(back.flag(&["welcome"], "off"));
    }

    #[test]
    fn leaves_everything_else_alone() {
        let before = include_str!("../../resources/default-config.kdl");
        let mut c = shipped();
        c.set_number(&["layout"], "gaps", 7., 0);
        let after = c.doc.to_string();

        assert_eq!(before.lines().count(), after.lines().count());
        assert_eq!(
            before.matches("binds").count(),
            after.matches("binds").count()
        );
        assert!(after.contains("preset-column-widths"));
    }

    #[test]
    fn reads_the_binds_it_ships() {
        let c = shipped();
        let binds = c.binds();
        assert!(binds.len() > 40, "only found {} binds", binds.len());

        let terminal = binds.iter().find(|b| b.key == "Mod+T").expect("Mod+T");
        assert_eq!(terminal.action, "spawn");
        assert_eq!(terminal.args, vec!["alacritty".to_owned()]);
        assert_eq!(terminal.title.as_deref(), Some("Terminal"));
    }

    #[test]
    fn rebinding_moves_the_key_and_keeps_the_action() {
        let mut c = shipped();
        c.rebind("Mod+T", "Mod+Shift+T").unwrap();

        let back = from(&c.doc.to_string());
        let moved = back
            .binds()
            .into_iter()
            .find(|b| b.key == "Mod+Shift+T")
            .expect("the rebound key");

        assert_eq!(moved.action, "spawn");
        assert_eq!(moved.args, vec!["alacritty".to_owned()]);
        assert!(!back.binds().iter().any(|b| b.key == "Mod+T"));
    }

    #[test]
    fn rebinding_onto_a_taken_chord_is_refused() {
        let mut c = shipped();
        assert!(c.rebind("Mod+T", "Mod+W").is_err());

        // and the config is untouched by the attempt
        let back = from(&c.doc.to_string());
        assert!(back.binds().iter().any(|b| b.key == "Mod+T"));
        assert!(back.binds().iter().any(|b| b.key == "Mod+W"));
    }

    #[test]
    fn changing_what_a_bind_launches() {
        let mut c = shipped();
        c.set_bind_args("Mod+T", &["kitty".to_owned()]).unwrap();

        let back = from(&c.doc.to_string());
        let terminal = back.binds().into_iter().find(|b| b.key == "Mod+T").unwrap();
        assert_eq!(terminal.args, vec!["kitty".to_owned()]);
        assert_eq!(terminal.title.as_deref(), Some("Terminal"));
    }

    #[test]
    fn startup_commands_round_trip() {
        let mut c = shipped();
        assert_eq!(
            c.startup(),
            vec![
                vec!["waybar".to_owned()],
                vec!["zen-wallpaper".to_owned(), "restore".to_owned()],
            ]
        );

        c.set_startup(&[
            vec!["waybar".to_owned()],
            vec!["zen-wallpaper".to_owned(), "restore".to_owned()],
        ]);

        let back = from(&c.doc.to_string());
        assert_eq!(
            back.startup(),
            vec![
                vec!["waybar".to_owned()],
                vec!["zen-wallpaper".to_owned(), "restore".to_owned()]
            ]
        );
    }

    #[test]
    fn output_blocks_round_trip() {
        let mut c = shipped();
        assert!(c.output("DP-1").mode.is_none(), "nothing configured yet");

        c.set_output_text("DP-1", "mode", "2560x1440@144.000");
        c.set_output_scale("DP-1", 1.25);
        c.set_output_position("DP-1", 1920, -120);
        c.set_output_off("DP-1", false);

        let back = from(&c.doc.to_string());
        let cfg = back.output("DP-1");
        assert_eq!(cfg.mode.as_deref(), Some("2560x1440@144.000"));
        assert_eq!(cfg.scale, Some(1.25));
        assert_eq!(cfg.position, Some((1920, -120)));
        assert!(!cfg.off);
    }

    #[test]
    fn turning_an_output_off_and_on_again_leaves_no_trace() {
        let mut c = shipped();
        c.set_output_off("HDMI-A-1", true);
        assert!(from(&c.doc.to_string()).output("HDMI-A-1").off);

        c.set_output_off("HDMI-A-1", false);
        let back = from(&c.doc.to_string());
        assert!(!back.output("HDMI-A-1").off);
    }

    #[test]
    fn moving_an_output_twice_does_not_stack_positions() {
        let mut c = shipped();
        c.set_output_position("DP-1", 100, 100);
        c.set_output_position("DP-1", 2560, 0);

        let back = from(&c.doc.to_string());
        assert_eq!(back.output("DP-1").position, Some((2560, 0)));

        // and it rewrote the node rather than appending a second one
        let node = back.output_node("DP-1").expect("the output block");
        let positions = node
            .children()
            .expect("its children")
            .nodes()
            .iter()
            .filter(|n| n.name().value() == "position")
            .count();
        assert_eq!(positions, 1, "position was written more than once");
    }

    #[test]
    fn writes_sections_that_are_missing() {
        let mut c = from("");
        c.set_number(&["glass"], "opacity", 0.2, 2);
        c.set_flag(&["layout", "shadow"], "on", true);
        c.set_prop(&["layout", "shadow"], "offset", "y", 4.);

        let back = from(&c.doc.to_string());
        assert_eq!(back.number(&["glass"], "opacity"), Some(0.2));
        assert!(back.flag(&["layout", "shadow"], "on"));
        assert_eq!(back.prop(&["layout", "shadow"], "offset", "y"), Some(4.));
    }
}

// lists
//
// Keybinds and startup commands are repeated nodes, not keyed values, so they need
// their own accessors. A bind is `Mod+T { spawn "alacritty"; }`: the node name is the
// chord and its one child is the action.
pub struct Entry {
    pub key: String,
    pub action: String,
    pub args: Vec<String>,
    pub title: Option<String>,
}

impl Config {
    pub fn binds(&self) -> Vec<Entry> {
        let Some(binds) = self.doc.get("binds").and_then(KdlNode::children) else {
            return Vec::new();
        };

        binds
            .nodes()
            .iter()
            .filter_map(|node| {
                let action = node.children()?.nodes().first()?;
                Some(Entry {
                    key: node.name().value().to_owned(),
                    action: action.name().value().to_owned(),
                    args: action
                        .entries()
                        .iter()
                        .filter(|e| e.name().is_none())
                        .filter_map(|e| e.value().as_string().map(str::to_owned))
                        .collect(),
                    title: node
                        .get("hotkey-overlay-title")
                        .and_then(|e| e.value().as_string())
                        .map(str::to_owned),
                })
            })
            .collect()
    }

    // Renaming the node is the whole edit: the action underneath is untouched, so a
    // rebind cannot lose what the bind does.
    pub fn rebind(&mut self, from: &str, to: &str) -> anyhow::Result<()> {
        if from == to {
            return Ok(());
        }
        if self.binds().iter().any(|b| b.key == to) {
            bail!("{to} is already bound");
        }

        let Some(binds) = self
            .doc
            .get_mut("binds")
            .and_then(|n| n.children_mut().as_mut())
        else {
            bail!("this config has no binds section");
        };
        let Some(node) = binds.nodes_mut().iter_mut().find(|n| n.name().value() == from) else {
            bail!("{from} is not bound");
        };

        node.set_name(to);
        Ok(())
    }

    // Only the arguments of a spawn change; the action stays whatever it was.
    pub fn set_bind_args(&mut self, key: &str, args: &[String]) -> anyhow::Result<()> {
        let Some(binds) = self
            .doc
            .get_mut("binds")
            .and_then(|n| n.children_mut().as_mut())
        else {
            bail!("this config has no binds section");
        };
        let Some(node) = binds.nodes_mut().iter_mut().find(|n| n.name().value() == key) else {
            bail!("{key} is not bound");
        };
        let Some(children) = node.children_mut().as_mut() else {
            bail!("{key} has no action");
        };
        let Some(action) = children.nodes_mut().first_mut() else {
            bail!("{key} has no action");
        };

        action.entries_mut().retain(|e| e.name().is_some());
        for arg in args {
            let mut entry = KdlEntry::new(KdlValue::String(arg.clone()));
            entry.set_value_repr(format!("{arg:?}"));
            action.entries_mut().push(entry);
        }
        Ok(())
    }

    pub fn startup(&self) -> Vec<Vec<String>> {
        self.doc
            .nodes()
            .iter()
            .filter(|n| matches!(n.name().value(), "spawn-at-startup" | "spawn-sh-at-startup"))
            .map(|n| {
                n.entries()
                    .iter()
                    .filter(|e| e.name().is_none())
                    .filter_map(|e| e.value().as_string().map(str::to_owned))
                    .collect()
            })
            .collect()
    }

    pub fn set_startup(&mut self, commands: &[Vec<String>]) {
        self.doc
            .nodes_mut()
            .retain(|n| !matches!(n.name().value(), "spawn-at-startup" | "spawn-sh-at-startup"));

        for command in commands {
            if command.is_empty() {
                continue;
            }
            let mut node = fresh("spawn-at-startup", 0);
            for arg in command {
                let mut entry = KdlEntry::new(KdlValue::String(arg.clone()));
                entry.set_value_repr(format!("{arg:?}"));
                node.entries_mut().push(entry);
            }
            self.doc.nodes_mut().push(node);
        }
    }
}

// outputs
//
// Outputs are repeated named blocks rather than keyed values, so they need their own
// accessors like the binds do. The name is the node's one argument.
pub struct OutputCfg {
    pub name: String,
    pub off: bool,
    pub mode: Option<String>,
    pub scale: Option<f64>,
    pub position: Option<(i64, i64)>,
    pub transform: Option<String>,
}

impl Config {
    fn output_node(&self, name: &str) -> Option<&KdlNode> {
        self.doc.nodes().iter().find(|n| {
            n.name().value() == "output"
                && n.entries()
                    .first()
                    .and_then(|e| e.value().as_string())
                    .is_some_and(|n| n == name)
        })
    }

    #[cfg(test)]
    pub fn from_str_for_test(text: &str) -> Self {
        Self {
            path: PathBuf::new(),
            doc: text.parse().unwrap(),
        }
    }

    pub fn output(&self, name: &str) -> OutputCfg {
        let mut cfg = OutputCfg {
            name: name.to_owned(),
            off: false,
            mode: None,
            scale: None,
            position: None,
            transform: None,
        };

        let Some(children) = self.output_node(name).and_then(KdlNode::children) else {
            return cfg;
        };

        cfg.off = children.get("off").is_some();
        cfg.mode = children
            .get("mode")
            .and_then(|n| n.entries().first())
            .and_then(|e| e.value().as_string())
            .map(str::to_owned);
        cfg.transform = children
            .get("transform")
            .and_then(|n| n.entries().first())
            .and_then(|e| e.value().as_string())
            .map(str::to_owned);
        cfg.scale = children
            .get("scale")
            .and_then(|n| n.entries().first())
            .and_then(|e| e.value().as_f64().or_else(|| e.value().as_i64().map(|i| i as f64)));

        if let Some(node) = children.get("position") {
            let read = |k: &str| {
                node.get(k)
                    .and_then(|e| e.value().as_i64())
                    .or_else(|| node.get(k).and_then(|e| e.value().as_f64()).map(|f| f as i64))
            };
            if let (Some(x), Some(y)) = (read("x"), read("y")) {
                cfg.position = Some((x, y));
            }
        }

        cfg
    }

    fn output_children(&mut self, name: &str) -> &mut KdlDocument {
        if self.output_node(name).is_none() {
            let mut node = fresh("output", 0);
            let mut entry = KdlEntry::new(KdlValue::String(name.to_owned()));
            entry.set_value_repr(format!("{name:?}"));
            node.entries_mut().push(entry);
            node.set_children(empty(0));
            self.doc.nodes_mut().push(node);
        }

        let node = self
            .doc
            .nodes_mut()
            .iter_mut()
            .find(|n| {
                n.name().value() == "output"
                    && n.entries()
                        .first()
                        .and_then(|e| e.value().as_string())
                        .is_some_and(|n| n == name)
            })
            .unwrap();

        if node.children().is_none() {
            node.set_children(empty(0));
        }
        node.children_mut().as_mut().unwrap()
    }

    pub fn set_output_off(&mut self, name: &str, off: bool) {
        let children = self.output_children(name);
        let present = children.get("off").is_some();
        if off && !present {
            children.nodes_mut().push(fresh("off", 1));
        } else if !off && present {
            children.nodes_mut().retain(|n| n.name().value() != "off");
        }
    }

    pub fn set_output_text(&mut self, name: &str, key: &str, value: &str) {
        let children = self.output_children(name);
        if children.get(key).is_none() {
            children.nodes_mut().push(fresh(key, 1));
        }
        let node = children.get_mut(key).unwrap();

        let mut entry = KdlEntry::new(KdlValue::String(value.to_owned()));
        entry.set_value_repr(format!("{value:?}"));

        node.entries_mut().retain(|e| e.name().is_some());
        node.entries_mut().push(entry);
    }

    pub fn set_output_scale(&mut self, name: &str, scale: f64) {
        let children = self.output_children(name);
        if children.get("scale").is_none() {
            children.nodes_mut().push(fresh("scale", 1));
        }
        let node = children.get_mut("scale").unwrap();

        let mut entry = KdlEntry::new(KdlValue::Base10Float(scale));
        entry.set_value_repr(format!("{scale:.2}"));

        node.entries_mut().retain(|e| e.name().is_some());
        node.entries_mut().push(entry);
    }

    pub fn set_output_position(&mut self, name: &str, x: i64, y: i64) {
        let children = self.output_children(name);
        if children.get("position").is_none() {
            children.nodes_mut().push(fresh("position", 1));
        }
        let node = children.get_mut("position").unwrap();
        node.entries_mut().clear();

        for (key, value) in [("x", x), ("y", y)] {
            let mut entry = KdlEntry::new_prop(key, KdlValue::Base10(value));
            entry.set_value_repr(format!("{value}"));
            node.entries_mut().push(entry);
        }
    }
}
