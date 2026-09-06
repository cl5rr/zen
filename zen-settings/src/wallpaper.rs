use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::config;
use crate::App;

const THUMB_W: i32 = 168;
const THUMB_H: i32 = 96;
const REEL_GAP: i32 = 12;

struct Reel {
    files: Vec<PathBuf>,
    index: RefCell<usize>,
    strip: gtk::Box,
    scroller: gtk::ScrolledWindow,
    preview: gtk::Stack,
    caption: gtk::Label,
}

// page
pub fn page(state: &Rc<App>) -> gtk::Widget {
    let column = gtk::Box::new(Orientation::Vertical, 10);
    column.add_css_class("page");

    column.append(
        &gtk::Label::builder()
            .label("Wallpaper")
            .halign(Align::Start)
            .css_classes(["page-title"])
            .build(),
    );
    column.append(
        &gtk::Label::builder()
            .label("The folder is the config. Anything you drop in it joins the cycle.")
            .halign(Align::Start)
            .xalign(0.)
            .wrap(true)
            .css_classes(["page-blurb"])
            .build(),
    );

    let dir = config::wallpaper_dir();
    let files = images(&dir);

    if files.is_empty() {
        column.append(
            &gtk::Label::builder()
                .label(format!("No images in {}", dir.display()))
                .halign(Align::Start)
                .css_classes(["setting-hint"])
                .build(),
        );
        return scrolled(&column);
    }

    let current = current_index(&files);

    let preview = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(220)
        .height_request(250)
        .build();
    preview.add_css_class("preview");

    for (i, file) in files.iter().enumerate() {
        let picture = gtk::Picture::for_filename(file);
        picture.set_content_fit(gtk::ContentFit::Cover);
        preview.add_named(&picture, Some(&i.to_string()));
    }
    preview.set_visible_child_name(&current.to_string());

    let caption = gtk::Label::builder()
        .label(name_of(&files[current]))
        .halign(Align::Start)
        .css_classes(["setting-hint"])
        .build();

    let strip = gtk::Box::new(Orientation::Horizontal, REEL_GAP as i32);
    strip.set_halign(Align::Start);

    for (i, file) in files.iter().enumerate() {
        let picture = gtk::Picture::for_filename(file);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(THUMB_W, THUMB_H);

        let frame = gtk::Box::new(Orientation::Vertical, 0);
        frame.add_css_class("thumb");
        if i == current {
            frame.add_css_class("current");
        }
        frame.append(&picture);
        strip.append(&frame);
    }

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .height_request(THUMB_H + 10)
        .child(&strip)
        .build();
    scroller.add_css_class("reel");

    let reel = Rc::new(Reel {
        files,
        index: RefCell::new(current),
        strip,
        scroller: scroller.clone(),
        preview: preview.clone(),
        caption: caption.clone(),
    });

    let controls = gtk::Box::new(Orientation::Horizontal, 8);
    controls.set_margin_top(4);

    let back = button("Previous");
    let next = button("Next");
    let random = button("Random");
    let apply = button("Set as wallpaper");

    controls.append(&back);
    controls.append(&next);
    controls.append(&random);

    let spacer = gtk::Box::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    controls.append(&spacer);
    controls.append(&apply);

    {
        let reel = reel.clone();
        back.connect_clicked(move |_| reel.step(-1));
    }
    {
        let reel = reel.clone();
        next.connect_clicked(move |_| reel.step(1));
    }
    {
        let reel = reel.clone();
        random.connect_clicked(move |_| {
            let n = reel.files.len();
            if n < 2 {
                return;
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as usize)
                .unwrap_or(0);
            let mut target = now % n;
            if target == *reel.index.borrow() {
                target = (target + 1) % n;
            }
            reel.go(target);
        });
    }
    {
        let reel = reel.clone();
        let state = state.clone();
        apply.connect_clicked(move |_| {
            let file = reel.files[*reel.index.borrow()].clone();
            match Command::new("zen-wallpaper").arg("set").arg(&file).status() {
                Ok(s) if s.success() => state.say(&format!("wallpaper: {}", name_of(&file)), "good"),
                Ok(_) => state.say("zen-wallpaper could not set that image", "bad"),
                Err(_) => state.say("zen-wallpaper is not on PATH", "bad"),
            }
        });
    }

    let caption_line = gtk::Box::new(Orientation::Horizontal, 12);
    caption_line.append(&caption);

    column.append(&preview);
    column.append(&caption_line);
    column.append(&controls);
    column.append(
        &gtk::Label::builder()
            .label("UP NEXT")
            .halign(Align::Start)
            .css_classes(["group-label"])
            .build(),
    );
    column.append(&scroller);
    column.append(
        &gtk::Label::builder()
            .label(dir.display().to_string())
            .halign(Align::Start)
            .selectable(true)
            .css_classes(["mono"])
            .build(),
    );

    reel.scroll_to(current, false);
    scrolled(&column)
}

impl Reel {
    fn step(self: &Rc<Self>, delta: i32) {
        let n = self.files.len() as i32;
        if n < 2 {
            return;
        }
        let at = *self.index.borrow() as i32;
        self.go((((at + delta) % n) + n) as usize % n as usize);
    }

    fn go(self: &Rc<Self>, target: usize) {
        {
            let mut index = self.index.borrow_mut();
            if *index == target {
                return;
            }
            mark(&self.strip, *index, false);
            *index = target;
        }
        mark(&self.strip, target, true);

        self.preview.set_visible_child_name(&target.to_string());
        self.caption.set_label(&name_of(&self.files[target]));
        self.scroll_to(target, true);
    }

    // A reel that jumps has no cycle to read ahead in, so the strip slides.
    fn scroll_to(self: &Rc<Self>, index: usize, animate: bool) {
        let adjustment = self.scroller.hadjustment();
        let slot = (THUMB_W + REEL_GAP) as f64;
        let visible = adjustment.page_size().max(slot);
        let target = (slot * index as f64 - (visible - slot) / 2.)
            .clamp(0., (adjustment.upper() - visible).max(0.));

        if !animate {
            adjustment.set_value(target);
            return;
        }

        let from = adjustment.value();
        let target_object = adw::CallbackAnimationTarget::new(move |value| {
            adjustment.set_value(value);
        });
        let animation = adw::TimedAnimation::new(&self.scroller, from, target, 320, target_object);
        animation.set_easing(adw::Easing::EaseOutCubic);

        let keep = RefCell::new(Some(animation.clone()));
        animation.connect_done(move |_| {
            keep.borrow_mut().take();
        });
        animation.play();
    }
}

// util
fn mark(strip: &gtk::Box, index: usize, on: bool) {
    let mut child = strip.first_child();
    let mut i = 0;
    while let Some(widget) = child {
        if i == index {
            if on {
                widget.add_css_class("current");
            } else {
                widget.remove_css_class("current");
            }
        }
        child = widget.next_sibling();
        i += 1;
    }
}

fn button(label: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .css_classes(["flat"])
        .build()
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

fn images(dir: &PathBuf) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .is_some_and(|e| {
                    matches!(e.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif")
                })
        })
        .collect();
    out.sort();
    out
}

fn current_index(files: &[PathBuf]) -> usize {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_default()
        .join("zen")
        .join("wallpaper");

    let Ok(text) = std::fs::read_to_string(state) else {
        return 0;
    };
    let wanted = PathBuf::from(text.trim());
    files.iter().position(|f| *f == wanted).unwrap_or(0)
}

fn name_of(path: &PathBuf) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
