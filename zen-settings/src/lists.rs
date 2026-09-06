use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::App;

// keybinds
pub fn binds_page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    heading(
        &column,
        "Keybinds",
        "Every bind in your config. Click a chord to rebind it; a bind that launches \
         something also lets you change what it launches.",
    );

    let list = gtk::Box::new(Orientation::Vertical, 18);
    rebuild_binds(state, &list);
    column.append(&list);

    scrolled(&column)
}

fn rebuild_binds(state: &Rc<App>, list: &gtk::Box) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

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
        return;
    }

    // Binds that launch something are the ones people actually want to change, so
    // they come first instead of being buried in a hundred layout actions.
    let (spawns, rest): (Vec<_>, Vec<_>) = binds
        .into_iter()
        .partition(|b| b.action.starts_with("spawn"));

    if !spawns.is_empty() {
        group(list, "APPLICATIONS", spawns, state, true);
    }
    if !rest.is_empty() {
        group(list, "EVERYTHING ELSE", rest, state, false);
    }
}

fn group(
    list: &gtk::Box,
    title: &str,
    entries: Vec<crate::config::Entry>,
    state: &Rc<App>,
    editable_command: bool,
) {
    let holder = gtk::Box::new(Orientation::Vertical, 4);
    holder.append(
        &gtk::Label::builder()
            .label(title)
            .halign(Align::Start)
            .css_classes(["group-label"])
            .build(),
    );

    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    for (i, entry) in entries.into_iter().enumerate() {
        if i > 0 {
            let sep = gtk::Box::new(Orientation::Horizontal, 0);
            sep.add_css_class("row-sep");
            card.append(&sep);
        }
        card.append(&bind_row(state, entry, editable_command));
    }

    holder.append(&card);
    list.append(&holder);
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

    let chord = gtk::Entry::builder()
        .text(&entry.key)
        .width_chars(16)
        .valign(Align::Center)
        .xalign(0.5)
        .css_classes(["field", "chord"])
        .build();

    let state = state.clone();
    let was = entry.key.clone();
    let previous = std::cell::RefCell::new(entry.key.clone());
    chord.connect_activate(move |field| {
        let wanted = field.text().trim().to_owned();
        if wanted.is_empty() {
            field.set_text(&previous.borrow());
            return;
        }

        let from = previous.borrow().clone();
        let result = state
            .config
            .borrow_mut()
            .as_mut()
            .map(|c| c.rebind(&from, &wanted));

        match result {
            Some(Err(err)) => {
                state.say(&format!("{err}"), "bad");
                field.set_text(&from);
            }
            _ => {
                *previous.borrow_mut() = wanted;
                state.touch();
            }
        }
    });
    let _ = was;
    line.append(&chord);

    line.upcast()
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
