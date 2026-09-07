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
        let found = resolve(app_id);
        self.paths.borrow_mut().insert(app_id.to_owned(), found.clone());
        found
    }
}

fn resolve(app_id: &str) -> Option<PathBuf> {
    if let Some(path) = icon_name_for(app_id).and_then(|name| find_icon_file(&name)) {
        return Some(path);
    }

    let lower = app_id.to_ascii_lowercase();
    let tail = app_id.rsplit('.').next().unwrap_or(app_id).to_ascii_lowercase();

    for name in [app_id, lower.as_str(), tail.as_str()] {
        if name.is_empty() {
            continue;
        }
        if let Some(path) = find_icon_file(name) {
            return Some(path);
        }
    }

    None
}

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

fn icon_name_for(app_id: &str) -> Option<String> {
    icon_name_in(&data_dirs(), app_id)
}

fn icon_name_in(dirs: &[PathBuf], app_id: &str) -> Option<String> {
    if app_id.is_empty() {
        return None;
    }

    let mut fallback = None;

    for dir in dirs {
        let apps = dir.join("applications");

        for candidate in [
            apps.join(format!("{app_id}.desktop")),
            apps.join(format!("{}.desktop", app_id.to_ascii_lowercase())),
        ] {
            if let Some(icon) = read_icon_key(&candidate) {
                return Some(icon);
            }
        }

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

fn find_icon_file(name: &str) -> Option<PathBuf> {
    if name.starts_with('/') {
        let path = PathBuf::from(name);
        return path.exists().then_some(path);
    }

    for theme in theme_search_order() {
        for dir in data_dirs() {
            let root = dir.join("icons").join(&theme);
            if !root.is_dir() {
                continue;
            }
            if let Some(found) = best_in_theme(&root, name) {
                return Some(found);
            }
        }
    }

    for dir in data_dirs() {
        for flat in [dir.join("pixmaps"), dir.join("icons")] {
            for ext in ["png", "svg"] {
                let path = flat.join(format!("{name}.{ext}"));
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn theme_search_order() -> Vec<String> {
    let mut order = Vec::new();
    let mut push = |name: String| {
        if !name.is_empty() && !order.contains(&name) {
            order.push(name);
        }
    };

    if let Some(theme) = configured_theme() {
        push(theme.clone());
        for parent in inherited_themes(&theme) {
            push(parent);
        }
    }
    push("Adwaita".to_owned());
    push("breeze".to_owned());
    push("hicolor".to_owned());
    order
}

fn configured_theme() -> Option<String> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;

    for rel in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
        let Ok(text) = std::fs::read_to_string(config.join(rel)) else {
            continue;
        };
        if let Some(name) = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("gtk-icon-theme-name="))
        {
            return Some(name.trim().to_owned());
        }
    }
    None
}

fn inherited_themes(theme: &str) -> Vec<String> {
    inherited_themes_in(&data_dirs(), theme)
}

fn inherited_themes_in(dirs: &[PathBuf], theme: &str) -> Vec<String> {
    for dir in dirs {
        let index = dir.join("icons").join(theme).join("index.theme");
        let Ok(text) = std::fs::read_to_string(index) else {
            continue;
        };
        if let Some(line) = text.lines().find_map(|l| l.trim().strip_prefix("Inherits=")) {
            return line
                .split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}

fn best_in_theme(root: &Path, name: &str) -> Option<PathBuf> {
    let png = format!("{name}.png");
    let svg = format!("{name}.svg");

    let mut best_png: Option<(u32, PathBuf)> = None;

    let mut stack = vec![(root.to_path_buf(), 0u32)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > 3 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }

            let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
                continue;
            };

            if file == svg {
                return Some(path);
            }
            if file == png {
                let score = size_hint(&path);
                if best_png.as_ref().is_none_or(|(best, _)| score > *best) {
                    best_png = Some((score, path));
                }
            }
        }
    }

    best_png.map(|(_, path)| path)
}

fn size_hint(path: &Path) -> u32 {
    let text = path.to_string_lossy();
    if text.contains("symbolic") {
        return 0;
    }
    path.components()
        .filter_map(|c| c.as_os_str().to_str())
        .filter_map(|part| part.split('x').next().and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(1)
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

    let drawn = path
        .and_then(|p| {
            let is_svg = p.extension().and_then(|e| e.to_str()) == Some("svg");
            if is_svg {
                draw_svg(&cr, p, size).ok()
            } else {
                draw_png(&cr, p, size).ok()
            }
        })
        .is_some();
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

fn draw_svg(cr: &cairo::Context, path: &Path, size: i32) -> anyhow::Result<()> {
    let data = std::fs::read(path)?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(size as u32, size as u32)
        .ok_or_else(|| anyhow::anyhow!("could not allocate a {size}px pixmap"))?;

    let tree_size = tree.size();
    let scale = size as f32 / tree_size.width().max(tree_size.height()).max(1.);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let stride = cairo::Format::ARgb32.stride_for_width(size as u32)?;
    let mut argb = vec![0u8; (stride * size) as usize];
    for y in 0..size as usize {
        for x in 0..size as usize {
            let src = (y * size as usize + x) * 4;
            let dst = y * stride as usize + x * 4;
            argb[dst] = pixmap.data()[src + 2];
            argb[dst + 1] = pixmap.data()[src + 1];
            argb[dst + 2] = pixmap.data()[src];
            argb[dst + 3] = pixmap.data()[src + 3];
        }
    }

    let icon = ImageSurface::create_for_data(argb, cairo::Format::ARgb32, size, size, stride)?;
    cr.set_source_surface(&icon, 0., 0.)?;
    cr.paint()?;
    Ok(())
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

#[cfg(test)]
mod lookup_tests {
    use super::*;

    fn theme(root: &Path) {
        for rel in [
            "icons/Fake/16x16/apps",
            "icons/Fake/256x256/apps",
            "icons/Fake/48x48/apps",
            "icons/Fake/scalable/apps",
            "icons/Fake/symbolic/apps",
            "applications",
        ] {
            std::fs::create_dir_all(root.join(rel)).unwrap();
        }
        std::fs::write(root.join("icons/Fake/index.theme"), "[Icon Theme]\nInherits=hicolor\n")
            .unwrap();
    }

    #[test]
    fn the_biggest_raster_wins_and_symbolic_never_does() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        theme(root);
        for rel in ["16x16", "48x48", "256x256", "symbolic"] {
            std::fs::write(root.join(format!("icons/Fake/{rel}/apps/thing.png")), b"x").unwrap();
        }

        let found = best_in_theme(&root.join("icons/Fake"), "thing").unwrap();
        assert!(
            found.to_string_lossy().contains("256x256"),
            "picked {found:?} instead of the 256px one"
        );
    }

    #[test]
    fn a_scalable_icon_is_found_and_preferred() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        theme(root);
        std::fs::write(root.join("icons/Fake/48x48/apps/thing.png"), b"x").unwrap();
        std::fs::write(root.join("icons/Fake/scalable/apps/thing.svg"), b"<svg/>").unwrap();

        let found = best_in_theme(&root.join("icons/Fake"), "thing").unwrap();
        assert_eq!(found.extension().unwrap(), "svg", "found {found:?}");
    }

    #[test]
    fn a_desktop_file_gives_up_its_icon_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        theme(root);
        std::fs::write(
            root.join("applications/thing.desktop"),
            "[Desktop Entry]\nName=Thing\nIcon=thing-icon\n",
        )
        .unwrap();

        let found = icon_name_in(&[root.to_path_buf()], "thing");
        assert_eq!(found.as_deref(), Some("thing-icon"));
    }

    #[test]
    fn startup_wm_class_is_matched_too() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        theme(root);
        std::fs::write(
            root.join("applications/some-vendor-app.desktop"),
            "[Desktop Entry]\nName=App\nIcon=vendor-app\nStartupWMClass=VendorApp\n",
        )
        .unwrap();

        let found = icon_name_in(&[root.to_path_buf()], "VendorApp");
        assert_eq!(found.as_deref(), Some("vendor-app"));
    }

    #[test]
    fn a_theme_declares_what_it_inherits() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        theme(root);
        let found = inherited_themes_in(&[root.to_path_buf()], "Fake");
        assert_eq!(found, vec!["hicolor".to_owned()]);
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;

    #[test]
    fn an_icon_named_after_the_app_is_found_without_a_desktop_file() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("icons/hicolor/scalable/apps");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("thing.svg"), b"<svg/>").unwrap();

        let found = best_in_theme(&dir.path().join("icons/hicolor"), "thing");
        assert!(found.is_some(), "an icon named after the app should be found");
    }
}
