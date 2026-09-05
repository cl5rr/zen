use crate::appearance::{Color, WorkspaceShadow, WorkspaceShadowPart, DEFAULT_BACKDROP_COLOR};
use crate::utils::{Flag, MergeWith};
use crate::FloatOrInt;

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct SpawnAtStartup {
    #[knuffel(arguments)]
    pub command: Vec<String>,
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct SpawnShAtStartup {
    #[knuffel(argument)]
    pub command: String,
}

#[derive(Debug, PartialEq)]
pub struct Cursor {
    pub xcursor_theme: String,
    pub xcursor_size: u8,
    pub hide_when_typing: bool,
    pub hide_after_inactive_ms: Option<u32>,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            xcursor_theme: String::from("default"),
            xcursor_size: 24,
            hide_when_typing: false,
            hide_after_inactive_ms: None,
        }
    }
}

#[derive(knuffel::Decode, Debug, PartialEq)]
pub struct CursorPart {
    #[knuffel(child, unwrap(argument))]
    pub xcursor_theme: Option<String>,
    #[knuffel(child, unwrap(argument))]
    pub xcursor_size: Option<u8>,
    #[knuffel(child)]
    pub hide_when_typing: Option<Flag>,
    #[knuffel(child, unwrap(argument))]
    pub hide_after_inactive_ms: Option<u32>,
}

impl MergeWith<CursorPart> for Cursor {
    fn merge_with(&mut self, part: &CursorPart) {
        merge_clone!((self, part), xcursor_theme, xcursor_size);
        merge!((self, part), hide_when_typing);
        merge_clone_opt!((self, part), hide_after_inactive_ms);
    }
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq)]
pub struct ScreenshotPath(#[knuffel(argument)] pub Option<String>);

impl Default for ScreenshotPath {
    fn default() -> Self {
        Self(Some(String::from(
            "~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png",
        )))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyOverlay {
    pub skip_at_startup: bool,
    pub hide_not_bound: bool,
}

#[derive(knuffel::Decode, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyOverlayPart {
    #[knuffel(child)]
    pub skip_at_startup: Option<Flag>,
    #[knuffel(child)]
    pub hide_not_bound: Option<Flag>,
}

impl MergeWith<HotkeyOverlayPart> for HotkeyOverlay {
    fn merge_with(&mut self, part: &HotkeyOverlayPart) {
        merge!((self, part), skip_at_startup, hide_not_bound);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConfigNotification {
    pub disable_failed: bool,
}

#[derive(knuffel::Decode, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConfigNotificationPart {
    #[knuffel(child)]
    pub disable_failed: Option<Flag>,
}

impl MergeWith<ConfigNotificationPart> for ConfigNotification {
    fn merge_with(&mut self, part: &ConfigNotificationPart) {
        merge!((self, part), disable_failed);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Clipboard {
    pub disable_primary: bool,
}

#[derive(knuffel::Decode, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardPart {
    #[knuffel(child)]
    pub disable_primary: Option<Flag>,
}

impl MergeWith<ClipboardPart> for Clipboard {
    fn merge_with(&mut self, part: &ClipboardPart) {
        merge!((self, part), disable_primary);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overview {
    pub zoom: f64,
    pub backdrop_color: Color,
    pub workspace_shadow: WorkspaceShadow,
}

impl Default for Overview {
    fn default() -> Self {
        Self {
            zoom: 0.5,
            backdrop_color: DEFAULT_BACKDROP_COLOR,
            workspace_shadow: WorkspaceShadow::default(),
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Copy, PartialEq)]
pub struct OverviewPart {
    #[knuffel(child, unwrap(argument))]
    pub zoom: Option<FloatOrInt<0, 1>>,
    #[knuffel(child)]
    pub backdrop_color: Option<Color>,
    #[knuffel(child)]
    pub workspace_shadow: Option<WorkspaceShadowPart>,
}

impl MergeWith<OverviewPart> for Overview {
    fn merge_with(&mut self, part: &OverviewPart) {
        merge!((self, part), zoom, workspace_shadow);
        merge_clone!((self, part), backdrop_color);
    }
}

/// Camera limits and step size.
///
/// The camera is per-output; these are the bounds it operates within.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Furthest the camera can zoom out.
    pub min_zoom: f64,
    /// Furthest the camera can zoom in. Above 1.0 magnifies.
    pub max_zoom: f64,
    /// Multiplier applied per zoom-in step (and its reciprocal per zoom-out).
    pub zoom_step: f64,
    /// Whether floating windows may live outside the viewport.
    ///
    /// This is ZEN's whole premise, so it defaults on. Turning it off restores zen's
    /// behaviour of pinning every floating window to its output.
    pub infinite_canvas: bool,
    /// Whether new windows open onto the canvas rather than into the scrolling strip.
    ///
    /// The canvas is where islands live, so this is what makes ZEN ZEN rather than zen with a
    /// camera bolted on. Turning it off gives you the inherited 1D scrolling layout, which is
    /// still there and still works -- window rules can send individual windows either way.
    pub open_on_canvas: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            min_zoom: 0.2,
            max_zoom: 4.0,
            zoom_step: 1.25,
            infinite_canvas: true,
            open_on_canvas: true,
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Copy, PartialEq)]
pub struct CameraPart {
    #[knuffel(child, unwrap(argument))]
    pub min_zoom: Option<FloatOrInt<0, 1>>,
    #[knuffel(child, unwrap(argument))]
    pub max_zoom: Option<FloatOrInt<1, 64>>,
    #[knuffel(child, unwrap(argument))]
    pub zoom_step: Option<FloatOrInt<1, 4>>,
    #[knuffel(child, unwrap(argument))]
    pub infinite_canvas: Option<bool>,
    #[knuffel(child, unwrap(argument))]
    pub open_on_canvas: Option<bool>,
}

impl MergeWith<CameraPart> for Camera {
    fn merge_with(&mut self, part: &CameraPart) {
        merge!((self, part), min_zoom, max_zoom, zoom_step);
        merge_clone!((self, part), infinite_canvas, open_on_canvas);
    }
}

/// The glass material applied to windows.
///
/// `opacity` spans the whole range the material can be: 0.0 is pure refraction, glass you see
/// straight through; intermediate values frost it toward `tint`; 1.0 is fully opaque and the
/// backdrop stops showing. Refraction still shapes the rim at any opacity, so an opaque pane
/// reads as a solid object rather than a flat rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    pub off: bool,
    /// 0.0 = fully transparent (pure refraction), 1.0 = fully opaque.
    pub opacity: f64,
    /// Colour the material tints toward as opacity rises.
    pub tint: Color,
    /// How far, in logical pixels, the rim bends what is behind it.
    pub refraction: f64,
    /// Distance over which refraction decays inward. Small values keep it a rim.
    pub falloff: f64,
    /// Superellipse exponent for corners. 2 is a circular arc; 4-5 is continuous curvature.
    pub squircle: f64,
    /// Saturation lift applied to the backdrop. Vibrancy is blur *plus* saturation; blur alone
    /// reads as frosted plastic.
    pub saturation: f64,
    pub specular: f64,
}

impl Default for Glass {
    fn default() -> Self {
        Self {
            off: true,
            opacity: 0.15,
            tint: Color::new_unpremul(1., 1., 1., 1.),
            refraction: 12.,
            falloff: 18.,
            squircle: 4.5,
            saturation: 1.3,
            specular: 0.12,
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Copy, PartialEq, Default)]
pub struct GlassPart {
    #[knuffel(child)]
    pub off: bool,
    #[knuffel(child, unwrap(argument))]
    pub opacity: Option<FloatOrInt<0, 1>>,
    #[knuffel(child)]
    pub tint: Option<Color>,
    #[knuffel(child, unwrap(argument))]
    pub refraction: Option<FloatOrInt<0, 128>>,
    #[knuffel(child, unwrap(argument))]
    pub falloff: Option<FloatOrInt<0, 512>>,
    #[knuffel(child, unwrap(argument))]
    pub squircle: Option<FloatOrInt<2, 16>>,
    #[knuffel(child, unwrap(argument))]
    pub saturation: Option<FloatOrInt<0, 4>>,
    #[knuffel(child, unwrap(argument))]
    pub specular: Option<FloatOrInt<0, 1>>,
}

impl MergeWith<GlassPart> for Glass {
    fn merge_with(&mut self, part: &GlassPart) {
        self.off = part.off;
        merge!((self, part), opacity, refraction, falloff, squircle, saturation, specular);
        merge_clone!((self, part), tint);
    }
}

/// ZEN's welcome animation: a cover that parts to reveal the desktop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Welcome {
    pub off: bool,
    /// Colour of the parting panels.
    pub color: Color,
}

impl Default for Welcome {
    fn default() -> Self {
        Self {
            off: false,
            color: Color::new_unpremul(0.96, 0.96, 0.97, 1.),
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Copy, PartialEq, Default)]
pub struct WelcomePart {
    #[knuffel(child)]
    pub off: bool,
    #[knuffel(child)]
    pub color: Option<Color>,
}

impl MergeWith<WelcomePart> for Welcome {
    fn merge_with(&mut self, part: &WelcomePart) {
        self.off = part.off;
        merge_clone!((self, part), color);
    }
}

/// Widgets drawn onto the canvas itself, rather than pinned to a screen edge.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Widgets {
    pub clock: Clock,
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Default)]
pub struct WidgetsPart {
    #[knuffel(child)]
    pub clock: Option<ClockPart>,
}

impl MergeWith<WidgetsPart> for Widgets {
    fn merge_with(&mut self, part: &WidgetsPart) {
        merge!((self, part), clock);
    }
}

/// A clock positioned on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct Clock {
    pub off: bool,
    /// Canvas coordinates, in logical pixels.
    pub position: (f64, f64),
    /// strftime format string.
    pub format: String,
    /// Pango font description.
    pub font: String,
    pub color: Color,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            off: true,
            position: (48., 48.),
            format: String::from("%H:%M"),
            font: String::from("sans 48px"),
            color: Color::new_unpremul(1., 1., 1., 0.85),
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Default)]
pub struct ClockPart {
    #[knuffel(child)]
    pub off: bool,
    #[knuffel(child, unwrap(argument))]
    pub x: Option<FloatOrInt<-1000000, 1000000>>,
    #[knuffel(child, unwrap(argument))]
    pub y: Option<FloatOrInt<-1000000, 1000000>>,
    #[knuffel(child, unwrap(argument))]
    pub format: Option<String>,
    #[knuffel(child, unwrap(argument))]
    pub font: Option<String>,
    #[knuffel(child)]
    pub color: Option<Color>,
}

impl MergeWith<ClockPart> for Clock {
    fn merge_with(&mut self, part: &ClockPart) {
        self.off = part.off;
        if let Some(x) = part.x {
            self.position.0 = x.0;
        }
        if let Some(y) = part.y {
            self.position.1 = y.0;
        }
        merge_clone!((self, part), format, font, color);
    }
}

#[derive(knuffel::Decode, Debug, Default, Clone, PartialEq, Eq)]
pub struct Environment(#[knuffel(children)] pub Vec<EnvironmentVariable>);

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentVariable {
    #[knuffel(node_name)]
    pub name: String,
    #[knuffel(argument)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XwaylandSatellite {
    pub off: bool,
    pub path: String,
}

impl Default for XwaylandSatellite {
    fn default() -> Self {
        Self {
            off: false,
            path: String::from("xwayland-satellite"),
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct XwaylandSatellitePart {
    #[knuffel(child)]
    pub off: bool,
    #[knuffel(child)]
    pub on: bool,
    #[knuffel(child, unwrap(argument))]
    pub path: Option<String>,
}

impl MergeWith<XwaylandSatellitePart> for XwaylandSatellite {
    fn merge_with(&mut self, part: &XwaylandSatellitePart) {
        self.off |= part.off;
        if part.on {
            self.off = false;
        }

        merge_clone!((self, part), path);
    }
}
