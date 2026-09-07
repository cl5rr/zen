use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::monitors::{row, separator};
use crate::require::{self, Need, Probe};
use crate::App;

const NEEDS: &[Need] = &[
    Need {
        command: "ly",
        what: "the login manager this page configures",
        package: "ly",
        probe: Probe::AnyFile(&[
            "/etc/ly/config.ini",
            "/usr/bin/ly",
            "/usr/lib/systemd/system/ly.service",
            "/usr/lib/systemd/system/ly@.service",
        ]),
    },
    Need {
        command: "pkexec",
        what: "how a desktop app is allowed to write /etc",
        package: "polkit",
        probe: Probe::OnPath,
    },
    Need {
        command: "polkit agent",
        what: "what draws the password prompt. Without one pkexec tries to ask on a \
               terminal, and a settings window has none",
        package: "polkit-gnome",
        probe: Probe::AnyFile(&[
            "/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1",
            "/usr/libexec/polkit-gnome-authentication-agent-1",
            "/usr/lib/polkit-kde-authentication-agent-1",
            "/usr/libexec/polkit-kde-authentication-agent-1",
            "/usr/lib/mate-polkit/polkit-mate-authentication-agent-1",
            "/usr/libexec/polkit-mate-authentication-agent-1",
            "/usr/lib/xfce-polkit/xfce-polkit",
            "/usr/bin/lxqt-policykit-agent",
            "/usr/bin/lxpolkit",
        ]),
    },
];

struct Field {
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    kind: Kind,
}

enum Kind {
    Flag,
    Text(&'static str),
    Choice(&'static [&'static str]),
    Number(f64, f64),
    Colour,
}

const TERMINAL_COLOURS: [(&str, &str); 16] = [
    ("Black", "#1a1d24"),
    ("Red", "#e06c75"),
    ("Green", "#8fd18a"),
    ("Yellow", "#e5c07b"),
    ("Blue", "#7fc8ff"),
    ("Magenta", "#c39ee0"),
    ("Cyan", "#6fd2d2"),
    ("Light grey", "#c7ccd4"),
    ("Dark grey", "#3a3f4b"),
    ("Bright red", "#ff8b93"),
    ("Bright green", "#a9e0a3"),
    ("Bright yellow", "#f2d3a0"),
    ("Bright blue", "#a3d9ff"),
    ("Bright magenta", "#d6b8ee"),
    ("Bright cyan", "#92e2e2"),
    ("White", "#f0f2f5"),
];

const FIELDS: &[Field] = &[
    Field {
        key: "animation",
        label: "Background",
        hint: "What plays behind the login box",
        kind: Kind::Choice(&["none", "doom", "matrix", "colormix"]),
    },
    Field {
        key: "box_title",
        label: "Title",
        hint: "Written above the login box",
        kind: Kind::Text("ZEN"),
    },
    Field {
        key: "clock",
        label: "Clock",
        hint: "A strftime format, for example %H:%M. Empty for no clock",
        kind: Kind::Text("%H:%M"),
    },
    Field {
        key: "blank_box",
        label: "Blank the box",
        hint: "Clears the fields when you switch between them",
        kind: Kind::Flag,
    },
    Field {
        key: "clear_password",
        label: "Clear the password on failure",
        hint: "",
        kind: Kind::Flag,
    },
    Field {
        key: "hide_borders",
        label: "Hide the borders",
        hint: "",
        kind: Kind::Flag,
    },
    Field {
        key: "numlock",
        label: "Num lock on",
        hint: "Turned on when the login screen appears",
        kind: Kind::Flag,
    },
    Field {
        key: "save",
        label: "Remember the last user",
        hint: "Fills the name in next time",
        kind: Kind::Flag,
    },
    Field {
        key: "vi_mode",
        label: "Vi keys",
        hint: "hjkl to move between the fields",
        kind: Kind::Flag,
    },
    Field {
        key: "bg",
        label: "Background colour",
        hint: "Ly draws in a terminal, so it can only use these sixteen",
        kind: Kind::Colour,
    },
    Field {
        key: "fg",
        label: "Text colour",
        hint: "",
        kind: Kind::Colour,
    },
    Field {
        key: "border_fg",
        label: "Border colour",
        hint: "",
        kind: Kind::Colour,
    },
];

// page
pub fn page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    column.append(
        &gtk::Label::builder()
            .label("Login screen")
            .halign(Align::Start)
            .css_classes(["page-title"])
            .build(),
    );
    column.append(
        &gtk::Label::builder()
            .label(
                "Ly, the login manager ZEN's installer sets up. Its config belongs to \
                 root, so saving asks for your password.",
            )
            .halign(Align::Start)
            .xalign(0.)
            .wrap(true)
            .css_classes(["page-blurb"])
            .build(),
    );

    let values = read().unwrap_or_default();
    let readable = !values.is_empty();

    let pending: Rc<RefCell<HashMap<String, String>>> = Rc::new(RefCell::new(HashMap::new()));

    let body = gtk::Box::new(Orientation::Vertical, 10);

    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    for (i, field) in FIELDS.iter().enumerate() {
        if i > 0 {
            separator(&card);
        }
        card.append(&field_row(field, &values, &pending));
    }
    body.append(&card);

    let apply = gtk::Button::builder()
        .label("Save to /etc/ly")
        .halign(Align::End)
        .css_classes(["flat"])
        .build();
    {
        let state = state.clone();
        let pending = pending.clone();
        apply.connect_clicked(move |_| write(&state, &pending));
    }

    let theme = gtk::Button::builder()
        .label("Use ZEN's colours")
        .css_classes(["flat"])
        .build();
    {
        let state = state.clone();
        theme.connect_clicked(move |_| {
            run_privileged(&state, &["theme".to_owned()], "login screen themed");
        });
    }

    let buttons = gtk::Box::new(Orientation::Horizontal, 8);
    buttons.set_margin_top(4);
    buttons.append(&theme);
    let spacer = gtk::Box::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    buttons.append(&spacer);
    buttons.append(&apply);
    body.append(&buttons);

    let blocked = require::guard(&column, &body.clone().upcast(), NEEDS);
    if !blocked && !readable {
        column.append(
            &gtk::Label::builder()
                .label(format!(
                    "Ly is installed but {} could not be read. Run ./setup.sh and \
                     choose the Ly greeter to create it.",
                    config_path()
                ))
                .halign(Align::Start)
                .wrap(true)
                .xalign(0.)
                .css_classes(["setting-hint"])
                .build(),
        );
        body.set_sensitive(false);
    }
    column.append(&body);

    column.append(
        &gtk::Label::builder()
            .label(config_path())
            .halign(Align::Start)
            .selectable(true)
            .css_classes(["mono"])
            .build(),
    );

    scrolled(&column)
}

fn field_row(
    field: &'static Field,
    values: &HashMap<String, String>,
    pending: &Rc<RefCell<HashMap<String, String>>>,
) -> gtk::Widget {
    let current = values.get(field.key).cloned().unwrap_or_default();

    let control: gtk::Widget = match &field.kind {
        Kind::Flag => {
            let on = gtk::Switch::builder()
                .valign(Align::Center)
                .active(current == "true")
                .build();
            let pending = pending.clone();
            on.connect_active_notify(move |s| {
                pending
                    .borrow_mut()
                    .insert(field.key.to_owned(), s.is_active().to_string());
            });
            on.upcast()
        }

        Kind::Text(placeholder) => {
            let entry = gtk::Entry::builder()
                .text(&current)
                .placeholder_text(*placeholder)
                .width_chars(14)
                .valign(Align::Center)
                .css_classes(["field"])
                .build();
            let pending = pending.clone();
            entry.connect_changed(move |e| {
                pending
                    .borrow_mut()
                    .insert(field.key.to_owned(), e.text().to_string());
            });
            entry.upcast()
        }

        Kind::Choice(options) => {
            let model = gtk::StringList::new(options);
            let picked = options.iter().position(|o| *o == current).unwrap_or(0);
            let drop = gtk::DropDown::builder()
                .model(&model)
                .selected(picked as u32)
                .valign(Align::Center)
                .build();
            let pending = pending.clone();
            let options = *options;
            drop.connect_selected_notify(move |d| {
                if let Some(choice) = options.get(d.selected() as usize) {
                    pending
                        .borrow_mut()
                        .insert(field.key.to_owned(), (*choice).to_owned());
                }
            });
            drop.upcast()
        }

        Kind::Colour => {
            let picked = current.parse::<usize>().unwrap_or(0).min(15);

            let swatch = gtk::DrawingArea::new();
            swatch.set_size_request(26, 20);
            swatch.set_valign(Align::Center);
            swatch.add_css_class("swatch");

            let shown = Rc::new(std::cell::Cell::new(picked));
            {
                let shown = shown.clone();
                swatch.set_draw_func(move |_, cr, w, h| {
                    let (r, g, b) = rgb_of(shown.get());
                    cr.set_source_rgb(r, g, b);
                    let _ = cr.paint();
                    let _ = h;
                    let _ = w;
                });
            }

            let names: Vec<&str> = TERMINAL_COLOURS.iter().map(|(n, _)| *n).collect();
            let drop = gtk::DropDown::builder()
                .model(&gtk::StringList::new(&names))
                .selected(picked as u32)
                .valign(Align::Center)
                .build();

            {
                let pending = pending.clone();
                let swatch = swatch.clone();
                let shown = shown.clone();
                drop.connect_selected_notify(move |d| {
                    let idx = d.selected() as usize;
                    shown.set(idx.min(15));
                    swatch.queue_draw();
                    pending
                        .borrow_mut()
                        .insert(field.key.to_owned(), idx.to_string());
                });
            }

            let line = gtk::Box::new(Orientation::Horizontal, 8);
            line.set_valign(Align::Center);
            line.append(&swatch);
            line.append(&drop);
            line.upcast()
        }

        Kind::Number(min, max) => {
            let spin = gtk::SpinButton::with_range(*min, *max, 1.);
            spin.set_value(current.parse().unwrap_or(*min));
            spin.set_valign(Align::Center);
            spin.set_width_chars(4);
            let pending = pending.clone();
            spin.connect_value_changed(move |s| {
                pending
                    .borrow_mut()
                    .insert(field.key.to_owned(), (s.value().round() as i64).to_string());
            });
            spin.upcast()
        }
    };

    row(field.label, field.hint, control)
}

// zen-ly
fn config_path() -> String {
    Command::new("zen-ly")
        .arg("path")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "/etc/ly/config.ini".to_owned())
}

fn read() -> Option<HashMap<String, String>> {
    let out = Command::new("zen-ly").arg("get").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            .collect(),
    )
}

fn write(state: &Rc<App>, pending: &Rc<RefCell<HashMap<String, String>>>) {
    let changes: Vec<String> = pending
        .borrow()
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();

    if changes.is_empty() {
        state.say("nothing changed", "");
        return;
    }

    let mut args = vec!["set".to_owned()];
    args.extend(changes);
    if run_privileged(state, &args, "login screen saved") {
        pending.borrow_mut().clear();
    }
}

fn run_privileged(state: &Rc<App>, args: &[String], good: &str) -> bool {
    let Ok(out) = Command::new("pkexec").arg("zen-ly").args(args).output() else {
        state.say("pkexec is not installed, so this cannot write /etc", "bad");
        return false;
    };

    if out.status.success() {
        state.say(good, "good");
        return true;
    }

    let why = String::from_utf8_lossy(&out.stderr);

    if why.contains("/dev/tty") || why.contains("textual authentication agent") {
        state.say(
            "no polkit agent is running, so nothing can ask for your password. Start \
             zen-polkit, or install polkit-gnome",
            "bad",
        );
        return false;
    }

    let why = why.lines().last().unwrap_or("").trim();
    if why.is_empty() {
        state.say("not saved", "");
    } else {
        state.say(why, "bad");
    }
    false
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

fn rgb_of(index: usize) -> (f64, f64, f64) {
    let hex = TERMINAL_COLOURS.get(index).map(|(_, h)| *h).unwrap_or("#000000");
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    (
        f64::from((value >> 16) & 0xff) / 255.,
        f64::from((value >> 8) & 0xff) / 255.,
        f64::from(value & 0xff) / 255.,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_terminal_colour_parses_to_a_channel_in_range() {
        for index in 0..16 {
            let (r, g, b) = rgb_of(index);
            for channel in [r, g, b] {
                assert!(
                    (0. ..=1.).contains(&channel),
                    "colour {index} gave {channel}"
                );
            }
        }
    }

    #[test]
    fn an_out_of_range_index_is_still_drawable() {
        let (r, g, b) = rgb_of(99);
        assert_eq!((r, g, b), (0., 0., 0.));
    }

    #[test]
    fn the_names_are_in_terminal_colour_order() {
        assert_eq!(TERMINAL_COLOURS[0].0, "Black");
        assert_eq!(TERMINAL_COLOURS[1].0, "Red");
        assert_eq!(TERMINAL_COLOURS[15].0, "White");
        assert_eq!(TERMINAL_COLOURS.len(), 16);
    }

    #[test]
    fn the_tty_is_not_settable_from_here() {
        assert!(
            !FIELDS.iter().any(|f| f.key == "tty"),
            "changing Ly's TTY from a settings app can only lock someone out"
        );
    }
}
