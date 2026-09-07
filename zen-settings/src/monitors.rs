use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};
use serde_json::Value;

use crate::App;

const CANVAS_H: i32 = 260;

#[derive(Clone)]
pub struct Screen {
    pub name: String,
    pub modes: Vec<String>,
    pub current_mode: Option<usize>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub scale: f64,
    pub is_virtual: bool,
}

fn detect() -> Vec<Screen> {
    let Ok(out) = Command::new("zen").args(["msg", "--json", "outputs"]).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let Ok(json) = serde_json::from_slice::<Value>(&out.stdout) else {
        return Vec::new();
    };
    let Some(map) = json.as_object() else {
        return Vec::new();
    };

    let mut screens: Vec<Screen> = map
        .iter()
        .map(|(name, o)| {
            let modes = o["modes"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .map(|m| {
                            let w = m["width"].as_u64().unwrap_or(0);
                            let h = m["height"].as_u64().unwrap_or(0);
                            let hz = m["refresh_rate"].as_u64().unwrap_or(0) as f64 / 1000.;
                            format!("{w}x{h}@{hz:.3}")
                        })
                        .collect()
                })
                .unwrap_or_default();

            let logical = &o["logical"];
            Screen {
                name: name.clone(),
                modes,
                current_mode: o["current_mode"].as_u64().map(|i| i as usize),
                x: logical["x"].as_f64().unwrap_or(0.),
                y: logical["y"].as_f64().unwrap_or(0.),
                w: logical["width"].as_f64().unwrap_or(1920.),
                h: logical["height"].as_f64().unwrap_or(1080.),
                scale: logical["scale"].as_f64().unwrap_or(1.),
                is_virtual: o["make"].as_str() == Some("ZEN")
                    && o["model"].as_str() == Some("Virtual"),
            }
        })
        .collect();

    screens.sort_by(|a, b| a.name.cmp(&b.name));
    screens
}

// page
pub fn page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    column.append(
        &gtk::Label::builder()
            .label("Monitors")
            .halign(Align::Start)
            .css_classes(["page-title"])
            .build(),
    );

    let screens = detect();

    if screens.is_empty() {
        column.append(
            &gtk::Label::builder()
                .label(
                    "Could not ask the compositor for its outputs. This page reads them \
                     from a running ZEN with `zen msg --json outputs`.",
                )
                .halign(Align::Start)
                .wrap(true)
                .xalign(0.)
                .max_width_chars(72)
                .css_classes(["setting-hint"])
                .build(),
        );
        return scrolled(&column);
    }

    column.append(
        &gtk::Label::builder()
            .label("Drag a screen to place it. Positions are in logical pixels, so a scaled monitor takes the space it looks like it takes.")
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.)
            .max_width_chars(78)
            .css_classes(["page-blurb"])
            .build(),
    );

    let names: Vec<String> = screens.iter().map(|s| s.name.clone()).collect();
    let placed = Rc::new(RefCell::new(screens.clone()));
    column.append(&arrangement(state, &placed));

    for (i, screen) in screens.iter().enumerate() {
        column.append(
            &gtk::Label::builder()
                .label(screen.name.to_uppercase())
                .halign(Align::Start)
                .css_classes(["group-label"])
                .build(),
        );
        column.append(&card(state, screen, i, &names));
    }

    column.append(
        &gtk::Label::builder()
            .label("VIRTUAL MONITORS")
            .halign(Align::Start)
            .css_classes(["group-label"])
            .build(),
    );
    column.append(&virtual_card(state, &screens));

    scrolled(&column)
}

fn virtual_card(state: &Rc<App>, screens: &[Screen]) -> gtk::Widget {
    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    let existing: Vec<&Screen> = screens.iter().filter(|s| s.is_virtual).collect();

    if existing.is_empty() {
        card.append(&row(
            "None right now",
            "A virtual monitor renders offscreen, so a screen share can pick it up              while nothing on your desk shows it",
            gtk::Box::new(Orientation::Horizontal, 0).upcast(),
        ));
    } else {
        for screen in existing {
            let remove = gtk::Button::builder()
                .label("Remove")
                .valign(Align::Center)
                .css_classes(["flat"])
                .build();
            {
                let state = state.clone();
                let name = screen.name.clone();
                remove.connect_clicked(move |_| {
                    run_output(&state, &name, &["destroy"], &format!("removed {name}"));
                });
            }
            card.append(&row(
                &screen.name,
                &format!("{:.0}x{:.0}", screen.w, screen.h),
                remove.upcast(),
            ));
            separator(&card);
        }
    }

    let name = gtk::Entry::builder()
        .placeholder_text("stream")
        .valign(Align::Center)
        .width_request(140)
        .css_classes(["field"])
        .build();
    card.append(&row("Name", "What it will be called in zen msg outputs", name.clone().upcast()));

    separator(&card);

    let width = spin(320., 7680., 1920.);
    let height = spin(240., 4320., 1080.);
    let size = gtk::Box::new(Orientation::Horizontal, 6);
    size.set_valign(Align::Center);
    size.append(&width);
    size.append(&gtk::Label::new(Some("x")));
    size.append(&height);
    card.append(&row("Size", "In pixels", size.upcast()));

    separator(&card);

    let refresh = spin(1., 480., 60.);
    card.append(&row("Refresh", "Hz", refresh.clone().upcast()));

    separator(&card);

    let create = gtk::Button::builder()
        .label("Create")
        .valign(Align::Center)
        .css_classes(["flat"])
        .build();
    {
        let state = state.clone();
        let name = name.clone();
        let width = width.clone();
        let height = height.clone();
        let refresh = refresh.clone();
        create.connect_clicked(move |_| {
            let chosen = name.text().trim().to_owned();
            if chosen.is_empty() {
                state.say("give it a name first", "bad");
                return;
            }
            let mhz = (refresh.value() * 1000.).round() as i64;
            run_output(
                &state,
                &chosen,
                &[
                    "create",
                    "--width",
                    &format!("{}", width.value().round() as i64),
                    "--height",
                    &format!("{}", height.value().round() as i64),
                    "--refresh",
                    &format!("{mhz}"),
                ],
                &format!("created {chosen}"),
            );
        });
    }
    card.append(&row(
        "Add one",
        "It appears immediately, and is gone when the session ends",
        create.upcast(),
    ));

    card.upcast()
}

fn spin(min: f64, max: f64, value: f64) -> gtk::SpinButton {
    let spin = gtk::SpinButton::with_range(min, max, 1.);
    spin.set_value(value);
    spin.set_valign(Align::Center);
    spin.set_width_chars(6);
    spin
}

fn run_output(state: &Rc<App>, name: &str, args: &[&str], good: &str) {
    let mut cmd = Command::new("zen");
    cmd.args(["msg", "output", name]).args(args);
    match cmd.output() {
        Ok(out) if out.status.success() => {
            state.say(&format!("{good}, reopen Monitors to refresh the list"), "good");
        }
        Ok(out) => {
            let why = String::from_utf8_lossy(&out.stderr);
            let why = why.lines().last().unwrap_or("zen msg refused that").trim();
            state.say(why, "bad");
        }
        Err(_) => state.say("zen is not on PATH", "bad"),
    }
}

// arrangement
fn arrangement(state: &Rc<App>, placed: &Rc<RefCell<Vec<Screen>>>) -> gtk::Widget {
    let area = gtk::DrawingArea::builder()
        .height_request(CANVAS_H)
        .hexpand(true)
        .build();
    area.add_css_class("card");

    let draw_list = placed.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let list = draw_list.borrow();
        let Some((scale, ox, oy)) = fit(&list, width as f64, height as f64) else {
            return;
        };

        for screen in list.iter() {
            let x = ox + screen.x * scale;
            let y = oy + screen.y * scale;
            let w = screen.w * scale;
            let h = screen.h * scale;

            rounded(cr, x, y, w, h, 8.);
            cr.set_source_rgba(1., 1., 1., 0.07);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(0.81, 0.90, 1., 0.55);
            cr.set_line_width(1.5);
            let _ = cr.stroke();

            cr.set_source_rgba(0.91, 0.92, 0.93, 0.9);
            cr.select_font_face("sans", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Bold);
            cr.set_font_size(13.);
            cr.move_to(x + 12., y + 24.);
            let _ = cr.show_text(&screen.name);

            cr.set_source_rgba(0.91, 0.92, 0.93, 0.45);
            cr.set_font_size(11.);
            cr.move_to(x + 12., y + 40.);
            let _ = cr.show_text(&format!("{} x {}", screen.w as i64, screen.h as i64));
        }
    });

    // drag
    let drag = gtk::GestureDrag::new();
    let held: Rc<RefCell<Option<(usize, f64, f64)>>> = Rc::new(RefCell::new(None));

    {
        let list = placed.clone();
        let held = held.clone();
        let area = area.clone();
        drag.connect_drag_begin(move |_, sx, sy| {
            let screens = list.borrow();
            let Some((scale, ox, oy)) = fit(&screens, area.width() as f64, area.height() as f64)
            else {
                return;
            };
            for (i, s) in screens.iter().enumerate().rev() {
                let x = ox + s.x * scale;
                let y = oy + s.y * scale;
                if sx >= x && sx <= x + s.w * scale && sy >= y && sy <= y + s.h * scale {
                    *held.borrow_mut() = Some((i, s.x, s.y));
                    return;
                }
            }
            *held.borrow_mut() = None;
        });
    }

    {
        let list = placed.clone();
        let held = held.clone();
        let area = area.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let Some((i, ox0, oy0)) = *held.borrow() else {
                return;
            };
            let mut screens = list.borrow_mut();
            let Some((scale, _, _)) = fit(&screens, area.width() as f64, area.height() as f64)
            else {
                return;
            };
            screens[i].x = ox0 + dx / scale;
            screens[i].y = oy0 + dy / scale;
            drop(screens);
            area.queue_draw();
        });
    }

    {
        let list = placed.clone();
        let held = held.clone();
        let area = area.clone();
        let state = state.clone();
        drag.connect_drag_end(move |_, _, _| {
            let Some((i, _, _)) = held.borrow_mut().take() else {
                return;
            };

            let mut screens = list.borrow_mut();
            let (name, x, y) = {
                let snapped = snap(&screens, i);
                screens[i].x = snapped.0;
                screens[i].y = snapped.1;
                (screens[i].name.clone(), snapped.0, snapped.1)
            };
            drop(screens);
            area.queue_draw();

            if let Some(config) = state.config.borrow_mut().as_mut() {
                config.set_output_position(&name, x.round() as i64, y.round() as i64);
            }
            state.touch();
        });
    }

    area.add_controller(drag);
    area.upcast()
}

fn snap(screens: &[Screen], i: usize) -> (f64, f64) {
    let me = &screens[i];
    let tolerance = (me.w.max(me.h) * 0.12).max(80.);

    let mut x = me.x;
    let mut y = me.y;

    for (j, other) in screens.iter().enumerate() {
        if j == i {
            continue;
        }
        for (candidate, current) in [
            (other.x + other.w, me.x),
            (other.x - me.w, me.x),
            (other.x, me.x),
        ] {
            if (candidate - current).abs() < tolerance {
                x = candidate;
            }
        }
        for (candidate, current) in [
            (other.y + other.h, me.y),
            (other.y - me.h, me.y),
            (other.y, me.y),
        ] {
            if (candidate - current).abs() < tolerance {
                y = candidate;
            }
        }
    }

    (x, y)
}

fn fit(screens: &[Screen], width: f64, height: f64) -> Option<(f64, f64, f64)> {
    if screens.is_empty() || width <= 1. || height <= 1. {
        return None;
    }

    let x0 = screens.iter().map(|s| s.x).fold(f64::MAX, f64::min);
    let y0 = screens.iter().map(|s| s.y).fold(f64::MAX, f64::min);
    let x1 = screens.iter().map(|s| s.x + s.w).fold(f64::MIN, f64::max);
    let y1 = screens.iter().map(|s| s.y + s.h).fold(f64::MIN, f64::max);

    let pad = 28.;
    let scale = ((width - pad * 2.) / (x1 - x0).max(1.))
        .min((height - pad * 2.) / (y1 - y0).max(1.))
        .min(0.25);

    let ox = (width - (x1 - x0) * scale) / 2. - x0 * scale;
    let oy = (height - (y1 - y0) * scale) / 2. - y0 * scale;
    Some((scale, ox, oy))
}

fn rounded(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.).min(h / 2.);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.);
    cr.arc(x + w - r, y + h - r, r, 0., std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
    cr.close_path();
}

fn is_last_enabled(config: &crate::config::Config, all: &[String], name: &str) -> bool {
    !all
        .iter()
        .any(|other| other != name && !config.output(other).off)
}

fn card(state: &Rc<App>, screen: &Screen, index: usize, all: &[String]) -> gtk::Widget {
    let card = gtk::Box::new(Orientation::Vertical, 0);
    card.add_css_class("card");

    let saved = state
        .config
        .borrow()
        .as_ref()
        .map(|c| c.output(&screen.name));

    // mode
    let modes = gtk::StringList::new(&[]);
    for mode in &screen.modes {
        modes.append(mode);
    }
    let chosen = saved
        .as_ref()
        .and_then(|c| c.mode.as_ref())
        .and_then(|m| screen.modes.iter().position(|x| x == m))
        .or(screen.current_mode)
        .unwrap_or(0);

    let dropdown = gtk::DropDown::builder()
        .model(&modes)
        .selected(chosen as u32)
        .valign(Align::Center)
        .build();

    {
        let state = state.clone();
        let name = screen.name.clone();
        let list = screen.modes.clone();
        dropdown.connect_selected_notify(move |d| {
            let Some(mode) = list.get(d.selected() as usize) else {
                return;
            };
            if let Some(config) = state.config.borrow_mut().as_mut() {
                config.set_output_text(&name, "mode", mode);
            }
            state.touch();
        });
    }
    card.append(&row("Resolution and refresh", "", dropdown.upcast()));

    separator(&card);

    // scale
    let scale = gtk::Scale::with_range(Orientation::Horizontal, 0.5, 3., 0.25);
    scale.set_value(saved.as_ref().and_then(|c| c.scale).unwrap_or(screen.scale));
    scale.set_draw_value(true);
    scale.set_size_request(200, -1);
    scale.set_valign(Align::Center);

    {
        let state = state.clone();
        let name = screen.name.clone();
        scale.connect_value_changed(move |s| {
            if let Some(config) = state.config.borrow_mut().as_mut() {
                config.set_output_scale(&name, s.value());
            }
            state.touch();
        });
    }
    card.append(&row(
        "Scale",
        "1 is native. 1.5 makes everything half again as big",
        scale.upcast(),
    ));

    separator(&card);

    // on
    let on = gtk::Switch::builder()
        .valign(Align::Center)
        .active(!saved.as_ref().is_some_and(|c| c.off))
        .build();

    {
        let state = state.clone();
        let name = screen.name.clone();
        let all: Vec<String> = all.to_vec();
        let reverting = Rc::new(Cell::new(false));
        let guard = reverting.clone();
        on.connect_active_notify(move |s| {
            if guard.get() {
                return;
            }

            if !s.is_active() {
                let last = state
                    .config
                    .borrow()
                    .as_ref()
                    .is_some_and(|c| is_last_enabled(c, &all, &name));
                if last {
                    guard.set(true);
                    s.set_active(true);
                    guard.set(false);
                    state.say("that is your only enabled monitor, so it stays on", "bad");
                    return;
                }
            }

            if let Some(config) = state.config.borrow_mut().as_mut() {
                config.set_output_off(&name, !s.is_active());
            }
            state.touch();
        });
    }
    card.append(&row(
        "Enabled",
        if index == 0 {
            "The last enabled monitor cannot be turned off"
        } else {
            ""
        },
        on.upcast(),
    ));

    card.upcast()
}

pub fn row(label: &str, hint: &str, control: gtk::Widget) -> gtk::Widget {
    let text = gtk::Box::new(Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(
        &gtk::Label::builder()
            .label(label)
            .halign(Align::Start)
            .css_classes(["setting-label"])
            .build(),
    );
    if !hint.is_empty() {
        text.append(
            &gtk::Label::builder()
                .label(hint)
                .halign(Align::Start)
                .wrap(true)
                .xalign(0.)
                .max_width_chars(52)
                .css_classes(["setting-hint"])
                .build(),
        );
    }

    let line = gtk::Box::new(Orientation::Horizontal, 12);
    line.add_css_class("setting");
    line.append(&text);
    line.append(&control);
    line.upcast()
}

pub fn separator(card: &gtk::Box) {
    let sep = gtk::Box::new(Orientation::Horizontal, 0);
    sep.add_css_class("row-sep");
    card.append(&sep);
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

    fn cfg(text: &str) -> crate::config::Config {
        crate::config::Config::from_str_for_test(text)
    }

    #[test]
    fn the_only_enabled_monitor_is_recognised() {
        let all = vec!["eDP-1".to_owned(), "HDMI-A-1".to_owned()];

        let both_on = cfg("");
        assert!(!is_last_enabled(&both_on, &all, "eDP-1"));

        let other_off = cfg("output \"HDMI-A-1\" {
    off
}
");
        assert!(
            is_last_enabled(&other_off, &all, "eDP-1"),
            "eDP-1 is the only one left on, so it must not be turned off"
        );
        assert!(
            !is_last_enabled(&other_off, &all, "HDMI-A-1"),
            "an already-off monitor is not the last enabled one"
        );
    }

    #[test]
    fn a_single_monitor_can_never_be_turned_off() {
        let all = vec!["eDP-1".to_owned()];
        assert!(is_last_enabled(&cfg(""), &all, "eDP-1"));
    }
}
