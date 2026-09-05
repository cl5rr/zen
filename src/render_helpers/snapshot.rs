use std::cell::OnceCell;

use zen_config::BlockOutFrom;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::{Kind, RenderElement};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size, Transform};

use super::{render_to_encompassing_texture, ToRenderElement};
use crate::render_helpers::{RenderCtx, RenderTarget};

#[derive(Debug)]
pub struct RenderSnapshot<C, B> {
    pub contents: Vec<C>,

    pub contents_with_blocked_out_bg: Option<Vec<C>>,

    pub blocked_out_contents: Vec<B>,

    pub block_out_from: Option<BlockOutFrom>,

    pub size: Size<f64, Logical>,

    pub texture: OnceCell<Option<(GlesTexture, Rectangle<i32, Physical>)>>,

    pub texture_with_blocked_out_bg: OnceCell<Option<(GlesTexture, Rectangle<i32, Physical>)>>,

    pub blocked_out_texture: OnceCell<Option<(GlesTexture, Rectangle<i32, Physical>)>>,
}

impl<C, B, EC, EB> RenderSnapshot<C, B>
where
    C: ToRenderElement<RenderElement = EC>,
    B: ToRenderElement<RenderElement = EB>,
    EC: RenderElement<GlesRenderer>,
    EB: RenderElement<GlesRenderer>,
{
    pub fn texture(
        &self,
        ctx: RenderCtx<GlesRenderer>,
        scale: Scale<f64>,
    ) -> Option<&(GlesTexture, Rectangle<i32, Physical>)> {
        if ctx.target.should_block_out(self.block_out_from) {
            self.blocked_out_texture.get_or_init(|| {
                let _span = tracy_client::span!("RenderSnapshot::texture");

                let elements: Vec<_> = self
                    .blocked_out_contents
                    .iter()
                    .map(|baked| {
                        baked.to_render_element(Point::from((0., 0.)), scale, 1., Kind::Unspecified)
                    })
                    .collect();

                match render_to_encompassing_texture(
                    ctx.renderer,
                    scale,
                    Transform::Normal,
                    Fourcc::Abgr8888,
                    &elements,
                ) {
                    Ok((texture, _sync_point, geo)) => Some((texture, geo)),
                    Err(err) => {
                        warn!("error rendering blocked-out contents to texture: {err:?}");
                        None
                    }
                }
            })
        } else if ctx.target != RenderTarget::Output && self.contents_with_blocked_out_bg.is_some()
        {
            let contents = self.contents_with_blocked_out_bg.as_ref().unwrap();
            self.texture_with_blocked_out_bg.get_or_init(|| {
                let _span = tracy_client::span!("RenderSnapshot::texture");

                let elements: Vec<_> = contents
                    .iter()
                    .map(|baked| {
                        baked.to_render_element(Point::from((0., 0.)), scale, 1., Kind::Unspecified)
                    })
                    .collect();

                match render_to_encompassing_texture(
                    ctx.renderer,
                    scale,
                    Transform::Normal,
                    Fourcc::Abgr8888,
                    &elements,
                ) {
                    Ok((texture, _sync_point, geo)) => Some((texture, geo)),
                    Err(err) => {
                        warn!("error rendering contents with blocked-out bg to texture: {err:?}");
                        None
                    }
                }
            })
        } else {
            self.texture.get_or_init(|| {
                let _span = tracy_client::span!("RenderSnapshot::texture");

                let elements: Vec<_> = self
                    .contents
                    .iter()
                    .map(|baked| {
                        baked.to_render_element(Point::from((0., 0.)), scale, 1., Kind::Unspecified)
                    })
                    .collect();

                match render_to_encompassing_texture(
                    ctx.renderer,
                    scale,
                    Transform::Normal,
                    Fourcc::Abgr8888,
                    &elements,
                ) {
                    Ok((texture, _sync_point, geo)) => Some((texture, geo)),
                    Err(err) => {
                        warn!("error rendering contents to texture: {err:?}");
                        None
                    }
                }
            })
        }
        .as_ref()
    }
}
