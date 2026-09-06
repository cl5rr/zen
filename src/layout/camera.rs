use std::time::Duration;

use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::animation::{Animation, Clock};
use crate::rubber_band::RubberBand;

const ZOOM_RUBBER_BAND: RubberBand = RubberBand {
    stiffness: 0.5,
    limit: 0.3,
};

#[derive(Debug, Clone, Copy)]
struct PanDrive {
    dir: Point<f64, Logical>,
    speed: f64,
    held_until: Duration,
}

// zoom
const ZOOM_WHILE_PANNING: f64 = 0.22;

const PAN_START_SPEED: f64 = 260.;
const PAN_MAX_SPEED: f64 = 3400.;
const PAN_ACCEL: f64 = 2600.;
const PAN_DECEL: f64 = 5200.;
const PAN_HELD_GRACE_MS: u64 = 180;

#[derive(Debug)]
pub struct Camera {
    pan: Point<f64, Logical>,
    zoom: f64,

    view_size: Size<f64, Logical>,

    pan_anim: Option<(Animation, Animation)>,
    zoom_anim: Option<Animation>,
    zoom_anchor: Option<Point<f64, Logical>>,

    drive: Option<PanDrive>,
    last_advance: Option<Duration>,

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
            zoom_anchor: None,
            drive: None,
            last_advance: None,
            min_zoom,
            max_zoom: max_zoom.max(min_zoom),
            clock,
        }
    }

    // transforms

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

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

    pub fn visible_rect(&self) -> Rectangle<f64, Logical> {
        let size = Size::from((self.view_size.w / self.zoom, self.view_size.h / self.zoom));
        Rectangle::new(self.view_to_content(Point::from((0., 0.))), size)
    }

    pub fn set_view_size(&mut self, view_size: Size<f64, Logical>) {
        self.view_size = view_size;
    }

    pub fn is_at_rest(&self) -> bool {
        (self.zoom - 1.).abs() < 1e-9 && self.pan.x.abs() < 1e-9 && self.pan.y.abs() < 1e-9
    }

    // zoom

    fn band_zoom(&self, zoom: f64) -> f64 {
        let z = zoom.max(1e-6).ln();
        ZOOM_RUBBER_BAND
            .clamp(self.min_zoom.ln(), self.max_zoom.ln(), z)
            .exp()
    }

    fn clamp_zoom(&self, zoom: f64) -> f64 {
        zoom.clamp(self.min_zoom, self.max_zoom)
    }

    pub fn set_zoom_immediate(&mut self, zoom: f64) {
        self.zoom_anim = None;
        self.zoom_anchor = None;
        self.zoom = self.clamp_zoom(zoom);
    }

    // A wheel notch is a target, not a jump. The pan is not animated alongside the
    // zoom: it is recomputed from the live zoom every frame, which is what holds the
    // point under the pointer still all the way through rather than only at the ends.
    pub fn zoom_about(
        &mut self,
        view_anchor: Point<f64, Logical>,
        factor: f64,
        config: zen_config::Animation,
    ) {
        // Zooming while the canvas is already moving is nearly always a slipped
        // finger, so a notch counts for much less then.
        let factor = if self.drive.is_some() {
            factor.powf(ZOOM_WHILE_PANNING)
        } else {
            factor
        };

        let current = match &self.zoom_anim {
            Some(anim) => anim.to(),
            None => self.zoom,
        };

        let target = self.band_zoom(current * factor);

        self.pan_anim = None;
        self.zoom_anchor = Some(view_anchor);
        self.start_zoom_anim(target, config);
    }

    pub fn animate_zoom_to(&mut self, target: f64, config: zen_config::Animation) {
        let target = self.clamp_zoom(target);
        self.start_zoom_anim(target, config);
    }

    fn start_zoom_anim(&mut self, target: f64, config: zen_config::Animation) {
        let from = self.zoom;
        let velocity = match &self.zoom_anim {
            Some(anim) => anim.current_velocity(),
            None => 0.,
        };

        self.zoom_anim = Some(Animation::new(
            self.clock.clone(),
            from,
            target,
            velocity,
            config,
        ));
    }

    pub fn settle_zoom(&mut self, config: zen_config::Animation) {
        let clamped = self.clamp_zoom(self.zoom);
        if (clamped - self.zoom).abs() > 1e-9 {
            self.animate_zoom_to(clamped, config);
        }
    }

    // pan

    pub fn set_pan_immediate(&mut self, pan: Point<f64, Logical>) {
        self.pan_anim = None;
        self.pan = pan;
    }

    pub fn pan_by_view_delta(&mut self, delta: Point<f64, Logical>) {
        self.pan += delta;
        self.pan_anim = None;
    }

    pub fn pan_drive(&mut self, dir: Point<f64, Logical>) {
        let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
        if len <= f64::EPSILON {
            return;
        }
        let dir = Point::from((dir.x / len, dir.y / len));
        let held_until = self.clock.now_unadjusted() + Duration::from_millis(PAN_HELD_GRACE_MS);

        let speed = match self.drive {
            Some(d) if d.dir.x * dir.x + d.dir.y * dir.y > 0. => d.speed,
            _ => PAN_START_SPEED,
        };

        self.last_advance.get_or_insert(self.clock.now_unadjusted());

        self.pan_anim = None;
        self.drive = Some(PanDrive {
            dir,
            speed,
            held_until,
        });
    }

    fn advance_drive(&mut self, dt: f64) -> bool {
        let Some(mut d) = self.drive else {
            return false;
        };

        let held = self.clock.now_unadjusted() < d.held_until;
        d.speed = if held {
            (d.speed + PAN_ACCEL * dt).min(PAN_MAX_SPEED)
        } else {
            d.speed - PAN_DECEL * dt
        };

        if d.speed <= 0. {
            self.drive = None;
            return false;
        }

        self.pan += d.dir.upscale(d.speed * dt);
        self.drive = Some(d);
        true
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

    pub fn reset(&mut self, config: zen_config::Animation) {
        self.animate_zoom_to(1., config);
        self.animate_pan_to(Point::from((0., 0.)), config);
    }

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
        let zoom = self.clamp_zoom((avail_w / rect.size.w).min(avail_h / rect.size.h));

        let rect_center = rect.loc + Point::from((rect.size.w / 2., rect.size.h / 2.));
        let view_center = Point::from((self.view_size.w / 2., self.view_size.h / 2.));
        let pan = view_center - rect_center.upscale(zoom);

        self.animate_zoom_to(zoom, config);
        self.animate_pan_to(pan, config);
    }

    // animation

    pub fn is_animating(&self) -> bool {
        self.zoom_anim.is_some() || self.pan_anim.is_some() || self.drive.is_some()
    }

    pub fn advance_animations(&mut self) {
        let now = self.clock.now_unadjusted();
        let dt = self
            .last_advance
            .map_or(0., |prev| now.saturating_sub(prev).as_secs_f64())
            .clamp(0., 0.05);
        self.last_advance = Some(now);

        if dt > 0. {
            self.advance_drive(dt);
        }

        if let Some(anim) = &self.zoom_anim {
            let done = anim.is_done();
            let next = if done { anim.to() } else { anim.value() };

            // Correct the pan by the zoom *change*, reading the pan as it stands after
            // the drive has already moved it this frame. Assigning it outright would
            // hold the anchor but silently undo panning for the length of the zoom.
            if let Some(view_anchor) = self.zoom_anchor {
                let under = (view_anchor - self.pan).downscale(self.zoom);
                self.pan = view_anchor - under.upscale(next);
            }

            self.zoom = next;
            if done {
                self.zoom_anim = None;
                self.zoom_anchor = None;
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

    fn advance(c: &mut Camera, ms: u64) {
        let now = c.clock.now_unadjusted() + Duration::from_millis(ms);
        c.clock.set_unadjusted(now);
        c.advance_animations();
    }

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

    #[test]
    fn zoom_holds_the_anchor_still_every_frame() {
        let mut clock = Clock::with_time(Duration::ZERO);
        let config = zen_config::animations::OverviewOpenCloseAnim::default().0;

        // The clock is shared and only ever moves forward: rewinding it for each
        // anchor would put the spring before its own start time and it would sit
        // still, which looks exactly like a broken animation.
        let mut now = 0u64;

        for anchor in [(0., 0.), (640., 360.), (1279., 719.), (200., 500.)] {
            let anchor = Point::<f64, Logical>::from(anchor);
            let mut c = Camera::new(clock.clone(), Size::from((1280., 720.)), 0.1, 10.);

            let before = c.view_to_content(anchor);
            c.zoom_about(anchor, 1.6, config);

            for step in 1..=60 {
                now += 10;
                clock.set_unadjusted(Duration::from_millis(now));
                c.advance_animations();

                let at = c.view_to_content(anchor);
                assert!(
                    approx(before, at, 1e-6),
                    "anchor {anchor:?} drifted to {at:?} from {before:?} at frame {step}"
                );
                c.verify_invariants();
            }

            assert!(c.zoom > 1.05, "zoom barely moved, ended at {}", c.zoom);
        }
    }

    #[test]
    fn a_zoom_does_not_stall_a_pan_in_progress() {
        let mut clock = Clock::with_time(Duration::ZERO);
        let config = zen_config::animations::OverviewOpenCloseAnim::default().0;
        let mut c = Camera::new(clock.clone(), Size::from((1280., 720.)), 0.1, 10.);

        c.pan_drive(Point::from((-1., 0.)));

        let mut now = 0u64;
        let advance = |c: &mut Camera, clock: &mut Clock, now: &mut u64, frames: u64| {
            for _ in 0..frames {
                *now += 16;
                clock.set_unadjusted(Duration::from_millis(*now));
                c.pan_drive(Point::from((-1., 0.)));
                c.advance_animations();
            }
        };

        advance(&mut c, &mut clock, &mut now, 10);
        let before_zoom = c.pan;

        c.zoom_about(Point::from((640., 360.)), 1.6, config);
        advance(&mut c, &mut clock, &mut now, 10);
        let during_zoom = c.pan;

        assert!(
            (during_zoom.x - before_zoom.x).abs() > 1.,
            "the pan stalled while zooming: {before_zoom:?} then {during_zoom:?}"
        );
        assert!(c.zoom > 1., "the zoom did not run, ended at {}", c.zoom);
        c.verify_invariants();
    }

    #[test]
    fn zoom_rubber_bands_past_limits_but_stays_finite() {
        let mut clock = Clock::with_time(Duration::ZERO);
        let config = zen_config::animations::OverviewOpenCloseAnim::default().0;
        let mut c = Camera::new(clock.clone(), Size::from((1280., 720.)), 0.1, 10.);
        let anchor = Point::<f64, Logical>::from((640., 360.));
        let mut now = 0u64;

        for _ in 0..50 {
            c.zoom_about(anchor, 2.0, config);
            for _ in 0..40 {
                now += 10;
                clock.set_unadjusted(Duration::from_millis(now));
                c.advance_animations();
            }
        }
        assert!(
            c.zoom() > c.max_zoom,
            "should be allowed to overshoot the max, got {}",
            c.zoom()
        );
        assert!(
            c.zoom() < c.max_zoom * 3.,
            "overshoot should be bounded, got {}",
            c.zoom()
        );

        for _ in 0..80 {
            c.zoom_about(anchor, 0.5, config);
            for _ in 0..40 {
                now += 10;
                clock.set_unadjusted(Duration::from_millis(now));
                c.advance_animations();
            }
        }
        assert!(
            c.zoom() < c.min_zoom,
            "should be allowed to overshoot the min, got {}",
            c.zoom()
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

    #[test]
    fn a_held_pan_starts_slow_and_winds_up() {
        let mut c = camera();
        let left = Point::<f64, Logical>::from((-1., 0.));

        let mut covered = Vec::new();
        let mut prev = c.pan_offset_view().x;
        for _ in 0..6 {
            c.pan_drive(left);
            advance(&mut c, 16);
            let now = c.pan_offset_view().x;
            covered.push(prev - now);
            prev = now;
        }

        assert!(
            covered.iter().all(|d| *d > 0.),
            "a held key must move the canvas every frame, got {covered:?}"
        );
        for w in covered.windows(2) {
            assert!(
                w[1] > w[0],
                "each frame of a held key must cover more than the last: {covered:?}"
            );
        }
    }

    #[test]
    fn releasing_coasts_to_a_stop() {
        let mut c = camera();
        for _ in 0..8 {
            c.pan_drive(Point::from((-1., 0.)));
            advance(&mut c, 16);
        }

        let mut moved_after_release = 0.;
        let mut prev = c.pan_offset_view().x;
        for _ in 0..60 {
            advance(&mut c, 16);
            let now = c.pan_offset_view().x;
            moved_after_release += prev - now;
            prev = now;
        }
        assert!(
            moved_after_release > 0.,
            "releasing should coast, not stop dead"
        );

        let settled = c.pan_offset_view().x;
        for _ in 0..30 {
            advance(&mut c, 16);
        }
        assert!(
            (c.pan_offset_view().x - settled).abs() < 0.001,
            "once stopped it must stay stopped, drifted to {}",
            c.pan_offset_view().x
        );
    }

    #[test]
    fn reversing_starts_again_from_rest() {
        let mut c = camera();
        for _ in 0..10 {
            c.pan_drive(Point::from((-1., 0.)));
            advance(&mut c, 16);
        }

        let before = c.pan_offset_view().x;
        c.pan_drive(Point::from((1., 0.)));
        advance(&mut c, 16);
        let first_back = c.pan_offset_view().x - before;

        let expected = PAN_START_SPEED * 0.016;
        assert!(
            first_back < expected * 2.,
            "reversing carried momentum: moved {first_back}, expected about {expected}"
        );
    }
}
