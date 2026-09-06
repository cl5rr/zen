use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pangocairo::cairo::{self, ImageSurface};
use pangocairo::pango::FontDescription;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::reexports::gbm::Format as Fourcc;
use smithay::utils::Transform;

use crate::render_helpers::renderer::ZenRenderer;
use crate::render_helpers::texture::TextureBuffer;

// An app_id turned into something you can recognise at a glance.
//
// The map draws applications, not windows, so it needs the icon a launcher would show.
// That means the freedesktop lookup: find the .desktop file that claims this app_id,
// read its Icon= key, then find a file for that name in the icon theme.
//
// Two caches, because they fail differently. The path cache holds the result of walking
// the filesystem and is keyed by app_id alone; the texture cache holds uploaded pixels
// and is keyed by size too. A miss in either is remembered as a miss, so a window whose
// app has no icon does not re-walk /usr/share/applications every frame.
#[derive(Debug, Default)]
pub struct AppIcons {
    paths: RefCell<HashMap<String, Option<PathBuf>>>,
    textures: RefCell<HashMap<(String, u32), Option<TextureBuffer<GlesTexture>>>>,
}

impl AppIcons {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&self) {
        self.textures.borrow_mut().clear();
    }

    // The icon for an app, at a square size in physical pixels. Falls back to the app's
    // initial on a disc, so this only returns None if even cairo failed.
    pub fn get<R: ZenRenderer>(
        &self,
        renderer: &mut R,
        app_id: &str,
        px: u32,
    ) -> Option<TextureBuffer<GlesTexture>> {
        let px = px.clamp(16, 512);
        let key = (app_id.to_owned(), px);

        if let Some(cached) = self.textures.borrow().get(&key) {
            return cached.clone();
        }

        let path = self.path_for(app_id);
        let renderer = renderer.as_gles_renderer();
        let built = match render_icon(renderer, path.as_deref(), app_id, px) {
            Ok(buffer) => Some(buffer),
            Err(err) => {
                warn!("could not draw an icon for {app_id:?}: {err:?}");
                None
            }
        };

        self.textures.borrow_mut().insert(key, built.clone());
        built
    }

    fn path_for(&self, app_id: &str) -> Option<PathBuf> {
        if let Some(cached) = self.paths.borrow().get(app_id) {
            return cached.clone();
        }
        let found = icon_name_for(app_id).and_then(|name| find_icon_file(&name));
        self.paths.borrow_mut().insert(app_id.to_owned(), found.clone());
        found
    }
}

// lookup
fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))
    }) {
        dirs.push(home);
    }

    let system = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
    dirs.extend(system.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));

    dirs
}

// The Icon= value from the .desktop file that belongs to this app_id.
//
// Matched by filename first, because that is what most Wayland apps set their app_id
// to, then by StartupWMClass, which is how the rest of them declare it.
fn icon_name_for(app_id: &str) -> Option<String> {
    if app_id.is_empty() {
        return None;
    }

    let mut fallback = None;

    for dir in data_dirs() {
        let apps = dir.join("applications");

        for candidate in [
            apps.join(format!("{app_id}.desktop")),
            apps.join(format!("{}.desktop", app_id.to_ascii_lowercase())),
        ] {
            if let Some(icon) = read_icon_key(&candidate) {
                return Some(icon);
            }
        }

        // No direct filename hit, so read the directory looking for a declared class.
        // Only entered when the cheap lookups missed, and the result is cached.
        let Ok(entries) = std::fs::read_dir(&apps) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };

            let mut icon = None;
            let mut matches = false;
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("Icon=") {
                    icon.get_or_insert_with(|| v.trim().to_owned());
                } else if let Some(v) = line.strip_prefix("StartupWMClass=") {
                    if v.trim().eq_ignore_ascii_case(app_id) {
                        matches = true;
                    }
                }
            }

            if matches {
                if let Some(icon) = icon {
                    return Some(icon);
                }
            } else if fallback.is_none() {
                // A last resort for app_ids that are a prefix of the desktop id, which
                // is common for Electron apps and for anything reverse-DNS named.
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem.eq_ignore_ascii_case(app_id)
                    || stem.rsplit('.').next().is_some_and(|s| s.eq_ignore_ascii_case(app_id))
                {
                    fallback = icon;
                }
            }
        }
    }

    fallback
}

fn read_icon_key(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("Icon="))
        .map(|v| v.trim().to_owned())
}

// An icon name resolved to a PNG on disk.
//
// Only PNG: ZEN has a png decoder already and no SVG rasteriser, and pulling one in for
// this would be a bigger dependency than the feature. Themes that ship only SVG fall
// through to the lettered disc, which is why that fallback is not an error path.
fn find_icon_file(name: &str) -> Option<PathBuf> {
    if name.starts_with('/') {
        let path = PathBuf::from(name);
        return path.exists().then_some(path);
    }

    // Biggest first: the map scales them down, and downscaling beats upscaling.
    const SIZES: &[&str] = &[
        "512x512", "256x256", "192x192", "128x128", "96x96", "64x64", "48x48", "32x32",
    ];

    let themes = ["hicolor", "Adwaita", "breeze", "Papirus", "gnome"];

    for dir in data_dirs() {
        let icons = dir.join("icons");
        for theme in themes {
            for size in SIZES {
                for sub in ["apps", "devices", "places"] {
                    let path = icons
                        .join(theme)
                        .join(size)
                        .join(sub)
                        .join(format!("{name}.png"));
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }

        // The flat directories, which is where most third-party installers drop things.
        for flat in [dir.join("pixmaps"), icons.clone()] {
            let path = flat.join(format!("{name}.png"));
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

// drawing
fn render_icon(
    renderer: &mut GlesRenderer,
    path: Option<&Path>,
    app_id: &str,
    px: u32,
) -> anyhow::Result<TextureBuffer<GlesTexture>> {
    let _span = tracy_client::span!("app_icons::render_icon");

    let size = px as i32;
    let surface = ImageSurface::create(cairo::Format::ARgb32, size, size)?;
    let cr = cairo::Context::new(&surface)?;

    let drawn = path.and_then(|p| draw_png(&cr, p, size).ok()).is_some();
    if !drawn {
        draw_initial(&cr, app_id, size)?;
    }

    drop(cr);
    let data = surface.take_data().unwrap();
    let buffer = TextureBuffer::from_memory(
        renderer,
        &data,
        Fourcc::Argb8888,
        (size, size),
        false,
        1.,
        Transform::Normal,
        Vec::new(),
    )?;
    Ok(buffer)
}

fn draw_png(cr: &cairo::Context, path: &Path, size: i32) -> anyhow::Result<()> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info()?;
    let mut raw = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut raw)?;

    if info.bit_depth != png::BitDepth::Eight {
        anyhow::bail!("unsupported bit depth for {}", path.display());
    }

    let (w, h) = (info.width as i32, info.height as i32);
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        _ => anyhow::bail!("unsupported colour type for {}", path.display()),
    };

    // Cairo wants premultiplied BGRA in native byte order, which on little endian is
    // B, G, R, A. Straight alpha from the PNG has to be multiplied in or every icon
    // with soft edges gets a bright halo.
    let mut argb = vec![0u8; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        let src = i * channels;
        let (r, g, b, a) = if channels == 4 {
            (raw[src], raw[src + 1], raw[src + 2], raw[src + 3])
        } else {
            (raw[src], raw[src + 1], raw[src + 2], 255)
        };
        let m = |c: u8| ((c as u16 * a as u16) / 255) as u8;
        let dst = i * 4;
        argb[dst] = m(b);
        argb[dst + 1] = m(g);
        argb[dst + 2] = m(r);
        argb[dst + 3] = a;
    }

    let stride = cairo::Format::ARgb32.stride_for_width(w as u32)?;
    let mut padded = vec![0u8; (stride * h) as usize];
    for y in 0..h as usize {
        let from = y * (w * 4) as usize;
        let to = y * stride as usize;
        padded[to..to + (w * 4) as usize].copy_from_slice(&argb[from..from + (w * 4) as usize]);
    }

    let icon = ImageSurface::create_for_data(padded, cairo::Format::ARgb32, w, h, stride)?;

    let scale = size as f64 / w.max(h) as f64;
    cr.save()?;
    cr.scale(scale, scale);
    cr.set_source_surface(&icon, 0., 0.)?;
    cr.paint()?;
    cr.restore()?;
    Ok(())
}

// The fallback, and it has to look deliberate rather than broken: a filled disc in a
// colour derived from the app_id, with its initial on top. Two apps only collide if
// their names hash the same, and the letter still tells them apart.
fn draw_initial(cr: &cairo::Context, app_id: &str, size: i32) -> anyhow::Result<()> {
    let (r, g, b) = hue_for(app_id);
    let half = size as f64 / 2.;

    cr.arc(half, half, half, 0., std::f64::consts::TAU);
    cr.set_source_rgba(r, g, b, 0.95);
    cr.fill()?;

    let letter: String = app_id
        .rsplit('.')
        .next()
        .unwrap_or(app_id)
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned());

    let mut font = FontDescription::from_string("sans Bold");
    font.set_absolute_size(size as f64 * 0.55 * f64::from(pangocairo::pango::SCALE));

    let layout = pangocairo::functions::create_layout(cr);
    layout.set_font_description(Some(&font));
    layout.set_text(&letter);

    let (tw, th) = layout.pixel_size();
    cr.set_source_rgba(1., 1., 1., 0.92);
    cr.move_to(half - tw as f64 / 2., half - th as f64 / 2.);
    pangocairo::functions::show_layout(cr, &layout);
    Ok(())
}

fn hue_for(app_id: &str) -> (f64, f64, f64) {
    let mut hash: u32 = 2166136261;
    for byte in app_id.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16777619);
    }

    // Kept away from full saturation so a screen of these does not look like a paint
    // chart, and away from full lightness so the white letter stays readable.
    let h = f64::from(hash % 360);
    let (s, l) = (0.45, 0.45);
    hsl_to_rgb(h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1. - (2. * l - 1.).abs()) * s;
    let hp = h / 60.;
    let x = c * (1. - (hp % 2. - 1.).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = l - c / 2.;
    (r + m, g + m, b + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_stable_and_differs_between_apps() {
        assert_eq!(hue_for("firefox"), hue_for("firefox"));
        assert_ne!(hue_for("firefox"), hue_for("discord"));
    }

    #[test]
    fn every_colour_is_in_range() {
        for app in ["a", "firefox", "org.gnome.Nautilus", "", "zz-long-app-id"] {
            let (r, g, b) = hue_for(app);
            for c in [r, g, b] {
                assert!((0. ..=1.).contains(&c), "{app}: {c} is out of range");
            }
        }
    }

    #[test]
    fn an_absolute_icon_path_is_taken_as_is() {
        assert_eq!(find_icon_file("/definitely/not/here.png"), None);
    }

    #[test]
    fn an_empty_app_id_has_no_desktop_file() {
        assert_eq!(icon_name_for(""), None);
    }
}
