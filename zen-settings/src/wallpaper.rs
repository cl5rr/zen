use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::config;
use crate::App;

const THUMB_W: i32 = 208;
const THUMB_H: i32 = 117;

const VIDEO_EXT: &[&str] = &["mp4", "mkv", "webm", "mov", "m4v", "avi"];
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

struct Picker {
    files: Vec<PathBuf>,
    selected: RefCell<usize>,
    preview: gtk::Stack,
    caption: gtk::Label,
    audio: gtk::Widget,
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
            .label(
                "The folder is the config. Drop anything in it, images or video, \
                 and it joins the grid.",
            )
            .halign(Align::Start)
            .xalign(0.)
            .wrap(true)
            .css_classes(["page-blurb"])
            .build(),
    );

    let dir = config::wallpaper_dir();
    let files = wallpapers(&dir);

    if files.is_empty() {
        column.append(
            &gtk::Label::builder()
                .label(format!("Nothing in {}", dir.display()))
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
        preview.add_named(&preview_for(file), Some(&i.to_string()));
    }
    preview.set_visible_child_name(&current.to_string());

    let caption = gtk::Label::builder()
        .label(caption_for(&files[current]))
        .halign(Align::Start)
        .css_classes(["setting-hint"])
        .build();

    // Volume only means anything for a video, so the row goes away for an image
    // rather than sitting there greyed out with nothing to explain it.
    let (audio_row, volume_scale) = audio_controls();

    let picker = Rc::new(Picker {
        files: files.clone(),
        selected: RefCell::new(current),
        preview: preview.clone(),
        caption: caption.clone(),
        audio: audio_row.clone(),
    });

    {
        let state = state.clone();
        volume_scale.connect_value_changed(move |s| {
            let v = s.value().round() as i64;
            match Command::new("zen-wallpaper")
                .arg("volume")
                .arg(v.to_string())
                .status()
            {
                Ok(st) if st.success() => state.say(&format!("volume {v}"), ""),
                Ok(_) => state.say("zen-wallpaper refused that volume", "bad"),
                Err(_) => state.say("zen-wallpaper is not on PATH", "bad"),
            }
        });
    }

    let grid = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .homogeneous(true)
        .row_spacing(12)
        .column_spacing(12)
        .min_children_per_line(3)
        .max_children_per_line(6)
        .build();
    grid.add_css_class("wallpaper-grid");

    for file in &files {
        grid.append(&tile(file));
    }

    // Selecting is one click. Applying stays deliberate, because it restarts a
    // renderer and, for a video, starts something that can make noise.
    {
        let picker = picker.clone();
        grid.connect_selected_children_changed(move |g| {
            let Some(child) = g.selected_children().first().cloned() else {
                return;
            };
            picker.select(child.index() as usize);
        });
    }

    let apply = gtk::Button::builder()
        .label("Set as wallpaper")
        .css_classes(["flat"])
        .build();
    {
        let picker = picker.clone();
        let state = state.clone();
        apply.connect_clicked(move |_| picker.apply(&state));
    }
    {
        let picker = picker.clone();
        let state = state.clone();
        grid.connect_child_activated(move |_, child| {
            picker.select(child.index() as usize);
            picker.apply(&state);
        });
    }

    let controls = gtk::Box::new(Orientation::Horizontal, 8);
    controls.set_margin_top(4);
    let spacer = gtk::Box::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    controls.append(&caption);
    controls.append(&spacer);
    controls.append(&apply);

    column.append(&preview);
    column.append(&controls);
    column.append(&audio_row);
    column.append(
        &gtk::Label::builder()
            .label("IN THE FOLDER")
            .halign(Align::Start)
            .css_classes(["group-label"])
            .build(),
    );
    column.append(&grid);
    column.append(
        &gtk::Label::builder()
            .label(dir.display().to_string())
            .halign(Align::Start)
            .selectable(true)
            .css_classes(["mono"])
            .build(),
    );

    if let Some(child) = grid.child_at_index(current as i32) {
        grid.select_child(&child);
    }
    picker.sync_audio();

    scrolled(&column)
}

impl Picker {
    fn select(self: &Rc<Self>, target: usize) {
        if target >= self.files.len() || *self.selected.borrow() == target {
            return;
        }
        *self.selected.borrow_mut() = target;
        self.preview.set_visible_child_name(&target.to_string());
        self.caption.set_label(&caption_for(&self.files[target]));
        self.sync_audio();
    }

    fn sync_audio(self: &Rc<Self>) {
        let is_video = is_video(&self.files[*self.selected.borrow()]);
        self.audio.set_visible(is_video);
    }

    fn apply(self: &Rc<Self>, state: &Rc<App>) {
        let file = self.files[*self.selected.borrow()].clone();
        match Command::new("zen-wallpaper").arg("set").arg(&file).status() {
            Ok(s) if s.success() => {
                state.say(&format!("wallpaper: {}", name_of(&file)), "good");
            }
            // The script says why on stderr, and the reasons differ: a missing
            // mpvpaper for video, a missing swaybg for an image.
            Ok(_) => state.say("zen-wallpaper could not set that, see its output", "bad"),
            Err(_) => state.say("zen-wallpaper is not on PATH", "bad"),
        }
    }
}

// tiles
fn tile(file: &Path) -> gtk::Widget {
    let frame = gtk::Box::new(Orientation::Vertical, 0);
    frame.add_css_class("thumb");
    frame.set_size_request(THUMB_W, THUMB_H);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&thumbnail(file)));

    if is_video(file) {
        let badge = gtk::Label::builder()
            .label("VIDEO")
            .halign(Align::End)
            .valign(Align::End)
            .margin_end(6)
            .margin_bottom(6)
            .css_classes(["badge"])
            .build();
        overlay.add_overlay(&badge);
    }

    frame.append(&overlay);
    frame.upcast()
}

// GdkPixbuf will not decode a video, so one gets a placeholder rather than an empty
// box. Pulling a real frame would mean shelling out to ffmpeg once per file.
fn thumbnail(file: &Path) -> gtk::Widget {
    if is_video(file) {
        let icon = gtk::Image::from_icon_name("video-x-generic-symbolic");
        icon.set_pixel_size(48);
        icon.set_size_request(THUMB_W, THUMB_H);
        icon.add_css_class("video-thumb");
        return icon.upcast();
    }

    let picture = gtk::Picture::for_filename(file);
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(THUMB_W, THUMB_H);
    picture.upcast()
}

fn preview_for(file: &Path) -> gtk::Widget {
    if is_video(file) {
        let stack = gtk::Box::new(Orientation::Vertical, 8);
        stack.set_valign(Align::Center);
        stack.add_css_class("video-preview");

        let icon = gtk::Image::from_icon_name("video-x-generic-symbolic");
        icon.set_pixel_size(64);
        stack.append(&icon);
        stack.append(
            &gtk::Label::builder()
                .label(name_of(file))
                .css_classes(["setting-hint"])
                .build(),
        );
        return stack.upcast();
    }

    let picture = gtk::Picture::for_filename(file);
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.upcast()
}

fn audio_controls() -> (gtk::Widget, gtk::Scale) {
    let row = gtk::Box::new(Orientation::Horizontal, 14);
    row.add_css_class("setting");

    let text = gtk::Box::new(Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(
        &gtk::Label::builder()
            .label("Volume")
            .halign(Align::Start)
            .build(),
    );
    text.append(
        &gtk::Label::builder()
            .label("Video wallpapers start muted. This applies straight away.")
            .halign(Align::Start)
            .css_classes(["setting-hint"])
            .build(),
    );
    row.append(&text);

    let scale = gtk::Scale::with_range(Orientation::Horizontal, 0., 100., 1.);
    scale.set_value(current_volume());
    scale.set_draw_value(true);
    scale.set_size_request(200, -1);
    scale.set_valign(Align::Center);
    row.append(&scale);

    (row.upcast(), scale)
}

// util
fn scrolled(child: &gtk::Box) -> gtk::Widget {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(child)
        .hexpand(true)
        .vexpand(true)
        .build()
        .upcast()
}

fn extension_is(path: &Path, list: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| list.contains(&e.as_str()))
}

fn is_video(path: &Path) -> bool {
    extension_is(path, VIDEO_EXT)
}

fn wallpapers(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| extension_is(p, IMAGE_EXT) || extension_is(p, VIDEO_EXT))
        .collect();
    out.sort();
    out
}

fn state_file(name: &str) -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_default()
        .join("zen")
        .join(name)
}

fn current_index(files: &[PathBuf]) -> usize {
    let Ok(text) = std::fs::read_to_string(state_file("wallpaper")) else {
        return 0;
    };
    let wanted = PathBuf::from(text.trim());
    files.iter().position(|f| *f == wanted).unwrap_or(0)
}

fn current_volume() -> f64 {
    std::fs::read_to_string(state_file("wallpaper-volume"))
        .ok()
        .and_then(|t| t.trim().parse::<f64>().ok())
        .unwrap_or(0.)
        .clamp(0., 100.)
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn caption_for(path: &Path) -> String {
    if is_video(path) {
        format!("{}  ·  video", name_of(path))
    } else {
        name_of(path)
    }
}
