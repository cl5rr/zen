use std::time::Duration;

use zen_config::OutputName;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, Offscreen};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::utils::{Logical, Physical, Point, Rectangle, Size, Transform};
use smithay::wayland::presentation::Refresh;

use super::{OutputId, RenderResult};
use crate::render_helpers::{RenderCtx, RenderTarget};
use crate::state::Zen;
use crate::utils::get_monotonic_time;

// preview
pub const PREVIEW_WIDTH: f64 = 0.25;
pub const PREVIEW_MAX_HEIGHT: f64 = 0.4;
pub const PREVIEW_MARGIN: f64 = 24.;

pub fn preview_rect(
    host: Size<f64, Logical>,
    virtual_size: Size<i32, Physical>,
) -> Option<Rectangle<f64, Logical>> {
    if host.w <= 0. || host.h <= 0. || virtual_size.w <= 0 || virtual_size.h <= 0 {
        return None;
    }

    let aspect = f64::from(virtual_size.h) / f64::from(virtual_size.w);
    let mut w = (host.w * PREVIEW_WIDTH).min(host.w - PREVIEW_MARGIN * 2.);
    let mut h = w * aspect;

    let tallest = host.h * PREVIEW_MAX_HEIGHT;
    if h > tallest {
        h = tallest;
        w = h / aspect;
    }
    if w < 1. || h < 1. {
        return None;
    }

    let x = (host.w - w - PREVIEW_MARGIN).max(0.);
    let y = (host.h - h - PREVIEW_MARGIN).max(0.);
    Some(Rectangle::new(Point::from((x, y)), Size::from((w, h))))
}

pub struct VirtualOutput {
    pub output: Output,
    pub id: OutputId,
    pub hidden: bool,
    pub preview: bool,
    size: Size<i32, smithay::utils::Physical>,
    damage: OutputDamageTracker,
    texture: Option<GlesTexture>,
}

impl VirtualOutput {
    pub fn new(name: &str, width: u16, height: u16, refresh: u32, hidden: bool) -> Self {
        let output = Output::new(
            name.to_owned(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "ZEN".to_owned(),
                model: "Virtual".to_owned(),
                serial_number: name.to_owned(),
            },
        );

        let mode = Mode {
            size: Size::from((i32::from(width), i32::from(height))),
            refresh: refresh as i32,
        };
        output.change_current_state(Some(mode), Some(Transform::Normal), None, None);
        output.set_preferred(mode);

        output.user_data().insert_if_missing(|| OutputName {
            connector: name.to_owned(),
            make: Some("ZEN".to_owned()),
            model: Some("Virtual".to_owned()),
            serial: Some(name.to_owned()),
        });

        Self {
            damage: OutputDamageTracker::from_output(&output),
            output,
            id: OutputId::next(),
            hidden,
            preview: false,
            size: mode.size,
            texture: None,
        }
    }

    pub fn preview_texture(&self) -> Option<(&GlesTexture, Size<i32, Physical>)> {
        if !self.preview {
            return None;
        }
        Some((self.texture.as_ref()?, self.size))
    }

    pub fn refresh_interval(&self) -> Duration {
        let refresh = self
            .output
            .current_mode()
            .map(|m| m.refresh)
            .filter(|r| *r > 0)
            .unwrap_or(60_000);
        Duration::from_secs_f64(1000. / f64::from(refresh))
    }

    pub fn render(&mut self, zen: &mut Zen, renderer: &mut GlesRenderer) -> RenderResult {
        let ctx = RenderCtx {
            renderer,
            target: RenderTarget::Output,
            xray: None,
        };
        let elements = zen.render_to_vec(ctx, &self.output, false);

        if self.texture.is_none() {
            let buffer_size = self.size.to_logical(1).to_buffer(1, Transform::Normal);
            match renderer.create_buffer(Fourcc::Abgr8888, buffer_size) {
                Ok(texture) => self.texture = Some(texture),
                Err(err) => {
                    warn!("virtual output {}: no texture: {err:?}", self.output.name());
                    return RenderResult::Skipped;
                }
            }
        }

        let texture = self.texture.as_mut().unwrap();
        let res = match renderer.bind(texture) {
            Ok(mut framebuffer) => {
                self.damage
                    .render_output(renderer, &mut framebuffer, 0, &elements, [0.; 4])
            }
            Err(err) => {
                warn!("virtual output {}: cannot bind: {err:?}", self.output.name());
                return RenderResult::Skipped;
            }
        };

        let Ok(res) = res else {
            return RenderResult::Skipped;
        };

        zen.update_primary_scanout_output(&self.output, &res.states);

        let mut feedback = zen.take_presentation_feedbacks(&self.output, &res.states);
        feedback.presented::<_, smithay::utils::Monotonic>(
            get_monotonic_time(),
            Refresh::Unknown,
            0,
            wp_presentation_feedback::Kind::empty(),
        );

        if res.damage.is_some() {
            RenderResult::Submitted
        } else {
            RenderResult::NoDamage
        }
    }
}
