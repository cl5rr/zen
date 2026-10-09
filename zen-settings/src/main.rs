mod bar;
mod config;
mod greeter;
mod lists;
mod monitors;
mod require;
mod spec;
mod style;
mod wallpaper;

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk::{Display, RGBA};
use gtk::glib;
use gtk::{Align, Orientation};

use config::Config;
use spec::{Kind, Page, PAGES};

const APP_ID: &str = "org.zen.Settings";

// state
pub struct App {
    pub config: RefCell<Option<Config>>,
    status: gtk::Label,
    pending: RefCell<u64>,
}

impl App {
    pub fn touch(self: &Rc<Self>) {
        let mut pending = self.pending.borrow_mut();
        *pending += 1;
        let generation = *pending;
        drop(pending);

        let this = self.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(90), move || {
            if *this.pending.borrow() != generation {
                return;
            }
            this.commit();
        });
    }

    fn commit(&self) {
        let borrow = self.config.borrow();
        let Some(config) = borrow.as_ref() else {
            return;
        };

        match config.save() {
            Ok(()) => self.say("saved", "good"),
            Err(err) => self.say(&format!("{err}"), "bad"),
        }
    }

    pub fn say(&self, text: &str, class: &str) {
        self.status.set_text(text);
        self.status.set_css_classes(&["status", class]);
    }
}

// init
fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build);
    app.run_with_args::<&str>(&[])
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));

    if let Some(display) = Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build(app: &adw::Application) {
    let status = gtk::Label::builder()
        .label("")
        .halign(Align::End)
        .css_classes(["status"])
        .build();

    let loaded = Config::load();
    let failure = loaded.as_ref().err().map(|e| e.to_string());

    let state = Rc::new(App {
        config: RefCell::new(loaded.ok()),
        status: status.clone(),
        pending: RefCell::new(0),
    });

    if let Some(why) = &failure {
        state.say(why, "bad");
    } else {
        state.say("ready", "");
    }

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(140)
        .hexpand(true)
        .build();

    stack.add_named(&style::page(&state), Some("Style"));
    for page in PAGES {
        stack.add_named(&build_page(&state, page), Some(page.name));
    }
    stack.add_named(&monitors::page(&state), Some("Monitors"));
    stack.add_named(&lists::binds_page(&state), Some("Keybinds"));
    stack.add_named(&lists::startup_page(&state), Some("Startup apps"));
    stack.add_named(&bar::page(&state), Some("Status bar"));
    stack.add_named(&greeter::page(&state), Some("Login screen"));
    stack.add_named(&wallpaper::page(&state), Some("Wallpaper"));

    let sidebar = build_sidebar(&stack);

    let header = adw::HeaderBar::builder()
        .css_classes(["zen"])
        .title_widget(&gtk::Label::builder().label("ZEN").css_classes(["title"]).build())
        .build();

    let split = gtk::Box::new(Orientation::Horizontal, 0);
    split.set_vexpand(true);
    split.append(&sidebar);
    split.append(&stack);

    let where_ = gtk::Label::builder()
        .label(config::config_path().display().to_string())
        .halign(Align::Start)
        .hexpand(true)
        .css_classes(["mono"])
        .build();

    let footer = gtk::Box::new(Orientation::Horizontal, 12);
    footer.add_css_class("footer");
    footer.append(&where_);
    footer.append(&status);

    let root = gtk::Box::new(Orientation::Vertical, 0);
    root.append(&header);
    root.append(&split);
    root.append(&footer);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("ZEN Settings")
        .default_width(980)
        .default_height(700)
        .content(&root)
        .build();
    window.add_css_class("zen");
    window.present();
}

// sidebar
fn build_sidebar(stack: &gtk::Stack) -> gtk::Widget {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();

    let mut names: Vec<&str> = vec!["Style"];
    names.extend(PAGES.iter().map(|p| p.name));
    names.push("Monitors");
    names.push("Keybinds");
    names.push("Startup apps");
    names.push("Status bar");
    names.push("Login screen");
    names.push("Wallpaper");

    for name in &names {
        let label = gtk::Label::builder()
            .label(*name)
            .halign(Align::Start)
            .build();
        list.append(&gtk::ListBoxRow::builder().child(&label).build());
    }

    let stack = stack.clone();
    let owned: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            if let Some(name) = owned.get(row.index() as usize) {
                stack.set_visible_child_name(name);
            }
        }
    });
    let start = std::env::args()
        .nth(1)
        .and_then(|want| names.iter().position(|n| n.eq_ignore_ascii_case(&want)))
        .unwrap_or(0);
    list.select_row(list.row_at_index(start as i32).as_ref());

    let wordmark = gtk::Label::builder()
        .label("ZEN")
        .halign(Align::Start)
        .css_classes(["wordmark"])
        .build();
    let sub = gtk::Label::builder()
        .label("settings")
        .halign(Align::Start)
        .css_classes(["wordmark-sub"])
        .build();

    let column = gtk::Box::new(Orientation::Vertical, 0);
    column.set_size_request(212, -1);
    column.add_css_class("sidebar");
    column.append(&wordmark);
    column.append(&sub);
    column.append(&list);

    column.upcast()
}

// pages
fn build_page(state: &Rc<App>, page: &'static Page) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    column.append(
        &gtk::Label::builder()
            .label(page.title)
            .halign(Align::Start)
            .css_classes(["page-title"])
            .build(),
    );
    column.append(
        &gtk::Label::builder()
            .label(page.blurb)
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.)
            .css_classes(["page-blurb"])
            .build(),
    );

    for group in page.groups {
        column.append(
            &gtk::Label::builder()
                .label(group.title)
                .halign(Align::Start)
                .css_classes(["group-label"])
                .build(),
        );

        let card = gtk::Box::new(Orientation::Vertical, 0);
        card.add_css_class("card");

        for (i, row) in group.rows.iter().enumerate() {
            if i > 0 {
                let sep = gtk::Box::new(Orientation::Horizontal, 0);
                sep.add_css_class("row-sep");
                card.append(&sep);
            }
            card.append(&build_row(state, row));
        }
        column.append(&card);
    }

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&column)
        .hexpand(true)
        .vexpand(true)
        .build()
        .upcast()
}

fn build_row(state: &Rc<App>, row: &'static spec::Row) -> gtk::Widget {
    let text = gtk::Box::new(Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(
        &gtk::Label::builder()
            .label(row.label)
            .halign(Align::Start)
            .css_classes(["setting-label"])
            .build(),
    );
    if !row.hint.is_empty() {
        text.append(
            &gtk::Label::builder()
                .label(row.hint)
                .halign(Align::Start)
                .wrap(true)
                .xalign(0.)
                .max_width_chars(52)
                .css_classes(["setting-hint"])
                .build(),
        );
    }

    let line = gtk::Box::new(Orientation::Horizontal, 14);
    line.add_css_class("setting");
    line.append(&text);

    match &row.kind {
        Kind::Flag { invert } => {
            let present = state
                .config
                .borrow()
                .as_ref()
                .is_some_and(|c| c.flag(row.path, row.key));
            let switch = gtk::Switch::builder()
                .valign(Align::Center)
                .active(if *invert { !present } else { present })
                .build();

            let state = state.clone();
            let invert = *invert;
            switch.connect_active_notify(move |s| {
                let on = if invert { !s.is_active() } else { s.is_active() };
                if let Some(c) = state.config.borrow_mut().as_mut() {
                    c.set_flag(row.path, row.key, on);
                }
                state.touch();
            });
            line.append(&switch);
        }

        Kind::Bool { default } => {
            let current = state
                .config
                .borrow()
                .as_ref()
                .and_then(|c| c.boolean(row.path, row.key))
                .unwrap_or(*default);
            let switch = gtk::Switch::builder()
                .valign(Align::Center)
                .active(current)
                .build();

            let state = state.clone();
            switch.connect_active_notify(move |s| {
                if let Some(c) = state.config.borrow_mut().as_mut() {
                    c.set_boolean(row.path, row.key, s.is_active());
                }
                state.touch();
            });
            line.append(&switch);
        }

        Kind::Number { min, max, step, digits, default } => {
            let current = state
                .config
                .borrow()
                .as_ref()
                .and_then(|c| c.number(row.path, row.key))
                .unwrap_or(*default);

            let readout = gtk::Label::builder()
                .label(format(current, *digits))
                .halign(Align::End)
                .css_classes(["value"])
                .build();

            let scale = gtk::Scale::with_range(Orientation::Horizontal, *min, *max, *step);
            scale.set_draw_value(false);
            scale.set_value(current);
            scale.set_size_request(240, -1);
            scale.set_valign(Align::Center);

            let state = state.clone();
            let readout2 = readout.clone();
            let digits = *digits;
            scale.connect_value_changed(move |s| {
                let value = s.value();
                readout2.set_label(&format(value, digits));
                if let Some(c) = state.config.borrow_mut().as_mut() {
                    c.set_number(row.path, row.key, value, digits);
                }
                state.touch();
            });

            line.append(&scale);
            line.append(&readout);
        }

        Kind::Prop { prop, min, max, default } => {
            let current = state
                .config
                .borrow()
                .as_ref()
                .and_then(|c| c.prop(row.path, row.key, prop))
                .unwrap_or(*default);

            let readout = gtk::Label::builder()
                .label(format(current, 0))
                .halign(Align::End)
                .css_classes(["value"])
                .build();

            let scale = gtk::Scale::with_range(Orientation::Horizontal, *min, *max, 1.);
            scale.set_draw_value(false);
            scale.set_value(current);
            scale.set_size_request(240, -1);
            scale.set_valign(Align::Center);

            let state = state.clone();
            let readout2 = readout.clone();
            let prop = *prop;
            scale.connect_value_changed(move |s| {
                let value = s.value();
                readout2.set_label(&format(value, 0));
                if let Some(c) = state.config.borrow_mut().as_mut() {
                    c.set_prop(row.path, row.key, prop, value);
                }
                state.touch();
            });

            line.append(&scale);
            line.append(&readout);
        }

        Kind::Text { fallback, hint } => {
            let current = state
                .config
                .borrow()
                .as_ref()
                .and_then(|c| c.string(row.path, row.key))
                .unwrap_or_else(|| (*fallback).to_owned());

            let field = gtk::Entry::builder()
                .text(&current)
                .placeholder_text(*hint)
                .width_chars(18)
                .valign(Align::Center)
                .css_classes(["field"])
                .build();

            let state = state.clone();
            field.connect_activate(move |field| {
                let value = field.text().trim().to_owned();
                if value.is_empty() {
                    return;
                }
                if let Some(c) = state.config.borrow_mut().as_mut() {
                    c.set_string(row.path, row.key, &value);
                }
                state.touch();
            });
            line.append(&field);
        }

        Kind::Color { fallback } => {
            let current = state
                .config
                .borrow()
                .as_ref()
                .and_then(|c| c.string(row.path, row.key))
                .unwrap_or_else(|| (*fallback).to_owned());
            let rgba = RGBA::parse(&current).unwrap_or(RGBA::WHITE);

            let button = gtk::ColorDialogButton::builder()
                .dialog(&gtk::ColorDialog::builder().with_alpha(true).build())
                .rgba(&rgba)
                .valign(Align::Center)
                .build();
            button.add_css_class("color");

            let state = state.clone();
            button.connect_rgba_notify(move |b| {
                let hex = to_hex(b.rgba());
                if let Some(c) = state.config.borrow_mut().as_mut() {
                    c.set_string(row.path, row.key, &hex);
                }
                state.touch();
            });
            line.append(&button);
        }
    }

    line.upcast()
}

// util
fn format(value: f64, digits: usize) -> String {
    if digits == 0 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.digits$}")
    }
}

fn to_hex(c: RGBA) -> String {
    let byte = |x: f32| (x.clamp(0., 1.) * 255.).round() as u8;
    let (r, g, b, a) = (byte(c.red()), byte(c.green()), byte(c.blue()), byte(c.alpha()));
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}
