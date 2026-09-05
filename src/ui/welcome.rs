use anyhow::Context as _;
use std::cell::RefCell;

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::reexports::gbm::Format as Fourcc;
use smithay::utils::{Logical, Point, Size, Transform};

use crate::animation::{Animation, Clock};
use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::renderer::ZenRenderer;
use crate::render_helpers::solid_color::{SolidColorBuffer, SolidColorRenderElement};
use crate::render_helpers::texture::{TextureBuffer, TextureRenderElement};
use crate::zen_render_elements;

static THUMBNAIL_PNG: &[u8] = include_bytes!("../../resources/zen-thumbnail.png");

const HOLD_MS: u64 = 1000;

const PART_MS: u64 = 900;

const TOTAL_MS: u64 = HOLD_MS + PART_MS;

const HOLD_FRACTION: f64 = HOLD_MS as f64 / TOTAL_MS as f64;

const FADE_FRACTION: f64 = 0.55;

zen_render_elements! {
    WelcomeRenderElement => {
        Panel = SolidColorRenderElement,
        Mark = PrimaryGpuTextureRenderElement,
    }
}

#[derive(Debug)]
pub struct Welcome {
    anim: Animation,
    mark: RefCell<Option<TextureBuffer<GlesTexture>>>,
    mark_loaded: RefCell<bool>,
    panels: RefCell<(SolidColorBuffer, SolidColorBuffer)>,
}

impl Welcome {
    pub fn new(clock: Clock, color: [f32; 4]) -> Self {
        Self {
            anim: Animation::ease(clock, 0., 1., 0., TOTAL_MS, crate::animation::Curve::Linear),
            mark: RefCell::new(None),
            mark_loaded: RefCell::new(false),
            panels: RefCell::new((
                SolidColorBuffer::new(Size::from((0., 0.)), color),
                SolidColorBuffer::new(Size::from((0., 0.)), color),
            )),
        }
    }

    pub fn is_done(&self) -> bool {
        self.anim.is_done()
    }

    pub fn are_animations_ongoing(&self) -> bool {
        !self.anim.is_done()
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

        let raw = self.anim.clamped_value().clamp(0., 1.);
        let progress = if raw <= HOLD_FRACTION {
            0.
        } else {
            let t = ((raw - HOLD_FRACTION) / (1. - HOLD_FRACTION)).clamp(0., 1.);
            1. - (1. - t).powi(3)
        };

        let half = view_size.h / 2.;

        let travel = half * progress;

        let alpha = 1. - (progress / FADE_FRACTION).clamp(0., 1.);
        if alpha > 0. {
            if !*self.mark_loaded.borrow() {
                *self.mark_loaded.borrow_mut() = true;
                *self.mark.borrow_mut() = load_mark(renderer.as_gles_renderer(), scale)
                    .map_err(|err| warn!("error loading the welcome mark: {err:?}"))
                    .ok();
            }

            if let Some(mark) = self.mark.borrow().as_ref() {
                let size = mark.logical_size();
                let location = Point::from((
                    (view_size.w - size.w) / 2.,
                    (view_size.h - size.h) / 2.,
                ));

                push(
                    PrimaryGpuTextureRenderElement(TextureRenderElement::from_texture_buffer(
                        mark.clone(),
                        location,
                        alpha as f32,
                        None,
                        None,
                        Kind::Unspecified,
                    ))
                    .into(),
                );
            }
        }

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
