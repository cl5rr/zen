use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};
use serde_json::{Map, Value};

use crate::monitors::{row, separator};
use crate::App;

// The pills across the top of the screen.
//
// ZEN does not draw these. Writing a widget toolkit is a second project the size of the
// compositor, and waybar already is one, so ZEN ships a themed config for it and this
// page edits that config. The pills are waybar modules; the look is resources/waybar.
//
// The file is JSON, and it is rewritten wholesale on every change, so anything in it
// that is not JSON does not survive. ZEN's shipped config is therefore comment-free.
const CONFIG: &str = "waybar/config.jsonc";

// Every module this page offers, in the order they read on screen.
//
// `left` decides which of the three regions a module joins when it is switched on, so a
// pill always comes back to where it belongs rather than to wherever it was appended.
struct Pill {
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    region: Region,
}

#[derive(Clone, Copy, PartialEq)]
enum Region {
    Left,
    Centre,
    Right,
}

impl Region {
    fn field(self) -> &'static str {
        match self {
            Region::Left => "modules-left",
            Region::Centre => "modules-center",
            Region::Right => "modules-right",
        }
    }
}

const PILLS: &[Pill] = &[
    Pill {
        key: "niri/workspaces",
        label: "Workspaces",
        hint: "Reads ZEN over its IPC socket",
        region: Region::Left,
    },
    Pill {
        key: "niri/window",
        label: "Focused window",
        hint: "The title of whatever has focus",
        region: Region::Left,
    },
    Pill {
        key: "clock",
        label: "Clock",
        hint: "Click it for the date, again for a calendar",
        region: Region::Centre,
    },
    Pill {
        key: "tray",
        label: "Tray",
        hint: "Icons from apps that ask for one",
        region: Region::Right,
    },
    Pill {
        key: "pulseaudio",
        label: "Volume",
        hint: "Scroll to change it",
        region: Region::Right,
    },
    Pill {
        key: "backlight",
        label: "Brightness",
        hint: "Laptop panels only",
        region: Region::Right,
    },
    Pill {
        key: "battery",
        label: "Battery",
        hint: "Turns amber under 25%, red under 10%",
        region: Region::Right,
    },
    Pill {
        key: "network",
        label: "Network",
        hint: "The network you are on, or offline",
        region: Region::Right,
    },
    Pill {
        key: "cpu",
        label: "CPU",
        hint: "Load, as a percentage",
        region: Region::Right,
    },
    Pill {
        key: "memory",
        label: "Memory",
        hint: "How much is in use",
        region: Region::Right,
    },
    Pill {
        key: "temperature",
        label: "Temperature",
        hint: "Needs a sensor waybar can find",
        region: Region::Right,
    },
];

pub fn config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default()
        .join(CONFIG)
}

// page
pub fn page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    column.append(
        &gtk::Label::builder()
            .label("Status bar")
            .halign(Align::Start)
            .css_classes(["page-title"])
            .build(),
    );
    column.append(
        &gtk::Label::builder()
            .label(
                "The pills across the top. ZEN hosts waybar rather than drawing these \
                 itself, so anything waybar can show, the bar can show.",
            )
            .halign(Align::Start)
            .xalign(0.)
            .wrap(true)
            .css_classes(["page-blurb"])
            .build(),
    );

    let path = config_path();
    let Some(doc) = read(&path) else {
        column.append(
            &gtk::Label::builder()
                .label(format!(
                    "No bar config at {}. Run ./setup.sh --update to install ZEN's.",
                    path.display()
                ))
                .halign(Align::Start)
                .wrap(true)
                .xalign(0.)
                .css_classes(["setting-hint"])
                .build(),
        );
        return scrolled(&column);
    };

    let doc = Rc::new(RefCell::new(doc));

    column.append(
        &gtk::Label::builder()
            .label("THE BAR")
            .halign(Align::Start)
            .css_classes(["group-label"])
            .build(),
    );
    column.append(&bar_card(state, &doc));

    column.append(
        &gtk::Label::builder()
            .label("PILLS")
            .halign(Align::Start)
            .css_classes(["group-label"])
            .build(),
    );
    column.append(&pills_card(state, &doc));

    column.append(
        &gtk::Label::builder()
            .label(path.display().to_string())
            .halign(Align::Start)
            .selectable(true)
            .css_classes(["mono"])
            .build(),
    );

    scrolled(&column)
}

fn bar_card(state: &Rc<App>, doc: &Rc<RefCell<Map<String, Value>>>) -> gtk::Widget {
    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    let at_top = doc
        .borrow()
        .get("position")
        .and_then(|v| v.as_str())
        .unwrap_or("top")
        == "top";

    let position = gtk::Switch::builder().valign(Align::Center).active(at_top).build();
    {
        let state = state.clone();
        let doc = doc.clone();
        position.connect_active_notify(move |s| {
            let where_ = if s.is_active() { "top" } else { "bottom" };
            doc.borrow_mut()
                .insert("position".into(), Value::String(where_.into()));
            save(&state, &doc);
        });
    }
    card.append(&row("At the top", "Off puts the bar along the bottom", position.upcast()));

    separator(&card);

    let height = gtk::SpinButton::with_range(20., 64., 1.);
    height.set_value(doc.borrow().get("height").and_then(|v| v.as_f64()).unwrap_or(34.));
    height.set_valign(Align::Center);
    height.set_width_chars(5);
    {
        let state = state.clone();
        let doc = doc.clone();
        height.connect_value_changed(move |s| {
            let v = s.value().round() as i64;
            doc.borrow_mut().insert("height".into(), Value::from(v));
            save(&state, &doc);
        });
    }
    card.append(&row("Height", "In pixels", height.upcast()));

    separator(&card);

    let gap = gtk::SpinButton::with_range(0., 40., 1.);
    gap.set_value(
        doc.borrow()
            .get("margin-top")
            .and_then(|v| v.as_f64())
            .unwrap_or(8.),
    );
    gap.set_valign(Align::Center);
    gap.set_width_chars(5);
    {
        let state = state.clone();
        let doc = doc.clone();
        gap.connect_value_changed(move |s| {
            let v = s.value().round() as i64;
            let mut doc_ref = doc.borrow_mut();
            // Kept off every edge together, so the bar floats rather than being tucked
            // into one corner of the screen.
            for key in ["margin-top", "margin-left", "margin-right"] {
                doc_ref.insert(key.into(), Value::from(v));
            }
            drop(doc_ref);
            save(&state, &doc);
        });
    }
    card.append(&row(
        "Edge gap",
        "How far the bar floats off the screen edges",
        gap.upcast(),
    ));

    card.upcast()
}

fn pills_card(state: &Rc<App>, doc: &Rc<RefCell<Map<String, Value>>>) -> gtk::Widget {
    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    for (i, pill) in PILLS.iter().enumerate() {
        if i > 0 {
            separator(&card);
        }

        let on = gtk::Switch::builder()
            .valign(Align::Center)
            .active(is_shown(&doc.borrow(), pill.key))
            .build();
        {
            let state = state.clone();
            let doc = doc.clone();
            on.connect_active_notify(move |s| {
                {
                    let mut doc_ref = doc.borrow_mut();
                    set_shown(&mut doc_ref, pill, s.is_active());
                }
                save(&state, &doc);
            });
        }
        card.append(&row(pill.label, pill.hint, on.upcast()));
    }

    card.upcast()
}

// the document
fn read(path: &PathBuf) -> Option<Map<String, Value>> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&strip_comments(&text)).ok()?;
    value.as_object().cloned()
}

// waybar accepts // comments; serde_json does not. Only line comments are handled,
// because that is the only kind waybar's own documentation uses.
fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            // A // inside a string is part of a value, not a comment.
            Some(at) if line[..at].matches('"').count() % 2 == 0 => &line[..at],
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_shown(doc: &Map<String, Value>, key: &str) -> bool {
    [Region::Left, Region::Centre, Region::Right]
        .iter()
        .any(|region| region_list(doc, *region).iter().any(|m| m == key))
}

fn region_list(doc: &Map<String, Value>, region: Region) -> Vec<String> {
    doc.get(region.field())
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

// Adding a pill puts it back in its own region, in the order PILLS declares, so
// switching one off and on again does not shuffle the bar.
fn set_shown(doc: &mut Map<String, Value>, pill: &Pill, on: bool) {
    for region in [Region::Left, Region::Centre, Region::Right] {
        let mut list = region_list(doc, region);
        list.retain(|m| m != pill.key);
        doc.insert(
            region.field().into(),
            Value::Array(list.into_iter().map(Value::String).collect()),
        );
    }

    if !on {
        return;
    }

    let mut list = region_list(doc, pill.region);
    let order: Vec<&str> = PILLS
        .iter()
        .filter(|p| p.region == pill.region)
        .map(|p| p.key)
        .collect();
    let rank = |key: &str| order.iter().position(|k| *k == key).unwrap_or(usize::MAX);

    let at = list
        .iter()
        .position(|m| rank(m) > rank(pill.key))
        .unwrap_or(list.len());
    list.insert(at, pill.key.to_owned());

    doc.insert(
        pill.region.field().into(),
        Value::Array(list.into_iter().map(Value::String).collect()),
    );
}

fn save(state: &Rc<App>, doc: &Rc<RefCell<Map<String, Value>>>) {
    let path = config_path();
    let text = match serde_json::to_string_pretty(&Value::Object(doc.borrow().clone())) {
        Ok(text) => text,
        Err(err) => {
            state.say(&format!("could not write the bar config: {err}"), "bad");
            return;
        }
    };

    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(err) = std::fs::write(&path, format!("{text}\n")) {
        state.say(&format!("could not write {}: {err}", path.display()), "bad");
        return;
    }

    // waybar rereads its config on SIGUSR2, so the bar changes without a restart and
    // without this page having to own its lifetime.
    let reloaded = Command::new("pkill")
        .args(["-USR2", "-x", "waybar"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if reloaded {
        state.say("bar updated", "good");
    } else {
        state.say("saved. Start waybar to see it", "");
    }
}

fn scrolled(child: &gtk::Box) -> gtk::Widget {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(child)
        .hexpand(true)
        .vexpand(true)
        .build()
        .upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Map<String, Value> {
        let text = include_str!("../../resources/waybar/config.jsonc");
        serde_json::from_str::<Value>(&strip_comments(text))
            .expect("the shipped bar config must be valid JSON once comments are gone")
            .as_object()
            .unwrap()
            .clone()
    }

    #[test]
    fn the_shipped_config_parses_and_has_the_pills_it_claims() {
        let doc = shipped();
        for key in ["niri/workspaces", "niri/window", "clock", "battery"] {
            assert!(is_shown(&doc, key), "{key} should be on by default");
        }
        for key in ["cpu", "memory", "temperature"] {
            assert!(!is_shown(&doc, key), "{key} should be off by default");
        }
    }

    // Every pill this page offers has to be a module waybar will accept, and every
    // module the shipped config turns on has to be one the page can turn off again.
    #[test]
    fn the_page_and_the_shipped_config_agree_on_the_modules() {
        let doc = shipped();
        for region in [Region::Left, Region::Centre, Region::Right] {
            for module in region_list(&doc, region) {
                assert!(
                    PILLS.iter().any(|p| p.key == module),
                    "the config ships {module} but the page cannot turn it off"
                );
            }
        }
    }

    #[test]
    fn switching_a_pill_off_and_on_puts_it_back_where_it_belongs() {
        let mut doc = shipped();
        let before = region_list(&doc, Region::Right);

        let battery = PILLS.iter().find(|p| p.key == "battery").unwrap();
        set_shown(&mut doc, battery, false);
        assert!(!is_shown(&doc, "battery"));

        set_shown(&mut doc, battery, true);
        assert_eq!(
            region_list(&doc, Region::Right),
            before,
            "the bar was reshuffled by a round trip"
        );
    }

    #[test]
    fn a_pill_never_lands_in_two_regions() {
        let mut doc = shipped();
        let clock = PILLS.iter().find(|p| p.key == "clock").unwrap();
        set_shown(&mut doc, clock, true);
        set_shown(&mut doc, clock, true);

        let count: usize = [Region::Left, Region::Centre, Region::Right]
            .iter()
            .map(|r| region_list(&doc, *r).iter().filter(|m| *m == "clock").count())
            .sum();
        assert_eq!(count, 1, "clock appears {count} times");
    }

    #[test]
    fn a_comment_is_stripped_but_a_url_in_a_string_is_not() {
        let text = "{\n  // a comment\n  \"a\": \"https://example.com\"\n}";
        let doc: Value = serde_json::from_str(&strip_comments(text)).unwrap();
        assert_eq!(doc["a"], "https://example.com");
    }
}
