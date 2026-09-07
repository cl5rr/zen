use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::monitors::{row, separator};
use crate::require::{self, Need};
use crate::App;

const NEEDS: &[Need] = &[
    Need {
        command: "ly",
        what: "the login manager this page configures",
        package: "ly",
    },
    Need {
        command: "pkexec",
        what: "how a desktop app is allowed to write /etc",
        package: "polkit",
    },
];

// Ly, the login screen you see before ZEN starts.
//
// Its config lives in /etc and is owned by root, so nothing here writes it directly:
// every change shells out to `pkexec zen-ly set key=value`, and zen-ly is the only
// privileged part. It refuses any key not on its own list, so a bug on this page cannot
// turn into an arbitrary write to /etc as root.
//
// Reading needs no privilege, so the page still shows the current state on a machine
// with no polkit agent; only saving asks.

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
}

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
        key: "tty",
        label: "TTY",
        hint: "Which virtual terminal Ly runs on. ZEN's installer uses 2",
        kind: Kind::Number(1., 12.),
    },
    Field {
        key: "bg",
        label: "Background colour",
        hint: "An ncurses colour number, 0 to 15. Ly cannot take hex",
        kind: Kind::Number(0., 15.),
    },
    Field {
        key: "fg",
        label: "Text colour",
        hint: "",
        kind: Kind::Number(0., 15.),
    },
    Field {
        key: "border_fg",
        label: "Border colour",
        hint: "",
        kind: Kind::Number(0., 15.),
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

    // The controls are built either way. Showing the page you would get, greyed, says
    // far more about what installing Ly buys you than an empty page with one sentence.
    let values = read().unwrap_or_default();
    let readable = !values.is_empty();

    // Edits collect here and go out in one pkexec call, because asking for a password
    // per switch would make the page unusable.
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
        // Ly is installed but its config could not be read, which is a different
        // problem from not having it and needs saying differently.
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
    // pkexec puts up the password prompt. Without it there is no way to write /etc from
    // a desktop app, so the page says what to run by hand instead of failing silently.
    let Ok(out) = Command::new("pkexec").arg("zen-ly").args(args).output() else {
        state.say("pkexec is not installed, so this cannot write /etc", "bad");
        return false;
    };

    if out.status.success() {
        state.say(good, "good");
        return true;
    }

    let why = String::from_utf8_lossy(&out.stderr);
    let why = why.lines().last().unwrap_or("").trim();
    if why.is_empty() {
        // Cancelling the password prompt is not an error worth shouting about.
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
