use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

    pub fn save(&self) -> anyhow::Result<bool> {
        let tmp = self.path.with_extension("kdl.new");
        fs::write(&tmp, self.doc.to_string())?;

        let checked = match Command::new("zen")
            .args(["validate", "--config"])
            .arg(&tmp)
            .output()
        {
            Ok(out) if out.status.success() => true,
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stderr).into_owned();
                let why = text
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("no reason given")
                    .to_owned();
                let _ = fs::remove_file(&tmp);
                bail!("rejected: {why}");
            }
            Err(_) => false,
        };

        fs::rename(&tmp, &self.path)?;
        Ok(checked)
    }

    // read
    fn section(&self, path: &[&str]) -> Option<&KdlNode> {
        let mut doc = &self.doc;
        let mut found = None;
        for name in path {
            let node = doc.get(name)?;
            found = Some(node);
            doc = node.children()?;
        }
        found
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
            if doc.get(name).is_none() {
                doc.nodes_mut().push(fresh(name, depth));
            }
            let node = doc.get_mut(name).unwrap();
            if node.children().is_none() {
                node.set_children(empty(depth));
            }
            doc = node.children_mut().as_mut().unwrap();
        }
        doc
    }
}

// util
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
