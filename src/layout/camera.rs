//! Per-output camera: the viewport transform through which one monitor sees its content.
//!
//! zen had no camera. Zoom was a bare `f64` derived from overview progress, applied ad-hoc at
//! roughly 130 call sites, and it was global -- every output zoomed together, always. ZEN needs
//! the opposite: a camera is a property of an *output*, so two monitors can look at different
//! regions of the same space, and a virtual output can frame something different again.
//!
//! # Coordinate spaces
//!
//! - **content space**: where windows actually live, unscaled. Today this is workspace-local;
//!   in Phase 2.5 it becomes the unbounded canvas. It is tagged `Logical` for now because that
//!   is what the layout still speaks.
//! - **view space**: pixels within the output, what the user points at.
//!
//! ```text
//! view    = content * zoom + pan
//! content = (view - pan) / zoom
//! ```
//!
//! `pan` is where content-space `(0, 0)` lands on screen -- origin-anchored, not
//! centre-anchored. That choice is deliberate. The layout already positions content as
//! `content * zoom + geo.loc`, so the camera slots in as one extra term on `geo.loc` rather
//! than requiring every call site to learn about centres. A centre-anchored camera would also
//! fight `workspaces_render_geo`, which re-centres the workspace itself as zoom changes.
//!
//! Nothing is lost: the *feel* of zooming comes from [`Camera::zoom_about`] holding a chosen
//! point still, not from where the origin happens to sit.

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::animation::{Animation, Clock};
use crate::rubber_band::RubberBand;

/// Overshoot allowed past the configured zoom limits before the camera springs back.
///
/// Applied in log space, so it means "a constant fraction past the limit" rather than a
/// constant number of zoom units -- zoom is multiplicative, not additive.
const ZOOM_RUBBER_BAND: RubberBand = RubberBand {
    stiffness: 0.5,
    limit: 0.3,
};

#[derive(Debug)]
pub struct Camera {
    /// Where content-space `(0, 0)` lands in view space.
    pan: Point<f64, Logical>,
    /// Content pixels per view pixel. Above 1.0 is magnification.
    zoom: f64,

    /// Size of the viewport in view space.
    view_size: Size<f64, Logical>,

    /// In-flight animations. Pan is split per axis so each can retarget independently.
    pan_anim: Option<(Animation, Animation)>,
    zoom_anim: Option<Animation>,

    min_zoom: f64,
    max_zoom: f64,

    clock: Clock,
}

impl Camera {
    pub fn new(clock: Clock, view_size: Size<f64, Logical>, min_zoom: f64, max_zoom: f64) -> Self {
        let min_zoom = min_zoom.max(1e-4);
        Self {
            pan: Point::from((0., 0.)),
            zoom: 1.,
            view_size,
            pan_anim: None,
            zoom_anim: None,
            min_zoom,
            max_zoom: max_zoom.max(min_zoom),
            clock,
        }
    }

    // ---------------------------------------------------------------- transforms ----

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    /// Where content-space `(0, 0)` lands in view space. Zero when the camera is at rest.
    pub fn pan_offset_view(&self) -> Point<f64, Logical> {
        self.pan
    }

    pub fn view_size(&self) -> Size<f64, Logical> {
        self.view_size
    }

    pub fn content_to_view(&self, p: Point<f64, Logical>) -> Point<f64, Logical> {
        p.upscale(self.zoom) + self.pan
    }

    pub fn view_to_content(&self, p: Point<f64, Logical>) -> Point<f64, Logical> {
        (p - self.pan).downscale(self.zoom)
    }

    /// The content-space rectangle currently visible, for culling.
    pub fn visible_rect(&self) -> Rectangle<f64, Logical> {
        let size = Size::from((self.view_size.w / self.zoom, self.view_size.h / self.zoom));
        Rectangle::new(self.view_to_content(Point::from((0., 0.))), size)
    }

    pub fn set_view_size(&mut self, view_size: Size<f64, Logical>) {
        self.view_size = view_size;
    }

    /// Whether the camera is an identity transform, i.e. contributing nothing.
    pub fn is_at_rest(&self) -> bool {
        (self.zoom - 1.).abs() < 1e-9 && self.pan.x.abs() < 1e-9 && self.pan.y.abs() < 1e-9
    }

    // -------------------------------------------------------------------- zoom -----

    /// Clamps `zoom` to the configured range, with rubber-band resistance past the ends.
    ///
    /// Done in log space: overshooting by a factor should feel the same whether you are at 0.5x
    /// or 4x, which a linear clamp would not give.
    fn band_zoom(&self, zoom: f64) -> f64 {
        let z = zoom.max(1e-6).ln();
        ZOOM_RUBBER_BAND
            .clamp(self.min_zoom.ln(), self.max_zoom.ln(), z)
            .exp()
    }

    /// Hard clamp with no overshoot, for animation targets.
    fn clamp_zoom(&self, zoom: f64) -> f64 {
        zoom.clamp(self.min_zoom, self.max_zoom)
    }

    pub fn set_zoom_immediate(&mut self, zoom: f64) {
        self.zoom_anim = None;
        self.zoom = self.clamp_zoom(zoom);
    }

    /// Zoom by `factor` while holding the content under `view_anchor` still.
    ///
    /// This is the primitive that makes wheel-zoom feel right: whatever is under the cursor
    /// stays under the cursor. Without it, zooming drifts the world away from the pointer.
    pub fn zoom_about(&mut self, view_anchor: Point<f64, Logical>, factor: f64) {
        let anchor_content = self.view_to_content(view_anchor);
        let new_zoom = self.band_zoom(self.zoom * factor);

        // Solve `anchor_content * new_zoom + pan = view_anchor` for pan.
        self.pan = view_anchor - anchor_content.upscale(new_zoom);
        self.zoom = new_zoom;

        self.zoom_anim = None;
        self.pan_anim = None;
    }

    /// Springs zoom towards `target`, carrying the current velocity if one is in flight.
    pub fn animate_zoom_to(&mut self, target: f64, config: zen_config::Animation) {
        let target = self.clamp_zoom(target);
        let from = self.zoom;
        let velocity = self
            .zoom_anim
            .as_ref()
            .map_or(0., Animation::current_velocity);

        self.zoom_anim = Some(Animation::new(
            self.clock.clone(),
            from,
            target,
            velocity,
            config,
        ));
    }

    /// Releases the camera after a gesture: if it overshot the limits, spring it back.
    pub fn settle_zoom(&mut self, config: zen_config::Animation) {
        let clamped = self.clamp_zoom(self.zoom);
        if (clamped - self.zoom).abs() > 1e-9 {
            self.animate_zoom_to(clamped, config);
        }
    }

    // -------------------------------------------------------------------- pan ------

    pub fn set_pan_immediate(&mut self, pan: Point<f64, Logical>) {
        self.pan_anim = None;
        self.pan = pan;
    }

    /// Pans by a delta given in *view* pixels.
    ///
    /// Motion arrives in screen space, so dragging tracks the pointer exactly at any zoom
    /// without the caller scaling anything.
    pub fn pan_by_view_delta(&mut self, delta: Point<f64, Logical>) {
        self.pan += delta;
        self.pan_anim = None;
    }

    pub fn animate_pan_to(&mut self, target: Point<f64, Logical>, config: zen_config::Animation) {
        let (vx, vy) = self.pan_anim.as_ref().map_or((0., 0.), |(x, y)| {
            (x.current_velocity(), y.current_velocity())
        });
        self.pan_anim = Some((
            Animation::new(self.clock.clone(), self.pan.x, target.x, vx, config),
            Animation::new(self.clock.clone(), self.pan.y, target.y, vy, config),
        ));
    }

    /// Springs back to an identity transform.
    pub fn reset(&mut self, config: zen_config::Animation) {
        self.animate_zoom_to(1., config);
        self.animate_pan_to(Point::from((0., 0.)), config);
    }

    /// Frames `rect` (content space) in the viewport with `padding` view pixels of margin.
    ///
    /// This is the primitive Phase 3's camera-maximize calls. It never touches the window.
    pub fn frame(
        &mut self,
        rect: Rectangle<f64, Logical>,
        padding: f64,
        config: zen_config::Animation,
    ) {
        if rect.size.w <= 0. || rect.size.h <= 0. {
            return;
        }
        let avail_w = (self.view_size.w - padding * 2.).max(1.);
        let avail_h = (self.view_size.h - padding * 2.).max(1.);
        // Fit, so the whole rect is visible and its aspect ratio preserved.
        let zoom = self.clamp_zoom((avail_w / rect.size.w).min(avail_h / rect.size.h));

        // Put the rect's centre at the viewport's centre.
        let rect_center = rect.loc + Point::from((rect.size.w / 2., rect.size.h / 2.));
        let view_center = Point::from((self.view_size.w / 2., self.view_size.h / 2.));
        let pan = view_center - rect_center.upscale(zoom);

        self.animate_zoom_to(zoom, config);
        self.animate_pan_to(pan, config);
    }

    // -------------------------------------------------------------- animation ------

    pub fn is_animating(&self) -> bool {
        self.zoom_anim.is_some() || self.pan_anim.is_some()
    }

    /// Samples in-flight animations and retires finished ones.
    pub fn advance_animations(&mut self) {
        if let Some(anim) = &self.zoom_anim {
            self.zoom = anim.value();
            if anim.is_done() {
                self.zoom = anim.to();
                self.zoom_anim = None;
            }
        }
        if let Some((x, y)) = &self.pan_anim {
            self.pan = Point::from((x.value(), y.value()));
            if x.is_done() && y.is_done() {
                self.pan = Point::from((x.to(), y.to()));
                self.pan_anim = None;
            }
        }
    }

    pub fn update_config(&mut self, min_zoom: f64, max_zoom: f64) {
        self.min_zoom = min_zoom.max(1e-4);
        self.max_zoom = max_zoom.max(self.min_zoom);
    }

    #[cfg(test)]
    fn verify_invariants(&self) {
        assert!(
            self.zoom.is_finite() && self.zoom > 0.,
            "zoom must be positive and finite"
        );
        assert!(self.pan.x.is_finite() && self.pan.y.is_finite());
        assert!(self.min_zoom <= self.max_zoom);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::animation::Clock;

    fn camera() -> Camera {
        Camera::new(
            Clock::with_time(Duration::ZERO),
            Size::from((1280., 720.)),
            0.1,
            10.,
        )
    }

    fn approx(a: Point<f64, Logical>, b: Point<f64, Logical>, tol: f64) -> bool {
        (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
    }

    /// A fresh camera must contribute nothing, or it would shift every existing layout.
    #[test]
    fn starts_at_rest_as_an_identity_transform() {
        let c = camera();
        assert!(c.is_at_rest());
        for p in [(0., 0.), (640., 360.), (-99., 1234.)] {
            let p = Point::<f64, Logical>::from(p);
            assert!(approx(c.content_to_view(p), p, 1e-12));
        }
    }

    #[test]
    fn transforms_round_trip_at_any_zoom() {
        let mut c = camera();
        for zoom in [0.25, 0.5, 1., 1.5, 3., 7.5] {
            c.set_zoom_immediate(zoom);
            c.set_pan_immediate(Point::from((123., -456.)));
            for p in [(0., 0.), (640., 360.), (-99., 1234.), (1280., 720.)] {
                let p = Point::<f64, Logical>::from(p);
                let round = c.view_to_content(c.content_to_view(p));
                assert!(
                    approx(round, p, 1e-9),
                    "zoom {zoom}: {p:?} round-tripped to {round:?}"
                );
            }
            c.verify_invariants();
        }
    }

    /// The property that makes wheel-zoom feel right.
    #[test]
    fn zoom_about_holds_the_anchor_still() {
        let mut c = camera();
        for anchor in [(0., 0.), (640., 360.), (1279., 719.), (200., 500.)] {
            let anchor = Point::<f64, Logical>::from(anchor);
            c.set_zoom_immediate(1.);
            c.set_pan_immediate(Point::from((0., 0.)));

            let before = c.view_to_content(anchor);
            for _ in 0..6 {
                c.zoom_about(anchor, 1.25);
            }
            let after = c.view_to_content(anchor);

            assert!(
                approx(before, after, 1e-6),
                "anchor {anchor:?} drifted from {before:?} to {after:?} while zooming"
            );
            c.verify_invariants();
        }
    }

    #[test]
    fn zoom_rubber_bands_past_limits_but_stays_finite() {
        let mut c = camera();
        let anchor = Point::<f64, Logical>::from((640., 360.));

        for _ in 0..50 {
            c.zoom_about(anchor, 2.0);
        }
        assert!(
            c.zoom() > c.max_zoom,
            "should be allowed to overshoot the max"
        );
        assert!(
            c.zoom() < c.max_zoom * 3.,
            "overshoot should be bounded, got {}",
            c.zoom()
        );

        for _ in 0..80 {
            c.zoom_about(anchor, 0.5);
        }
        assert!(
            c.zoom() < c.min_zoom,
            "should be allowed to overshoot the min"
        );
        assert!(c.zoom() > 0., "zoom must stay positive, got {}", c.zoom());
        c.verify_invariants();
    }

    #[test]
    fn pan_tracks_the_pointer_exactly() {
        let mut c = camera();
        for zoom in [0.5, 1., 2.5] {
            c.set_zoom_immediate(zoom);
            c.set_pan_immediate(Point::from((0., 0.)));

            let probe = Point::<f64, Logical>::from((300., 200.));
            let delta = Point::<f64, Logical>::from((60., -40.));

            // Grab the content under `probe`, drag by `delta`; it must end up under probe+delta.
            let grabbed = c.view_to_content(probe);
            c.pan_by_view_delta(delta);
            let now_under = c.view_to_content(probe + delta);

            assert!(
                approx(grabbed, now_under, 1e-9),
                "zoom {zoom}: grabbed content {grabbed:?} but it is now under {now_under:?}"
            );
        }
    }

    #[test]
    fn frame_fits_the_rect_inside_the_viewport() {
        let mut c = camera();
        let rect =
            Rectangle::<f64, Logical>::new(Point::from((1000., 2000.)), Size::from((400., 300.)));
        c.frame(rect, 20., zen_config::Animation::new_off());

        // Animations are pending; apply their targets directly for the geometry check.
        let z = c.zoom_anim.as_ref().unwrap().to();
        let (px, py) = {
            let (x, y) = c.pan_anim.as_ref().unwrap();
            (x.to(), y.to())
        };
        c.set_zoom_immediate(z);
        c.set_pan_immediate(Point::from((px, py)));

        let tl = c.content_to_view(rect.loc);
        let br = c.content_to_view(rect.loc + Point::from((rect.size.w, rect.size.h)));
        assert!(
            tl.x >= 19.9 && tl.y >= 19.9,
            "top-left {tl:?} should sit inside the padding"
        );
        assert!(
            br.x <= 1280. - 19.9 && br.y <= 720. - 19.9,
            "bottom-right {br:?} should sit inside the padding"
        );
    }
}
