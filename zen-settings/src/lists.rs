use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{glib, Align, Orientation};

use crate::App;

pub fn binds_page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    heading(
        &column,
        "Keybinds",
        "Every bind in your config. Click a chord and press the keys you want; a bind \
         that launches something also lets you change what it launches.",
    );

    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search binds, actions or commands")
        .hexpand(true)
        .build();
    search.add_css_class("field");
    column.append(&search);

    let list = gtk::Box::new(Orientation::Vertical, 12);
    let groups = build_binds(state, &list);
    column.append(&list);

    {
        let groups = groups.clone();
        search.connect_search_changed(move |entry| {
            let needle = entry.text().to_lowercase();
            for group in groups.iter() {
                let mut any = false;
                for (haystack, row) in &group.rows {
                    let shown = needle.is_empty() || haystack.contains(&needle);
                    row.set_visible(shown);
                    any |= shown;
                }
                group.holder.set_visible(any);
                if !needle.is_empty() && any {
                    group.expander.set_expanded(true);
                }
            }
        });
    }

    scrolled(&column)
}

struct BindGroup {
    holder: gtk::Widget,
    expander: gtk::Expander,
    rows: Vec<(String, gtk::Widget)>,
}

fn build_binds(state: &Rc<App>, list: &gtk::Box) -> Rc<Vec<BindGroup>> {
    let binds = match state.config.borrow().as_ref() {
        Some(config) => config.binds(),
        None => Vec::new(),
    };

    if binds.is_empty() {
        list.append(
            &gtk::Label::builder()
                .label("No binds found in this config.")
                .halign(Align::Start)
                .css_classes(["setting-hint"])
                .build(),
        );
        return Rc::new(Vec::new());
    }

    let (spawns, rest): (Vec<_>, Vec<_>) = binds
        .into_iter()
        .partition(|b| b.action.starts_with("spawn"));

    let mut groups = Vec::new();
    if !spawns.is_empty() {
        groups.push(group(list, "APPLICATIONS", spawns, state, true, true));
    }
    if !rest.is_empty() {
        groups.push(group(list, "EVERYTHING ELSE", rest, state, false, false));
    }
    Rc::new(groups)
}

fn group(
    list: &gtk::Box,
    title: &str,
    entries: Vec<crate::config::Entry>,
    state: &Rc<App>,
    editable_command: bool,
    open: bool,
) -> BindGroup {
    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    let mut rows = Vec::new();
    for (i, entry) in entries.into_iter().enumerate() {
        if i > 0 {
            let sep = gtk::Box::new(Orientation::Horizontal, 0);
            sep.add_css_class("row-sep");
            card.append(&sep);
        }

        let haystack = format!(
            "{} {} {} {}",
            entry.key,
            entry.action,
            entry.args.join(" "),
            entry.title.clone().unwrap_or_default()
        )
        .to_lowercase();

        let row = bind_row(state, entry, editable_command);
        card.append(&row);
        rows.push((haystack, row));
    }

    let label = gtk::Label::builder()
        .label(format!("{title}   {}", rows.len()))
        .halign(Align::Start)
        .css_classes(["group-label"])
        .build();

    let expander = gtk::Expander::builder()
        .label_widget(&label)
        .expanded(open)
        .child(&card)
        .build();
    expander.add_css_class("group");

    let holder = gtk::Box::new(Orientation::Vertical, 4);
    holder.append(&expander);
    list.append(&holder);

    BindGroup {
        holder: holder.upcast(),
        expander,
        rows,
    }
}

fn bind_row(state: &Rc<App>, entry: crate::config::Entry, editable_command: bool) -> gtk::Widget {
    let what = entry
        .title
        .clone()
        .unwrap_or_else(|| pretty_action(&entry.action, &entry.args));

    let text = gtk::Box::new(Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(
        &gtk::Label::builder()
            .label(&what)
            .halign(Align::Start)
            .css_classes(["setting-label"])
            .build(),
    );
    text.append(
        &gtk::Label::builder()
            .label(entry.action.clone())
            .halign(Align::Start)
            .css_classes(["setting-hint"])
            .build(),
    );

    let line = gtk::Box::new(Orientation::Horizontal, 10);
    line.add_css_class("setting");
    line.append(&text);

    if editable_command && !entry.args.is_empty() {
        let command = gtk::Entry::builder()
            .text(entry.args.join(" "))
            .width_chars(22)
            .valign(Align::Center)
            .css_classes(["field"])
            .build();

        let state = state.clone();
        let key = entry.key.clone();
        command.connect_activate(move |field| {
            let args: Vec<String> = field
                .text()
                .split_whitespace()
                .map(str::to_owned)
                .collect();

            let result = state
                .config
                .borrow_mut()
                .as_mut()
                .map(|c| c.set_bind_args(&key, &args));

            match result {
                Some(Err(err)) => state.say(&format!("{err}"), "bad"),
                _ => state.touch(),
            }
        });
        line.append(&command);
    }

    line.append(&chord_button(state, &entry.key));
    line.upcast()
}

fn chord_button(state: &Rc<App>, key: &str) -> gtk::Widget {
    let button = gtk::Button::builder()
        .label(key)
        .valign(Align::Center)
        .width_request(170)
        .css_classes(["flat", "chord"])
        .build();

    let current = Rc::new(RefCell::new(key.to_owned()));
    let recording = Rc::new(Cell::new(false));

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let button = button.clone();
        let state = state.clone();
        let current = current.clone();
        let recording = recording.clone();
        keys.connect_key_pressed(move |_, keyval, _, modifiers| {
            if !recording.get() {
                return glib::Propagation::Proceed;
            }

            if is_modifier(keyval) {
                return glib::Propagation::Stop;
            }

            let stop = || glib::Propagation::Stop;

            if keyval == gtk::gdk::Key::Escape {
                recording.set(false);
                button.set_label(&current.borrow());
                button.remove_css_class("recording");
                return stop();
            }

            let Some(name) = keyval.name() else {
                return stop();
            };
            let wanted = chord_string(
                &name,
                modifiers.contains(gtk::gdk::ModifierType::SUPER_MASK),
                modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK),
                modifiers.contains(gtk::gdk::ModifierType::ALT_MASK),
                modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK),
            );

            recording.set(false);
            button.remove_css_class("recording");

            let from = current.borrow().clone();
            if wanted == from {
                button.set_label(&from);
                return stop();
            }

            let result = state
                .config
                .borrow_mut()
                .as_mut()
                .map(|c| c.rebind(&from, &wanted));

            match result {
                Some(Err(err)) => {
                    state.say(&format!("{err}"), "bad");
                    button.set_label(&from);
                }
                _ => {
                    *current.borrow_mut() = wanted.clone();
                    button.set_label(&wanted);
                    state.touch();
                }
            }
            stop()
        });
    }
    button.add_controller(keys);

    {
        let recording = recording.clone();
        button.connect_clicked(move |b| {
            recording.set(true);
            b.set_label("press keys, Esc to cancel");
            b.add_css_class("recording");
            b.grab_focus();
        });
    }

    button.upcast()
}

fn is_modifier(key: gtk::gdk::Key) -> bool {
    use gtk::gdk::Key;
    matches!(
        key,
        Key::Shift_L
            | Key::Shift_R
            | Key::Control_L
            | Key::Control_R
            | Key::Alt_L
            | Key::Alt_R
            | Key::Super_L
            | Key::Super_R
            | Key::Meta_L
            | Key::Meta_R
            | Key::ISO_Level3_Shift
            | Key::Caps_Lock
            | Key::Num_Lock
    )
}

fn chord_string(keyname: &str, sup: bool, ctrl: bool, alt: bool, shift: bool) -> String {
    let mut parts = Vec::new();
    if sup {
        parts.push("Mod");
    }
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }

    let mut chord = parts.join("+");
    if !chord.is_empty() {
        chord.push('+');
    }
    chord.push_str(&pretty_key(keyname));
    chord
}

fn pretty_key(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {
            first.to_uppercase().collect::<String>() + chars.as_str()
        }
        _ => name.to_owned(),
    }
}

fn pretty_action(action: &str, args: &[String]) -> String {
    if args.is_empty() {
        action.replace('-', " ")
    } else {
        args.join(" ")
    }
}

// startup
pub fn startup_page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    heading(
        &column,
        "Startup",
        "Commands ZEN runs when a session begins. One per line, in the order they run.",
    );

    let commands = match state.config.borrow().as_ref() {
        Some(config) => config.startup(),
        None => Vec::new(),
    };

    let text = commands
        .iter()
        .map(|c| c.join(" "))
        .collect::<Vec<_>>()
        .join("\n");

    let buffer = gtk::TextBuffer::builder().text(&text).build();
    let view = gtk::TextView::builder()
        .buffer(&buffer)
        .monospace(true)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(14)
        .right_margin(14)
        .build();
    view.add_css_class("editor");

    let frame = gtk::ScrolledWindow::builder()
        .height_request(220)
        .child(&view)
        .build();
    frame.add_css_class("card");
    column.append(&frame);

    let apply = gtk::Button::builder()
        .label("Apply")
        .halign(Align::End)
        .css_classes(["flat"])
        .build();

    {
        let state = state.clone();
        let buffer = buffer.clone();
        apply.connect_clicked(move |_| {
            let (start, end) = buffer.bounds();
            let text = buffer.text(&start, &end, false);

            let commands: Vec<Vec<String>> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| l.split_whitespace().map(str::to_owned).collect())
                .collect();

            if let Some(config) = state.config.borrow_mut().as_mut() {
                config.set_startup(&commands);
            }
            state.touch();
        });
    }
    column.append(&apply);

    column.append(
        &gtk::Label::builder()
            .label("zen-wallpaper restore is what puts your wallpaper back after a reboot.")
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.)
            .css_classes(["setting-hint"])
            .build(),
    );

    scrolled(&column)
}

// util
fn heading(column: &gtk::Box, title: &str, blurb: &str) {
    column.append(
        &gtk::Label::builder()
            .label(title)
            .halign(Align::Start)
            .css_classes(["page-title"])
            .build(),
    );
    column.append(
        &gtk::Label::builder()
            .label(blurb)
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.)
            .max_width_chars(78)
            .css_classes(["page-blurb"])
            .build(),
    );
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

    #[test]
    fn a_chord_reads_the_way_the_shipped_config_writes_them() {
        assert_eq!(chord_string("l", true, false, true, false), "Mod+Alt+L");
        assert_eq!(chord_string("Escape", true, false, false, true), "Mod+Shift+Escape");
        assert_eq!(chord_string("comma", true, false, false, false), "Mod+Comma");
        assert_eq!(chord_string("r", true, true, false, true), "Mod+Ctrl+Shift+R");
    }

    #[test]
    fn a_chord_with_no_modifiers_is_just_the_key() {
        assert_eq!(chord_string("Print", false, false, false, false), "Print");
        assert!(!chord_string("Print", false, false, false, false).starts_with('+'));
    }

    #[test]
    fn key_names_keep_the_capitalisation_xkb_gave_them() {
        assert_eq!(pretty_key("XF86AudioRaiseVolume"), "XF86AudioRaiseVolume");
        assert_eq!(pretty_key("F1"), "F1");
        assert_eq!(pretty_key("1"), "1");
        assert_eq!(pretty_key("period"), "Period");
    }

    #[test]
    fn every_chord_it_builds_parses_as_a_bind() {
        let cases = [
            chord_string("l", true, false, true, false),
            chord_string("Escape", true, false, false, true),
            chord_string("comma", true, false, false, false),
            chord_string("Print", false, false, false, false),
            chord_string("XF86AudioRaiseVolume", false, false, false, false),
            chord_string("slash", true, true, true, true),
        ];

        for chord in cases {
            let text = format!("binds {{\n    {chord} {{ close-window; }}\n}}\n");
            zen_config::Config::parse_mem(&text)
                .unwrap_or_else(|e| panic!("{chord} does not parse as a bind: {e}"));
        }
    }
}
