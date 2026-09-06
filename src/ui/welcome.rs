use anyhow::Context as _;
use std::cell::RefCell;

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::reexports::gbm::Format as Fourcc;
use keyframe::EasingFunction as _;
use smithay::utils::{Logical, Point, Size, Transform};

use crate::animation::{Animation, Clock, CubicBezier};
use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::renderer::ZenRenderer;
use crate::render_helpers::solid_color::{SolidColorBuffer, SolidColorRenderElement};
use crate::render_helpers::texture::{TextureBuffer, TextureRenderElement};
use crate::zen_render_elements;

static THUMBNAIL_PNG: &[u8] = include_bytes!("../../resources/zen-thumbnail.png");

// timeline
const INTRO_MS: u64 = 380;

const HOLD_MS: u64 = 700;

const PART_MS: u64 = 1150;

const TOTAL_MS: u64 = INTRO_MS + HOLD_MS + PART_MS;

const INTRO_END: f64 = INTRO_MS as f64 / TOTAL_MS as f64;

const PART_START: f64 = (INTRO_MS + HOLD_MS) as f64 / TOTAL_MS as f64;

// mark
const MARK_FADE: f64 = 0.45;

const MARK_GROW: f64 = 0.055;

// seam
const SEAM_SPAN: f64 = 0.55;

const SEAM_TEX_W: i32 = 256;

const SEAM_TEX_H: i32 = 96;

const SEAM_HEIGHT: f64 = 88.;

const SEAM_COLOR: [f64; 3] = [0.90, 0.95, 1.];

zen_render_elements! {
    WelcomeRenderElement => {
        Panel = SolidColorRenderElement,
        Mark = PrimaryGpuTextureRenderElement,
    }
}

#[derive(Debug)]
pub struct Welcome {
    // Started on the first frame, not at construction.
    //
    // The whole animation is 2.2 seconds. Between building this and drawing anything,
    // a TTY session still has to scan connectors, set a mode, build a GLES context,
    // start Xwayland and spawn the startup apps. That routinely takes longer than the
    // animation lasts, so a timer started here would be finished before the first
    // frame and the welcome would never be seen on the one boot it exists for. Nested,
    // where startup is nearly instant, it looked fine.
    clock: Clock,
    anim: RefCell<Option<Animation>>,
    mark: RefCell<Option<TextureBuffer<GlesTexture>>>,
    mark_loaded: RefCell<bool>,
    panels: RefCell<(SolidColorBuffer, SolidColorBuffer)>,
    seam: RefCell<Option<(TextureBuffer<GlesTexture>, TextureBuffer<GlesTexture>)>>,
}

impl Welcome {
    pub fn new(clock: Clock, color: [f32; 4]) -> Self {
        Self {
            clock,
            anim: RefCell::new(None),
            mark: RefCell::new(None),
            mark_loaded: RefCell::new(false),
            panels: RefCell::new((
                SolidColorBuffer::new(Size::from((0., 0.)), color),
                SolidColorBuffer::new(Size::from((0., 0.)), color),
            )),
            seam: RefCell::new(None),
        }
    }

    // Not started is not done: it has to survive until something draws it.
    pub fn is_done(&self) -> bool {
        self.anim.borrow().as_ref().is_some_and(Animation::is_done)
    }

    pub fn are_animations_ongoing(&self) -> bool {
        !self.is_done()
    }

    fn progress(&self) -> f64 {
        let mut anim = self.anim.borrow_mut();
        let anim = anim.get_or_insert_with(|| {
            Animation::ease(
                self.clock.clone(),
                0.,
                1.,
                0.,
                TOTAL_MS,
                crate::animation::Curve::Linear,
            )
        });
        anim.clamped_value().clamp(0., 1.)
    }

    fn load<R: ZenRenderer>(&self, renderer: &mut R, scale: f64) {
        if *self.mark_loaded.borrow() {
            return;
        }
        *self.mark_loaded.borrow_mut() = true;

        let renderer = renderer.as_gles_renderer();

        *self.mark.borrow_mut() = load_mark(renderer, scale)
            .map_err(|err| warn!("error loading the welcome mark: {err:?}"))
            .ok();

        *self.seam.borrow_mut() = load_seam(renderer, scale)
            .map_err(|err| warn!("error building the welcome seam: {err:?}"))
            .ok();
    }

    pub fn render<R: ZenRenderer>(
        &self,
        renderer: &mut R,
        view_size: Size<f64, Logical>,
        scale: f64,
        push: &mut dyn FnMut(WelcomeRenderElement),
    ) {
        if self.is_done() {
            return;
        }

        let raw = self.progress();

        let intro = smoothstep((raw / INTRO_END).clamp(0., 1.));
        let part = ((raw - PART_START) / (1. - PART_START)).clamp(0., 1.);

        let opening = CubicBezier::new(0.62, 0., 0.14, 1.).y(part);

        let half = view_size.h / 2.;
        let travel = half * opening;

        let dissolve = smoothstep((part / MARK_FADE).clamp(0., 1.));
        let alpha = intro * (1. - dissolve);

        // mark
        if alpha > 0.004 {
            self.load(renderer, scale);

            if let Some(mark) = self.mark.borrow().as_ref() {
                let base = mark.logical_size();
                let grow = 1. + MARK_GROW * dissolve;
                let size = Size::from((base.w * grow, base.h * grow));
                let location =
                    Point::from(((view_size.w - size.w) / 2., (view_size.h - size.h) / 2.));

                push(
                    PrimaryGpuTextureRenderElement(TextureRenderElement::from_texture_buffer(
                        mark.clone(),
                        location,
                        alpha as f32,
                        None,
                        Some(size),
                        Kind::Unspecified,
                    ))
                    .into(),
                );
            }
        }

        // seam
        let u = (part / SEAM_SPAN).clamp(0., 1.);
        let spill = (4. * u * (1. - u)) as f32;
        if spill > 0.004 {
            self.load(renderer, scale);

            if let Some((down, up)) = self.seam.borrow().as_ref() {
                let size = Size::from((view_size.w, SEAM_HEIGHT));

                push(
                    PrimaryGpuTextureRenderElement(TextureRenderElement::from_texture_buffer(
                        down.clone(),
                        Point::from((0., half - travel)),
                        spill,
                        None,
                        Some(size),
                        Kind::Unspecified,
                    ))
                    .into(),
                );
                push(
                    PrimaryGpuTextureRenderElement(TextureRenderElement::from_texture_buffer(
                        up.clone(),
                        Point::from((0., half + travel - SEAM_HEIGHT)),
                        spill,
                        None,
                        Some(size),
                        Kind::Unspecified,
                    ))
                    .into(),
                );
            }
        }

        // panels
        let mut panels = self.panels.borrow_mut();
        panels.0.resize(Size::from((view_size.w, half)));
        panels.1.resize(Size::from((view_size.w, half)));

        push(
            SolidColorRenderElement::from_buffer(
                &panels.0,
                Point::from((0., -travel)),
                1.,
                Kind::Unspecified,
            )
            .into(),
        );
        push(
            SolidColorRenderElement::from_buffer(
                &panels.1,
                Point::from((0., half + travel)),
                1.,
                Kind::Unspecified,
            )
            .into(),
        );
    }
}

fn load_seam(
    renderer: &mut GlesRenderer,
    scale: f64,
) -> anyhow::Result<(TextureBuffer<GlesTexture>, TextureBuffer<GlesTexture>)> {
    let (w, h) = (SEAM_TEX_W, SEAM_TEX_H);
    let mut down = vec![0u8; (w * h * 4) as usize];
    let mut up = vec![0u8; (w * h * 4) as usize];

    for y in 0..h {
        let d = y as f64;
        let core = 0.95 * (-d / 6.).exp() + 0.12 * (-d / 34.).exp();

        for x in 0..w {
            let u = x as f64 / (w - 1) as f64;
            let env = 0.16 + 0.84 * (1. - (2. * u - 1.).abs()).powf(1.6);
            let a = (core * env).clamp(0., 1.);

            let px = [
                (SEAM_COLOR[2] * a * 255.) as u8,
                (SEAM_COLOR[1] * a * 255.) as u8,
                (SEAM_COLOR[0] * a * 255.) as u8,
                (a * 255.) as u8,
            ];

            let i = ((y * w + x) * 4) as usize;
            down[i..i + 4].copy_from_slice(&px);

            let j = (((h - 1 - y) * w + x) * 4) as usize;
            up[j..j + 4].copy_from_slice(&px);
        }
    }

    let mut make = |buf: &[u8]| {
        TextureBuffer::from_memory(
            renderer,
            buf,
            Fourcc::Argb8888,
            (w, h),
            false,
            scale,
            Transform::Normal,
            Vec::new(),
        )
    };

    let down = make(&down)?;
    let up = make(&up)?;
    Ok((down, up))
}

fn smoothstep(x: f64) -> f64 {
    x * x * (3. - 2. * x)
}

fn load_mark(renderer: &mut GlesRenderer, scale: f64) -> anyhow::Result<TextureBuffer<GlesTexture>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(THUMBNAIL_PNG));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);

    let mut reader = decoder.read_info()?;
    let size = reader
        .output_buffer_size()
        .context("png reported no output buffer size")?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf)?;
    buf.truncate(info.buffer_size());

    let (width, height) = (info.width as i32, info.height as i32);

    anyhow::ensure!(
        info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight,
        "expected 8-bit RGBA after transformation, got {:?}/{:?}",
        info.color_type,
        info.bit_depth
    );

    for px in buf.chunks_exact_mut(4) {
        let a = u32::from(px[3]);
        let (r, g, b) = (u32::from(px[0]), u32::from(px[1]), u32::from(px[2]));
        px[0] = ((b * a) / 255) as u8;
        px[1] = ((g * a) / 255) as u8;
        px[2] = ((r * a) / 255) as u8;
    }

    let buffer = TextureBuffer::from_memory(
        renderer,
        &buf,
        Fourcc::Argb8888,
        (width, height),
        false,
        scale,
        Transform::Normal,
        Vec::new(),
    )?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // The bug: the timer used to start at construction, and a TTY session spends longer
    // than the whole animation getting to its first frame, so the welcome was finished
    // before anything could draw it. It has to survive an arbitrary startup delay.
    #[test]
    fn a_slow_startup_does_not_consume_the_welcome() {
        let mut clock = Clock::with_time(Duration::ZERO);
        let welcome = Welcome::new(clock.clone(), [1., 1., 1., 1.]);

        // Far longer than the animation, exactly as a cold boot would be.
        clock.set_unadjusted(Duration::from_secs(30));
        assert!(
            !welcome.is_done(),
            "the welcome expired before anything drew it"
        );
        assert_eq!(welcome.progress(), 0., "it should begin at the first frame");

        clock.set_unadjusted(Duration::from_secs(30) + Duration::from_millis(TOTAL_MS / 2));
        let half = welcome.progress();
        assert!(half > 0.4 && half < 0.6, "half way through, got {half}");

        clock.set_unadjusted(Duration::from_secs(30) + Duration::from_millis(TOTAL_MS + 50));
        assert!(welcome.is_done(), "it should finish once it has actually run");
    }
}
