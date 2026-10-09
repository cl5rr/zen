use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};
use serde_json::{json, Value};

use crate::config::Config;
use crate::monitors::{row, separator};
use crate::App;

const RULE: &[&str] = &["layer-rule@namespace=caelestia*", "background-effect"];
const DEFAULT_FROST: f64 = 0.62;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    Glass,
    Flat,
}

// shell
pub fn shell_config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    base.join("caelestia").join("shell.json")
}

fn read_shell() -> Value {
    fs::read_to_string(shell_config_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn write_shell(value: &Value) -> anyhow::Result<()> {
    let path = shell_config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}

fn transparency(shell: &mut Value) -> &mut serde_json::Map<String, Value> {
    let root = shell.as_object_mut().expect("shell config is an object");
    let appearance = root.entry("appearance").or_insert_with(|| json!({}));
    if !appearance.is_object() {
        *appearance = json!({});
    }
    let t = appearance
        .as_object_mut()
        .unwrap()
        .entry("transparency")
        .or_insert_with(|| json!({}));
    if !t.is_object() {
        *t = json!({});
    }
    t.as_object_mut().unwrap()
}

// apply
pub fn current(config: &Config, shell: &Value) -> Style {
    let glass = config.boolean(RULE, "glass").unwrap_or(false);
    let see_through = shell["appearance"]["transparency"]["enabled"]
        .as_bool()
        .unwrap_or(false);
    if glass && see_through {
        Style::Glass
    } else {
        Style::Flat
    }
}

pub fn frost(shell: &Value) -> f64 {
    shell["appearance"]["transparency"]["base"]
        .as_f64()
        .unwrap_or(DEFAULT_FROST)
}

pub fn apply(config: &mut Config, shell: &mut Value, style: Style, frost: f64) {
    let glass = style == Style::Glass;
    config.set_boolean(RULE, "glass", glass);
    config.set_boolean(RULE, "blur", glass);
    config.set_boolean(RULE, "alpha-mask", true);

    let t = transparency(shell);
    t.insert("enabled".into(), json!(glass));
    t.insert("base".into(), json!((frost * 100.).round() / 100.));
    t.entry("layers").or_insert(json!(0.4));
}

// page
pub fn page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    column.append(
        &gtk::Label::builder()
            .label("Style")
            .halign(Align::Start)
            .css_classes(["page-title"])
            .build(),
    );
    column.append(
        &gtk::Label::builder()
            .label("How the bar, the launcher, notifications and every other part of ZEN Shell look. Both take their colours from your wallpaper.")
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.)
            .max_width_chars(78)
            .css_classes(["page-blurb"])
            .build(),
    );

    let shell = read_shell();
    let chosen = state
        .config
        .borrow()
        .as_ref()
        .map_or(Style::Glass, |c| current(c, &shell));
    let frost_now = Rc::new(Cell::new(frost(&shell)));

    let cards = gtk::Box::new(Orientation::Horizontal, 14);
    cards.set_homogeneous(true);
    let glass_card = card(Style::Glass, "Material Glass", "Translucent panels with real glass behind them");
    let flat_card = card(Style::Flat, "Material You", "Solid tonal panels, calm and quick to read");
    flat_card.set_group(Some(&glass_card));
    glass_card.set_active(chosen == Style::Glass);
    flat_card.set_active(chosen == Style::Flat);
    cards.append(&glass_card);
    cards.append(&flat_card);
    column.append(&cards);

    let settings = gtk::Box::new(Orientation::Vertical, 0);
    settings.add_css_class("card");

    let slider = gtk::Scale::with_range(Orientation::Horizontal, 0.35, 0.9, 0.01);
    slider.set_value(frost_now.get());
    slider.set_hexpand(true);
    slider.set_width_request(220);
    slider.set_valign(Align::Center);
    slider.set_sensitive(chosen == Style::Glass);
    settings.append(&row(
        "Frost",
        "Lower shows more of what is behind a panel, higher reads more like paper",
        slider.clone().upcast(),
    ));
    separator(&settings);
    settings.append(&row(
        "Changes apply straight away",
        "ZEN and the shell both reload their settings while you watch",
        gtk::Box::new(Orientation::Horizontal, 0).upcast(),
    ));
    column.append(&settings);

    let commit = {
        let state = state.clone();
        let frost_now = frost_now.clone();
        Rc::new(move |style: Style| {
            let mut shell = read_shell();
            if let Some(config) = state.config.borrow_mut().as_mut() {
                apply(config, &mut shell, style, frost_now.get());
            }
            match write_shell(&shell) {
                Ok(()) => state.touch(),
                Err(err) => state.say(&format!("could not write the shell settings: {err}"), "bad"),
            }
        })
    };

    {
        let commit = commit.clone();
        let slider = slider.clone();
        glass_card.connect_toggled(move |b| {
            if b.is_active() {
                slider.set_sensitive(true);
                commit(Style::Glass);
            }
        });
    }
    {
        let commit = commit.clone();
        let slider = slider.clone();
        flat_card.connect_toggled(move |b| {
            if b.is_active() {
                slider.set_sensitive(false);
                commit(Style::Flat);
            }
        });
    }
    {
        let commit = commit.clone();
        slider.connect_value_changed(move |s| {
            frost_now.set(s.value());
            commit(Style::Glass);
        });
    }

    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&column)
        .hexpand(true)
        .vexpand(true)
        .build()
        .upcast()
}

fn card(style: Style, title: &str, subtitle: &str) -> gtk::ToggleButton {
    let preview = gtk::DrawingArea::builder()
        .content_height(130)
        .hexpand(true)
        .build();
    preview.set_draw_func(move |_, cr, w, h| draw_preview(cr, f64::from(w), f64::from(h), style));

    let inner = gtk::Box::new(Orientation::Vertical, 6);
    inner.append(&preview);
    inner.append(
        &gtk::Label::builder()
            .label(title)
            .halign(Align::Start)
            .css_classes(["style-title"])
            .build(),
    );
    inner.append(
        &gtk::Label::builder()
            .label(subtitle)
            .halign(Align::Start)
            .wrap(true)
            .xalign(0.)
            .css_classes(["setting-hint"])
            .build(),
    );

    gtk::ToggleButton::builder()
        .child(&inner)
        .css_classes(["style-card"])
        .build()
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

fn draw_preview(cr: &gtk::cairo::Context, w: f64, h: f64, style: Style) {
    rounded(cr, 0., 0., w, h, 12.);
    cr.clip();

    let sky = gtk::cairo::LinearGradient::new(0., 0., w, h);
    sky.add_color_stop_rgb(0., 0.93, 0.89, 0.72);
    sky.add_color_stop_rgb(0.45, 0.42, 0.66, 0.92);
    sky.add_color_stop_rgb(1., 0.10, 0.27, 0.72);
    let _ = cr.set_source(&sky);
    let _ = cr.paint();

    cr.set_source_rgba(1., 1., 1., 0.35);
    cr.set_line_width(2.);
    cr.move_to(w * 0.05, h * 1.05);
    cr.curve_to(w * 0.35, h * 0.55, w * 0.55, h * 0.35, w * 1.05, -h * 0.05);
    let _ = cr.stroke();

    let bar = (8., 8., 18., h - 16.);
    let panel = (w * 0.28, h * 0.42, w * 0.56, h * 0.5);

    for (x, y, pw, ph, r) in [(bar.0, bar.1, bar.2, bar.3, 9.), (panel.0, panel.1, panel.2, panel.3, 14.)] {
        rounded(cr, x, y, pw, ph, r);
        match style {
            Style::Glass => {
                cr.set_source_rgba(1., 1., 1., 0.22);
                let _ = cr.fill_preserve();
                cr.set_source_rgba(1., 1., 1., 0.65);
                cr.set_line_width(1.);
                let _ = cr.stroke();
            }
            Style::Flat => {
                cr.set_source_rgb(0.16, 0.19, 0.25);
                let _ = cr.fill();
            }
        }
    }

    let (accent, text) = match style {
        Style::Glass => ((1., 1., 1., 0.85), (1., 1., 1., 0.75)),
        Style::Flat => ((0.66, 0.78, 1., 1.), (0.85, 0.88, 0.95, 0.85)),
    };
    cr.set_source_rgba(accent.0, accent.1, accent.2, accent.3);
    cr.arc(bar.0 + bar.2 / 2., bar.1 + 14., 5., 0., std::f64::consts::TAU);
    let _ = cr.fill();

    for i in 0..3 {
        let y = panel.1 + 14. + f64::from(i) * 14.;
        rounded(cr, panel.0 + 12., y, panel.2 * (0.75 - f64::from(i) * 0.15), 7., 3.5);
        cr.set_source_rgba(text.0, text.1, text.2, text.3 * (1. - f64::from(i) * 0.25));
        let _ = cr.fill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choosing_glass_turns_on_both_halves() {
        let mut c = Config::from_str_for_test("");
        let mut shell = json!({});
        apply(&mut c, &mut shell, Style::Glass, 0.6);

        assert_eq!(c.boolean(RULE, "glass"), Some(true));
        assert_eq!(c.boolean(RULE, "blur"), Some(true));
        assert_eq!(c.boolean(RULE, "alpha-mask"), Some(true));
        assert_eq!(shell["appearance"]["transparency"]["enabled"], json!(true));
        assert_eq!(shell["appearance"]["transparency"]["base"], json!(0.6));
        assert_eq!(current(&c, &shell), Style::Glass);
    }

    #[test]
    fn choosing_material_you_turns_both_off() {
        let mut c = Config::from_str_for_test("");
        let mut shell = json!({});
        apply(&mut c, &mut shell, Style::Glass, 0.6);
        apply(&mut c, &mut shell, Style::Flat, 0.6);

        assert_eq!(c.boolean(RULE, "glass"), Some(false));
        assert_eq!(shell["appearance"]["transparency"]["enabled"], json!(false));
        assert_eq!(current(&c, &shell), Style::Flat);
    }

    #[test]
    fn the_rest_of_the_shell_config_is_kept() {
        let mut c = Config::from_str_for_test("");
        let mut shell = json!({ "bar": { "persistent": false }, "appearance": { "rounding": { "scale": 2 } } });
        apply(&mut c, &mut shell, Style::Glass, 0.7);

        assert_eq!(shell["bar"]["persistent"], json!(false));
        assert_eq!(shell["appearance"]["rounding"]["scale"], json!(2));
    }

    #[test]
    fn the_shipped_config_is_found_as_glass_with_a_glass_shell() {
        let c = Config::from_str_for_test(include_str!("../../resources/default-config.kdl"));
        let shell = json!({ "appearance": { "transparency": { "enabled": true } } });
        assert_eq!(current(&c, &shell), Style::Glass);
    }

    #[test]
    fn a_new_rule_matches_every_shell_layer() {
        let mut c = Config::from_str_for_test("");
        let mut shell = json!({});
        apply(&mut c, &mut shell, Style::Glass, 0.6);
        assert!(
            c.text().contains("namespace=\"^caelestia\""),
            "the shell's layers are caelestia-drawers, caelestia-background and so on, so the rule must be a prefix: {}",
            c.text()
        );
    }

    #[test]
    fn one_rule_is_edited_not_a_second_one_added() {
        let mut c = Config::from_str_for_test(include_str!("../../resources/default-config.kdl"));
        let before = c.text().matches("namespace=\"^caelestia").count();
        let mut shell = json!({});
        apply(&mut c, &mut shell, Style::Flat, 0.6);
        assert_eq!(c.text().matches("namespace=\"^caelestia").count(), before);
    }
}
