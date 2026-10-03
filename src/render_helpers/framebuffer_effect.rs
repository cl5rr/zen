use std::cell::RefCell;

use glam::{Mat3, Vec2};
use zen_config::CornerRadius;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::gles::{
    ffi, GlesError, GlesFrame, GlesRenderer, GlesTexture, Uniform,
};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Frame as _, FrameContext, Offscreen, Texture as _};
use smithay::gpu_span_location;
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Logical, Physical, Rectangle, Scale, Size, Transform};

use crate::backend::tty::{TtyFrame, TtyRenderer, TtyRendererError};
use crate::render_helpers::background_effect::RenderParams;
use crate::render_helpers::blur::{Blur, BlurOptions};
use crate::render_helpers::renderer::AsGlesFrame as _;
use crate::render_helpers::shaders::{mat3_uniform, Shaders};
use crate::utils::region::TransformedRegion;

#[derive(Debug)]
pub struct FramebufferEffect {
    id: Id,
    commit: CommitCounter,
}

#[derive(Debug)]
pub struct FramebufferEffectElement {
    id: Id,
    commit: CommitCounter,
    geometry: Rectangle<f64, Logical>,
    clip_geo: Rectangle<f64, Logical>,
    corner_radius: CornerRadius,
    subregion: Option<TransformedRegion>,
    scale: f32,
    blur_options: Option<BlurOptions>,
    noise: f32,
    saturation: f32,
    glass: Option<GlassParams>,
    mask: Option<SurfaceMask>,
}

// mask

pub const MASK_THRESHOLD: f32 = 0.08;

#[derive(Debug, Clone)]
pub struct SurfaceMask {
    pub texture: GlesTexture,
    pub rect: Rectangle<f64, Logical>,
}

pub fn geo_to_mask(
    clip: Rectangle<f64, Logical>,
    mask: Rectangle<f64, Logical>,
    flip_y: bool,
) -> Mat3 {
    let clip_loc = Vec2::new(clip.loc.x as f32, clip.loc.y as f32);
    let clip_size = Vec2::new(clip.size.w as f32, clip.size.h as f32);
    let mask_loc = Vec2::new(mask.loc.x as f32, mask.loc.y as f32);
    let mask_size = Vec2::new(mask.size.w as f32, mask.size.h as f32).max(Vec2::ONE);

    let m = Mat3::from_translation((clip_loc - mask_loc) / mask_size)
        * Mat3::from_scale(clip_size / mask_size);
    if flip_y {
        Mat3::from_translation(Vec2::new(0., 1.)) * Mat3::from_scale(Vec2::new(1., -1.)) * m
    } else {
        m
    }
}

// glass

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassParams {
    pub tint: [f32; 4],
    pub refraction: f32,
    pub falloff: f32,
    pub squircle: f32,
    pub specular: f32,
    pub spec_power: f32,
    pub light_dir: (f32, f32),
}

impl From<zen_config::Glass> for GlassParams {
    fn from(g: zen_config::Glass) -> Self {
        let [r, gr, b, _] = g.tint.to_array_unpremul();
        Self {
            tint: [r, gr, b, g.opacity as f32],
            refraction: g.refraction as f32,
            falloff: g.falloff as f32,
            squircle: g.squircle as f32,
            specular: g.specular as f32,
            spec_power: 24.,
            light_dir: (0., -1.),
        }
    }
}

#[derive(Debug)]
struct Inner {
    framebuffer: Option<GlesTexture>,
    blur: Option<Blur>,
    intermediate: Option<GlesTexture>,
    subregion_damage: Vec<Rectangle<i32, Physical>>,
}

impl FramebufferEffect {
    pub fn new() -> Self {
        Self {
            id: Id::new(),
            commit: CommitCounter::default(),
        }
    }

    pub fn damage(&mut self) {
        self.commit.increment();
    }

    pub fn render(
        &self,
        ns: Option<usize>,
        params: RenderParams,
        blur_options: Option<BlurOptions>,
        noise: f32,
        saturation: f32,
        glass: Option<GlassParams>,
    ) -> FramebufferEffectElement {
        let (clip_geo, corner_radius) = params
            .clip
            .unwrap_or((params.geometry, CornerRadius::default()));

        let mut id = self.id.clone();
        if let Some(ns) = ns {
            id = id.namespaced(ns);
        }

        FramebufferEffectElement {
            id,
            commit: self.commit,
            geometry: params.geometry,
            clip_geo,
            corner_radius,
            subregion: params.subregion,
            scale: params.scale as f32,
            blur_options,
            noise,
            saturation,
            glass,
            mask: params.mask,
        }
    }
}

impl FramebufferEffectElement {
    fn compute_uniforms(
        &self,
        crop: Rectangle<f64, Logical>,
        transform: Transform,
    ) -> Vec<Uniform<'static>> {
        let offset = crop.loc - (self.clip_geo.loc - self.geometry.loc);
        let offset = Vec2::new(offset.x as f32, offset.y as f32);
        let crop_size = Vec2::new(crop.size.w as f32, crop.size.h as f32);
        let clip_size = Vec2::new(self.clip_geo.size.w as f32, self.clip_geo.size.h as f32);

        let input_to_clip_geo =
            Mat3::from_scale(crop_size / clip_size) * Mat3::from_translation(offset / crop_size);

        let transform_mat = Mat3::from_translation(Vec2::new(0.5, 0.5))
            * transform.matrix()
            * Mat3::from_translation(Vec2::new(-0.5, -0.5));
        let input_to_clip_geo = input_to_clip_geo * transform_mat;

        let clip_geo_size = (self.clip_geo.size.w as f32, self.clip_geo.size.h as f32);

        let mut uniforms = vec![
            Uniform::new("zen_scale", self.scale),
            Uniform::new("geo_size", clip_geo_size),
            Uniform::new("corner_radius", <[f32; 4]>::from(self.corner_radius)),
            mat3_uniform("input_to_geo", input_to_clip_geo),
            Uniform::new("noise", self.noise),
            Uniform::new("saturation", self.saturation),
            Uniform::new("bg_color", [0f32, 0., 0., 0.]),
        ];

        if let Some(g) = self.glass {
            uniforms.extend([
                Uniform::new("glass_tint", g.tint),
                Uniform::new("refraction_strength", g.refraction),
                Uniform::new("refraction_falloff", g.falloff),
                Uniform::new("squircle_n", g.squircle),
                Uniform::new("spec_strength", g.specular),
                Uniform::new("spec_power", g.spec_power),
                Uniform::new("light_dir", g.light_dir),
            ]);

            let (use_mask, to_mask) = match &self.mask {
                Some(m) => (
                    1f32,
                    geo_to_mask(self.clip_geo, m.rect, m.texture.is_y_inverted()),
                ),
                None => (0f32, Mat3::IDENTITY),
            };
            uniforms.extend([
                Uniform::new("use_mask", use_mask),
                Uniform::new("mask_tex", 1i32),
                mat3_uniform("geo_to_mask", to_mask),
                Uniform::new("mask_threshold", MASK_THRESHOLD),
            ]);
        }

        uniforms
    }
}

impl Element for FramebufferEffectElement {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        let size = self.geometry.size.to_buffer(1., Transform::Normal);
        Rectangle::from_size(size)
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.geometry.to_physical_precise_round(scale)
    }

    fn is_framebuffer_effect(&self) -> bool {
        true
    }
}

impl RenderElement<GlesRenderer> for FramebufferEffectElement {
    fn capture_framebuffer(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        cache: &UserDataMap,
    ) -> Result<(), GlesError> {
        let _span = tracy_client::span!("FramebufferEffectElement::capture_framebuffer");
        let location = gpu_span_location!("FramebufferEffectElement::capture_framebuffer");
        frame.with_gpu_span(location, |frame| {
            let output_rect = Rectangle::from_size(frame.output_size());
            let transform = frame.transformation();

            let mut guard = frame.renderer();

            let inner = cache
                .get_or_insert::<RefCell<Inner>, _>(|| RefCell::new(Inner::new(guard.as_mut())));
            let mut inner = inner.borrow_mut();
            let inner = &mut *inner;

            inner.intermediate = None;

            let clamped_dst = match dst.intersection(output_rect) {
                Some(clamped) => clamped,
                None => return Ok(()),
            };
            let clamp_scale = clamped_dst.size.to_f64() / dst.size.to_f64();

            let dst = transform.transform_rect_in(clamped_dst, &output_rect.size);

            let size = src
                .size
                .to_logical(1., Transform::Normal)
                .upscale(clamp_scale)
                .to_physical_precise_round(self.scale);
            let size = transform.transform_size(size);

            let size = size.to_logical(1).to_buffer(1, Transform::Normal);

            let size = Size::from((size.w.max(1), size.h.max(1)));

            if inner
                .framebuffer
                .as_ref()
                .is_some_and(|fb| fb.size() != size)
            {
                inner.framebuffer = None;
            }
            let framebuffer = if let Some(fb) = &inner.framebuffer {
                fb
            } else {
                trace!("creating framebuffer texture sized {} × {}", size.w, size.h);
                let renderer = guard.as_mut();
                let texture = renderer.create_buffer(Fourcc::Abgr8888, size)?;
                inner.framebuffer.insert(texture)
            };

            let mut blur = Option::zip(inner.blur.as_mut(), self.blur_options);
            if let Some((b, options)) = &mut blur {
                let renderer = guard.as_mut();
                if let Err(err) = b.prepare_textures(
                    |fourcc, size| renderer.create_buffer(fourcc, size),
                    framebuffer,
                    *options,
                ) {
                    warn!("error preparing blur textures: {err:?}");
                    blur = None;
                }
            }

            drop(guard);

            frame.with_context(|gl| unsafe {
                while gl.GetError() != ffi::NO_ERROR {}

                let mut current_fbo = 0i32;
                gl.GetIntegerv(ffi::DRAW_FRAMEBUFFER_BINDING, &mut current_fbo as *mut _);

                gl.Disable(ffi::SCISSOR_TEST);

                let mut fbo = 0;
                gl.GenFramebuffers(1, &mut fbo as *mut _);
                gl.BindFramebuffer(ffi::DRAW_FRAMEBUFFER, fbo);

                gl.FramebufferTexture2D(
                    ffi::DRAW_FRAMEBUFFER,
                    ffi::COLOR_ATTACHMENT0,
                    ffi::TEXTURE_2D,
                    framebuffer.tex_id(),
                    0,
                );

                gl.BlitFramebuffer(
                    dst.loc.x,
                    dst.loc.y,
                    dst.loc.x + dst.size.w,
                    dst.loc.y + dst.size.h,
                    0,
                    0,
                    size.w,
                    size.h,
                    ffi::COLOR_BUFFER_BIT,
                    ffi::LINEAR,
                );

                gl.BindFramebuffer(ffi::DRAW_FRAMEBUFFER, current_fbo as u32);
                gl.Enable(ffi::SCISSOR_TEST);

                gl.DeleteFramebuffers(1, &mut fbo as *mut _);

                if gl.GetError() != ffi::NO_ERROR {
                    Err(GlesError::BlitError)
                } else {
                    Ok(())
                }
            })??;

            if self.blur_options.is_none() {
                inner.intermediate = Some(framebuffer.clone());
                return Ok(());
            }

            if let Some((blur, options)) = blur {
                let mut guard = frame.renderer();
                let renderer = guard.as_mut();
                match blur.render(renderer, framebuffer, options) {
                    Ok(blurred) => inner.intermediate = Some(blurred),
                    Err(err) => {
                        warn!("error rendering blur: {err:?}");
                    }
                }
            }

            Ok(())
        })
    }

    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        let Some(cache) = cache else {
            return Ok(());
        };
        let Some(inner) = cache.get::<RefCell<Inner>>() else {
            return Ok(());
        };
        let mut inner = inner.borrow_mut();
        let inner = &mut *inner;

        let Some(texture) = &inner.intermediate else {
            return Ok(());
        };

        let output_rect = Rectangle::from_size(frame.output_size());
        let clamped_dst = match dst.intersection(output_rect) {
            Some(clamped) => clamped,
            None => return Ok(()),
        };
        let clamp_offset = clamped_dst.loc - dst.loc;

        let filtered = &mut inner.subregion_damage;
        filtered.clear();

        if let Some(subregion) = &self.subregion {
            let mut crop = src.to_logical(1., Transform::Normal, &src.size);
            crop.loc += self.geometry.loc;
            subregion.filter_damage(crop, dst, damage, filtered);
        } else {
            filtered.extend(damage.iter());
        };

        if clamped_dst != dst {
            let r = Rectangle::new(clamp_offset, clamped_dst.size);
            filtered.retain_mut(|d| {
                if let Some(mut crop) = d.intersection(r) {
                    crop.loc -= clamp_offset;
                    *d = crop;
                    true
                } else {
                    false
                }
            });
        }

        if filtered.is_empty() {
            return Ok(());
        }
        let damage = &filtered[..];

        let src_loc = src.loc.to_logical(1., Transform::Normal, &src.size);
        let dst_to_src = src.size / dst.size.to_f64();
        let crop = Rectangle::new(
            src_loc + clamp_offset.to_f64().upscale(dst_to_src).to_logical(1.),
            clamped_dst.size.to_f64().upscale(dst_to_src).to_logical(1.),
        );

        let shaders = Shaders::get_from_frame(frame);
        let program = if self.glass.is_some() {
            shaders.glass_and_clip.clone()
        } else {
            shaders.postprocess_and_clip.clone()
        };
        let uniforms = program
            .is_some()
            .then(|| self.compute_uniforms(crop, frame.transformation()));
        let uniforms = uniforms.as_ref().map_or(&[][..], |x| &x[..]);

        let mask = self.mask.as_ref().filter(|_| self.glass.is_some() && program.is_some());
        if let Some(m) = mask {
            let id = m.texture.tex_id();
            frame.with_context(|gl| unsafe {
                gl.ActiveTexture(ffi::TEXTURE1);
                gl.BindTexture(ffi::TEXTURE_2D, id);
                gl.ActiveTexture(ffi::TEXTURE0);
            })?;
        }

        let res = frame.render_texture_from_to(
            texture,
            Rectangle::from_size(texture.size().to_f64()),
            clamped_dst,
            damage,
            &[],
            frame.transformation().invert(),
            1.,
            program.as_ref(),
            uniforms,
        );

        if mask.is_some() {
            frame.with_context(|gl| unsafe {
                gl.ActiveTexture(ffi::TEXTURE1);
                gl.BindTexture(ffi::TEXTURE_2D, 0);
                gl.ActiveTexture(ffi::TEXTURE0);
            })?;
        }

        res
    }
}

impl<'render> RenderElement<TtyRenderer<'render>> for FramebufferEffectElement {
    fn capture_framebuffer(
        &self,
        frame: &mut TtyFrame<'_, '_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        cache: &UserDataMap,
    ) -> Result<(), TtyRendererError<'render>> {
        let gles_frame = frame.as_gles_frame();
        RenderElement::<GlesRenderer>::capture_framebuffer(&self, gles_frame, src, dst, cache)?;
        Ok(())
    }

    fn draw(
        &self,
        frame: &mut TtyFrame<'_, '_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), TtyRendererError<'render>> {
        let gles_frame = frame.as_gles_frame();
        RenderElement::<GlesRenderer>::draw(
            &self,
            gles_frame,
            src,
            dst,
            damage,
            opaque_regions,
            cache,
        )?;
        Ok(())
    }
}

impl Inner {
    fn new(renderer: &mut GlesRenderer) -> Self {
        Inner {
            framebuffer: None,
            blur: Blur::new(renderer),
            intermediate: None,
            subregion_damage: Vec::new(),
        }
    }
}
