use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};
use serde_json::{Map, Value};

use crate::monitors::{row, separator};
use crate::require::{self, Need, Probe};
use crate::App;

const NEEDS: &[Need] = &[Need {
    command: "waybar",
    what: "the bar itself. ZEN hosts it rather than drawing one",
    package: "waybar",
    probe: Probe::OnPath,
}];

const CONFIG: &str = "waybar/config.jsonc";
const MODULES: &str = "~/.config/waybar/zen-modules.jsonc";

const WAYBAR_RULE: &str = "layer-rule@namespace=waybar";

const DEFAULT_FILL: f64 = 0.55;

const MODES: [&str; 4] = ["dock", "hide", "overlay", "invisible"];
const LAYERS: [&str; 3] = ["top", "overlay", "bottom"];

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
        key: "custom/power",
        label: "Power menu",
        hint: "Lock, log out, suspend, restart, shut down",
        region: Region::Right,
    },
    Pill {
        key: "custom/overview",
        label: "Map button",
        hint: "The same thing Mod+O does",
        region: Region::Left,
    },
    Pill {
        key: "custom/wallpaper",
        label: "Wallpaper button",
        hint: "Click to pick one, right click for the next",
        region: Region::Right,
    },
    Pill {
        key: "custom/clipboard",
        label: "Clipboard history",
        hint: "Needs cliphist and fuzzel",
        region: Region::Right,
    },
    Pill {
        key: "custom/notifications",
        label: "Dismiss notifications",
        hint: "Needs mako",
        region: Region::Right,
    },
    Pill {
        key: "mpris",
        label: "Now playing",
        hint: "Whatever a media player is reporting",
        region: Region::Centre,
    },
    Pill {
        key: "bluetooth",
        label: "Bluetooth",
        hint: "",
        region: Region::Right,
    },
    Pill {
        key: "idle_inhibitor",
        label: "Keep awake",
        hint: "Click to stop the idle timer locking the screen",
        region: Region::Right,
    },
    Pill {
        key: "disk",
        label: "Disk",
        hint: "How full the root filesystem is",
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

    let body = gtk::Box::new(Orientation::Vertical, 10);

    for (title, card) in [
        ("THE BAR", bar_card(state, &doc)),
        ("PILLS", pills_card(state, &doc)),
        ("LOOK", look_card(state)),
    ] {
        body.append(
            &gtk::Label::builder()
                .label(title)
                .halign(Align::Start)
                .css_classes(["group-label"])
                .build(),
        );
        body.append(&card);
    }

    require::guard(&column, &body.clone().upcast(), NEEDS);
    column.append(&body);

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
            for key in ["margin-top", "margin-bottom", "margin-left", "margin-right"] {
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

    separator(&card);

    let reserve = gtk::Switch::builder()
        .valign(Align::Center)
        .active(
            doc.borrow()
                .get("exclusive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        )
        .build();
    {
        let state = state.clone();
        let doc = doc.clone();
        reserve.connect_active_notify(move |s| {
            doc.borrow_mut()
                .insert("exclusive".into(), Value::Bool(s.is_active()));
            save(&state, &doc);
        });
    }
    let reserve_row = row(
        "Reserve its space",
        "On, windows stop short of the bar. Off, it floats over them and nothing is \
         cut down to make room",
        reserve.upcast(),
    );
    card.append(&reserve_row);

    separator(&card);

    let current_mode = doc
        .borrow()
        .get("mode")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "dock".to_owned());
    let mode = gtk::DropDown::builder()
        .model(&gtk::StringList::new(&[
            "Always visible",
            "Hidden until Mod+B",
            "Over fullscreen too",
            "Invisible but still there",
        ]))
        .selected(MODES.iter().position(|m| *m == current_mode).unwrap_or(0) as u32)
        .valign(Align::Center)
        .build();
    {
        let state = state.clone();
        let doc = doc.clone();
        mode.connect_selected_notify(move |d| {
            let Some(picked) = MODES.get(d.selected() as usize) else {
                return;
            };
            {
                let mut doc = doc.borrow_mut();
                doc.insert("mode".into(), Value::String((*picked).into()));
                if *picked != "dock" {
                    doc.remove("exclusive");
                    doc.remove("layer");
                }
            }
            save(&state, &doc);
        });
    }
    card.append(&row(
        "When to show it",
        "waybar cannot reveal itself on hover, so hiding it is a toggle: Mod+B. \
         Anything but Always visible makes waybar decide the two settings below",
        mode.clone().upcast(),
    ));

    separator(&card);

    let current_layer = doc
        .borrow()
        .get("layer")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "top".to_owned());
    let layer = gtk::DropDown::builder()
        .model(&gtk::StringList::new(&[
            "Above windows",
            "Above everything",
            "Behind windows",
        ]))
        .selected(LAYERS.iter().position(|l| *l == current_layer).unwrap_or(0) as u32)
        .valign(Align::Center)
        .build();
    {
        let state = state.clone();
        let doc = doc.clone();
        layer.connect_selected_notify(move |d| {
            let Some(picked) = LAYERS.get(d.selected() as usize) else {
                return;
            };
            doc.borrow_mut()
                .insert("layer".into(), Value::String((*picked).into()));
            save(&state, &doc);
        });
    }
    let layer_row = row(
        "Stacking",
        "Where the bar sits against windows",
        layer.upcast(),
    );
    card.append(&layer_row);

    let follow_mode = {
        let reserve_row = reserve_row.clone();
        let layer_row = layer_row.clone();
        move |picked: &str| {
            let owned = picked == "dock";
            reserve_row.set_sensitive(owned);
            layer_row.set_sensitive(owned);
        }
    };
    follow_mode(&current_mode);
    {
        let follow_mode = follow_mode.clone();
        mode.connect_selected_notify(move |d| {
            if let Some(picked) = MODES.get(d.selected() as usize) {
                follow_mode(picked);
            }
        });
    }

    card.upcast()
}

fn look_card(state: &Rc<App>) -> gtk::Widget {
    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    let opacity = gtk::Scale::with_range(Orientation::Horizontal, 0., 1., 0.01);
    opacity.set_value(current_opacity());
    opacity.set_draw_value(true);
    opacity.set_size_request(200, -1);
    opacity.set_valign(Align::Center);
    {
        let state = state.clone();
        opacity.connect_value_changed(move |s| write_pill_css(&state, s.value()));
    }
    card.append(&row(
        "Fill",
        "0 is fully transparent, 1 is solid. The shipped look is 0.55",
        opacity.upcast(),
    ));

    separator(&card);

    let blur = gtk::Switch::builder()
        .valign(Align::Center)
        .active(
            state
                .config
                .borrow()
                .as_ref()
                .and_then(|c| c.boolean(&[WAYBAR_RULE, "background-effect"], "blur"))
                .unwrap_or(false),
        )
        .build();
    {
        let state = state.clone();
        blur.connect_active_notify(move |s| {
            if let Some(config) = state.config.borrow_mut().as_mut() {
                config.set_boolean(&[WAYBAR_RULE, "background-effect"], "blur", s.is_active());
            }
            state.touch();
        });
    }
    card.append(&row(
        "Blur behind",
        "ZEN blurs what is behind the bar, which GTK cannot do for itself. Costs a \
         blur pass across the whole strip",
        blur.upcast(),
    ));

    card.upcast()
}

fn pill_css_path() -> PathBuf {
    config_path().with_file_name("zen-pills.css")
}

fn current_opacity() -> f64 {
    let Ok(text) = std::fs::read_to_string(pill_css_path()) else {
        return DEFAULT_FILL;
    };
    text.lines()
        .find(|l| l.contains("pill-background"))
        .and_then(|l| l.rsplit(',').next())
        .and_then(|tail| {
            let digits: String = tail
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            digits.parse().ok()
        })
        .unwrap_or(DEFAULT_FILL)
}

fn write_pill_css(state: &Rc<App>, opacity: f64) {
    let opacity = opacity.clamp(0., 1.);
    let border = 0.09 + 0.06 * opacity;

    let text = format!(
        "/* Written by Settings > Status bar. Everything else about the bar lives in\n\
           style.css, which is never touched from here. */\n\n\
         @define-color pill-background rgba(14, 16, 20, {opacity:.2});\n\
         @define-color pill-border rgba(255, 255, 255, {border:.2});\n"
    );

    let path = pill_css_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(err) = std::fs::write(&path, text) {
        state.say(&format!("could not write {}: {err}", path.display()), "bad");
        return;
    }
    reload(state, "pills updated");
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

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
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
    if !doc.borrow().contains_key("include") {
        doc.borrow_mut().insert(
            "include".into(),
            Value::Array(vec![Value::String(MODULES.into())]),
        );
    }

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

    reload(state, "bar updated");
}

fn reload(state: &Rc<App>, good: &str) {
    let reloaded = Command::new("pkill")
        .args(["-USR2", "-x", "waybar"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if reloaded {
        state.say(good, "good");
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
    fn the_shipped_config_pulls_in_the_module_definitions() {
        let doc = shipped();
        let include = doc
            .get("include")
            .and_then(|v| v.as_array())
            .expect("the shipped config must include the module file");
        assert!(
            include.iter().any(|v| v.as_str().is_some_and(|s| s.ends_with("zen-modules.jsonc"))),
            "the include does not name zen-modules.jsonc: {include:?}"
        );
    }

    #[test]
    fn every_listed_module_is_defined() {
        let doc = shipped();
        let modules: serde_json::Value =
            serde_json::from_str(&strip_comments(include_str!(
                "../../resources/waybar/zen-modules.jsonc"
            )))
            .expect("zen-modules.jsonc must be valid JSON");

        for region in [Region::Left, Region::Centre, Region::Right] {
            for name in region_list(&doc, region) {
                assert!(
                    modules.get(&name).is_some(),
                    "the bar lists {name} but nothing defines it"
                );
            }
        }
    }

    #[test]
    fn every_pill_the_page_offers_is_defined() {
        let modules: serde_json::Value =
            serde_json::from_str(&strip_comments(include_str!(
                "../../resources/waybar/zen-modules.jsonc"
            )))
            .expect("zen-modules.jsonc must be valid JSON");

        for pill in PILLS {
            assert!(
                modules.get(pill.key).is_some(),
                "the page offers {} but nothing defines it",
                pill.key
            );
        }
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
