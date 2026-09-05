use std::cell::RefCell;
use std::collections::HashMap;

use ordered_float::NotNan;
use pangocairo::cairo::{self, ImageSurface};
use pangocairo::pango::FontDescription;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::reexports::gbm::Format as Fourcc;
use smithay::utils::{Logical, Point, Transform};
use zen_config::Clock as ClockConfig;

use crate::render_helpers::primary_gpu_texture::PrimaryGpuTextureRenderElement;
use crate::render_helpers::renderer::ZenRenderer;
use crate::render_helpers::texture::{TextureBuffer, TextureRenderElement};
use crate::utils::{format_local_time, to_physical_precise_round};

#[derive(Debug)]
pub struct CanvasClock {
    buffers: RefCell<HashMap<(String, NotNan<f64>), Option<TextureBuffer<GlesTexture>>>>,
    config: ClockConfig,
}

impl CanvasClock {
    pub fn new(config: ClockConfig) -> Self {
        Self {
            buffers: RefCell::new(HashMap::new()),
            config,
        }
    }

    pub fn update_config(&mut self, config: ClockConfig) {
        if self.config != config {
            self.config = config;
            self.buffers.borrow_mut().clear();
        }
    }

    pub fn position(&self) -> Point<f64, Logical> {
        Point::from((self.config.position.0, self.config.position.1))
    }

    pub fn is_enabled(&self) -> bool {
        !self.config.off
    }

    pub fn text(&self) -> Option<String> {
        if !self.is_enabled() {
            return None;
        }
        format_local_time(&self.config.format)
    }

    pub fn render<R: ZenRenderer>(
        &self,
        renderer: &mut R,
        scale: f64,
    ) -> Option<PrimaryGpuTextureRenderElement> {
        let text = self.text()?;
        let key = (text.clone(), NotNan::new(scale).ok()?);

        let mut buffers = self.buffers.borrow_mut();
        if buffers.len() > 8 {
            buffers.retain(|(t, _), _| t == &text);
        }

        let buffer = buffers.entry(key).or_insert_with(|| {
            let renderer = renderer.as_gles_renderer();
            match render_text(renderer, scale, &text, &self.config) {
                Ok(buffer) => Some(buffer),
                Err(err) => {
                    warn!("error rendering the canvas clock: {err:?}");
                    None
                }
            }
        });

        let buffer = buffer.as_ref()?;
        Some(PrimaryGpuTextureRenderElement(
            TextureRenderElement::from_texture_buffer(
                buffer.clone(),
                self.position(),
                1.,
                None,
                None,
                Kind::Unspecified,
            ),
        ))
    }
}

fn render_text(
    renderer: &mut GlesRenderer,
    scale: f64,
    text: &str,
    cfg: &ClockConfig,
) -> anyhow::Result<TextureBuffer<GlesTexture>> {
    let _span = tracy_client::span!("canvas_clock::render_text");

    let mut font = FontDescription::from_string(&cfg.font);
    font.set_absolute_size(to_physical_precise_round(scale, font.size()));

    let surface = ImageSurface::create(cairo::Format::ARgb32, 0, 0)?;
    let cr = cairo::Context::new(&surface)?;
    let layout = pangocairo::functions::create_layout(&cr);
    layout.context().set_round_glyph_positions(false);
    layout.set_font_description(Some(&font));
    layout.set_text(text);

    let (width, height) = layout.pixel_size();
    let width = width.max(1);
    let height = height.max(1);

    let surface = ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let cr = cairo::Context::new(&surface)?;

    let layout = pangocairo::functions::create_layout(&cr);
    layout.context().set_round_glyph_positions(false);
    layout.set_font_description(Some(&font));
    layout.set_text(text);

    let [r, g, b, a] = cfg.color.to_array_unpremul();
    cr.set_source_rgba(r.into(), g.into(), b.into(), a.into());
    cr.move_to(0., 0.);
    pangocairo::functions::show_layout(&cr, &layout);
    drop(cr);

    let data = surface.take_data().unwrap();
    let buffer = TextureBuffer::from_memory(
        renderer,
        &data,
        Fourcc::Argb8888,
        (width, height),
        false,
        scale,
        Transform::Normal,
        Vec::new(),
    )?;

    Ok(buffer)
}
