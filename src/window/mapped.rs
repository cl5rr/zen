use std::cell::{Cell, Ref, RefCell};
use std::time::Duration;

use zen_config::{Color, Config, CornerRadius, GradientInterpolation, WindowRule};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::space::SpaceElement as _;
use smithay::desktop::{PopupKind, PopupManager, Window};
use smithay::output::{self, Output};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource as _;
use smithay::utils::{Logical, Point, Rectangle, Scale, Serial, Size, Transform};
use smithay::wayland::compositor::{remove_pre_commit_hook, with_states, HookId, SurfaceData};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::{
    SurfaceCachedState, ToplevelCachedState, ToplevelConfigure, ToplevelSurface,
    XdgToplevelSurfaceData,
};
use wayland_backend::server::Credentials;

use super::{ResolvedWindowRules, WindowRef};
use crate::handlers::KdeDecorationsModeState;
use crate::layout::{
    ConfigureIntent, InteractiveResizeData, LayoutElement, LayoutElementRenderElement,
    LayoutElementRenderSnapshot, SizingMode,
};
use crate::zen_render_elements;
use crate::render_helpers::background_effect::BackgroundEffectElement;
use crate::render_helpers::border::BorderRenderElement;
use crate::render_helpers::offscreen::OffscreenData;
use crate::render_helpers::renderer::ZenRenderer;
use crate::render_helpers::snapshot::RenderSnapshot;
use crate::render_helpers::solid_color::{SolidColorBuffer, SolidColorRenderElement};
use crate::render_helpers::surface::{
    push_elements_from_surface_tree, render_snapshot_from_surface_tree,
};
use crate::render_helpers::xray::XrayPos;
use crate::render_helpers::{background_effect, BakedBuffer, RenderCtx, RenderTarget};
use crate::utils::id::IdCounter;
use crate::utils::transaction::Transaction;
use crate::utils::{
    get_credentials_for_surface, send_scale_transform, update_tiled_state,
    with_toplevel_last_uncommitted_configure, with_toplevel_role, with_toplevel_role_and_current,
    ResizeEdge,
};

#[derive(Debug)]
pub struct Mapped {
    pub window: Window,

    id: MappedId,

    credentials: Option<Credentials>,

    pre_commit_hook: HookId,

    rules: ResolvedWindowRules,

    need_to_recompute_rules: bool,

    needs_configure: bool,

    needs_frame_callback: bool,

    offscreen_data: RefCell<Option<OffscreenData>>,

    is_urgent: bool,

    is_focused: bool,

    is_active_in_column: bool,

    is_floating: bool,

    is_window_cast_target: bool,

    ignore_opacity_window_rule: bool,

    block_out_buffer: RefCell<SolidColorBuffer>,

    blur_config: zen_config::Blur,

    animate_next_configure: bool,

    animate_serials: Vec<Serial>,

    animation_snapshot: Option<LayoutElementRenderSnapshot>,

    request_size_once: Option<RequestSizeOnce>,

    transaction_for_next_configure: Option<Transaction>,

    pending_transactions: Vec<(Serial, Transaction)>,

    interactive_resize: Option<InteractiveResize>,

    last_interactive_resize_start: Cell<Option<(Duration, ResizeEdge)>>,

    is_windowed_fullscreen: bool,

    is_pending_windowed_fullscreen: bool,

    uncommitted_windowed_fullscreen: Vec<(Serial, bool)>,

    is_maximized: bool,

    is_pending_maximized: bool,

    uncommitted_maximized: Vec<(Serial, bool)>,

    focus_timestamp: Option<Duration>,
}

zen_render_elements! {
    WindowCastRenderElements<R> => {
        Layout = LayoutElementRenderElement<R>,
        Border = BorderRenderElement,
    }
}

static MAPPED_ID_COUNTER: IdCounter = IdCounter::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MappedId(u64);

impl MappedId {
    pub fn next() -> MappedId {
        MappedId(MAPPED_ID_COUNTER.next())
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn to_protocol_identifier(self) -> String {
        format!("{}", self.0)
    }
}

#[derive(Debug)]
enum InteractiveResize {
    Ongoing(InteractiveResizeData),
    WaitingForLastConfigure(InteractiveResizeData),
    WaitingForLastCommit {
        data: InteractiveResizeData,
        serial: Serial,
    },
}

impl InteractiveResize {
    fn data(&self) -> InteractiveResizeData {
        match self {
            InteractiveResize::Ongoing(data) => *data,
            InteractiveResize::WaitingForLastConfigure(data) => *data,
            InteractiveResize::WaitingForLastCommit { data, .. } => *data,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RequestSizeOnce {
    WaitingForConfigure,
    WaitingForCommit(Serial),
    UseWindowSize,
}

impl Mapped {
    pub fn new(window: Window, rules: ResolvedWindowRules, hook: HookId, config: &Config) -> Self {
        let surface = window.wl_surface().expect("no X11 support");
        let credentials = get_credentials_for_surface(&surface);
        let mut rv = Self {
            window,
            id: MappedId::next(),
            credentials,
            pre_commit_hook: hook,
            rules,
            need_to_recompute_rules: false,
            needs_configure: false,
            needs_frame_callback: false,
            offscreen_data: RefCell::new(None),
            is_urgent: false,
            is_focused: false,
            is_active_in_column: true,
            is_floating: false,
            is_window_cast_target: false,
            ignore_opacity_window_rule: false,
            block_out_buffer: RefCell::new(SolidColorBuffer::new((0., 0.), [0., 0., 0., 1.])),
            blur_config: config.blur,
            animate_next_configure: false,
            animate_serials: Vec::new(),
            animation_snapshot: None,
            request_size_once: None,
            transaction_for_next_configure: None,
            pending_transactions: Vec::new(),
            interactive_resize: None,
            last_interactive_resize_start: Cell::new(None),
            is_windowed_fullscreen: false,
            is_pending_windowed_fullscreen: false,
            uncommitted_windowed_fullscreen: Vec::new(),
            is_maximized: false,
            is_pending_maximized: false,
            uncommitted_maximized: Vec::new(),
            focus_timestamp: None,
        };

        rv.is_maximized = rv.sizing_mode().is_maximized();
        rv.is_pending_maximized = rv.pending_sizing_mode().is_maximized();

        rv
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.window.toplevel().expect("no X11 support")
    }

    pub fn recompute_window_rules(&mut self, rules: &[WindowRule], is_at_startup: bool) -> bool {
        self.need_to_recompute_rules = false;

        let new_rules = ResolvedWindowRules::compute(rules, WindowRef::Mapped(self), is_at_startup);
        if new_rules == self.rules {
            return false;
        }

        if !new_rules.opacity.is_some_and(|o| o < 1.) {
            self.ignore_opacity_window_rule = false;
        }

        self.rules = new_rules;
        true
    }

    pub fn recompute_window_rules_if_needed(
        &mut self,
        rules: &[WindowRule],
        is_at_startup: bool,
    ) -> bool {
        if !self.need_to_recompute_rules {
            return false;
        }

        self.recompute_window_rules(rules, is_at_startup)
    }

    pub fn set_needs_configure(&mut self) {
        self.needs_configure = true;
    }

    pub fn id(&self) -> MappedId {
        self.id
    }

    pub fn credentials(&self) -> Option<&Credentials> {
        self.credentials.as_ref()
    }

    pub fn offscreen_data(&self) -> Ref<'_, Option<OffscreenData>> {
        self.offscreen_data.borrow()
    }

    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    pub fn is_active_in_column(&self) -> bool {
        self.is_active_in_column
    }

    pub fn is_floating(&self) -> bool {
        self.is_floating
    }

    pub fn is_window_cast_target(&self) -> bool {
        self.is_window_cast_target
    }

    pub fn toggle_ignore_opacity_window_rule(&mut self) {
        self.ignore_opacity_window_rule = !self.ignore_opacity_window_rule;
    }

    pub fn set_is_focused(&mut self, is_focused: bool) {
        if self.is_focused == is_focused {
            return;
        }

        self.is_focused = is_focused;
        self.is_urgent = false;
        self.need_to_recompute_rules = true;
    }

    pub fn set_is_window_cast_target(&mut self, value: bool) {
        if self.is_window_cast_target == value {
            return;
        }

        self.is_window_cast_target = value;
        self.need_to_recompute_rules = true;
    }

    fn render_snapshot(&self, renderer: &mut GlesRenderer) -> LayoutElementRenderSnapshot {
        let _span = tracy_client::span!("Mapped::render_snapshot");

        let size = self.size().to_f64();

        let mut buffer = self.block_out_buffer.borrow_mut();
        buffer.resize(size);
        let blocked_out_contents = vec![BakedBuffer {
            buffer: buffer.clone(),
            location: Point::from((0., 0.)),
            src: None,
            dst: None,
        }];

        let buf_pos = self.window.geometry().loc.upscale(-1).to_f64();

        let mut contents = vec![];

        let surface = self.toplevel().wl_surface();
        render_snapshot_from_surface_tree(renderer, surface, buf_pos, &mut contents);

        RenderSnapshot {
            contents,
            contents_with_blocked_out_bg: None,
            blocked_out_contents,
            block_out_from: self.rules().block_out_from,
            size,
            texture: Default::default(),
            texture_with_blocked_out_bg: Default::default(),
            blocked_out_texture: Default::default(),
        }
    }

    pub fn should_animate_commit(&mut self, commit_serial: Serial) -> bool {
        let mut should_animate = false;
        self.animate_serials.retain_mut(|serial| {
            if commit_serial.is_no_older_than(serial) {
                should_animate = true;
                false
            } else {
                true
            }
        });
        should_animate
    }

    pub fn store_animation_snapshot(&mut self, renderer: &mut GlesRenderer) {
        self.animation_snapshot = Some(self.render_snapshot(renderer));
    }

    pub fn take_pending_transaction(&mut self, commit_serial: Serial) -> Option<Transaction> {
        let mut rv = None;

        while let Some((serial, _)) = self.pending_transactions.first() {
            if commit_serial.is_no_older_than(serial) {
                let (_, transaction) = self.pending_transactions.remove(0);
                rv = Some(transaction);
            } else {
                break;
            }
        }

        rv
    }

    pub fn last_interactive_resize_start(&self) -> &Cell<Option<(Duration, ResizeEdge)>> {
        &self.last_interactive_resize_start
    }

    pub fn render_for_screen_cast<R: ZenRenderer>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
        push: &mut dyn FnMut(WindowCastRenderElements<R>),
    ) {
        let bbox = self.window.bbox_with_popups().to_physical_precise_up(scale);

        let has_border_shader = BorderRenderElement::has_shader(renderer);
        let radius = self.geometry_corner_radius();
        let window_size = self
            .size()
            .to_f64()
            .to_physical_precise_round(scale)
            .to_logical(scale);
        let radius = radius.fit_to(window_size.w as f32, window_size.h as f32);
        let location = self.window.geometry().loc.to_f64() - bbox.loc.to_logical(scale);

        let use_border = |elem| {
            if let LayoutElementRenderElement::SolidColor(elem) = &elem {
                if radius != CornerRadius::default() && has_border_shader {
                    let geo = elem.geo();
                    return BorderRenderElement::new(
                        geo.size,
                        Rectangle::from_size(geo.size),
                        GradientInterpolation::default(),
                        Color::from_color32f(elem.color()),
                        Color::from_color32f(elem.color()),
                        0.,
                        Rectangle::from_size(geo.size),
                        0.,
                        radius,
                        scale.x as f32,
                        1.,
                    )
                    .with_location(geo.loc)
                    .into();
                }
            }

            WindowCastRenderElements::from(elem)
        };

        self.render(
            RenderCtx {
                renderer,
                target: RenderTarget::Screencast,
                xray: None,
            },
            location,
            scale,
            1.,
            XrayPos::default(),
            &mut |elem| push(use_border(elem)),
        );
    }

    pub fn get_focus_timestamp(&self) -> Option<Duration> {
        self.focus_timestamp
    }

    pub fn set_focus_timestamp(&mut self, timestamp: Duration) {
        self.focus_timestamp.replace(timestamp);
    }

    pub fn send_frame<T, F>(
        &mut self,
        output: &Output,
        time: T,
        throttle: Option<Duration>,
        mut primary_scan_out_output: F,
    ) where
        T: Into<Duration>,
        F: FnMut(&WlSurface, &SurfaceData) -> Option<Output> + Copy,
    {
        let needs_frame_callback = self.needs_frame_callback;
        self.needs_frame_callback = false;

        let should_send = move |surface: &WlSurface, states: &SurfaceData| {
            if let Some(output) = primary_scan_out_output(surface, states) {
                return Some(output);
            }

            needs_frame_callback.then(|| output.clone())
        };
        self.window.send_frame(output, time, throttle, should_send);
    }

    pub fn update_tiled_state(&self, prefer_no_csd: bool) {
        update_tiled_state(self.toplevel(), prefer_no_csd, self.rules.tiled_state);
    }

    pub fn is_windowed_fullscreen(&self) -> bool {
        self.is_windowed_fullscreen
    }

    pub fn set_urgent(&mut self, urgent: bool) {
        if self.is_focused && urgent {
            return;
        }

        let changed = self.is_urgent != urgent;
        self.is_urgent = urgent;
        self.need_to_recompute_rules |= changed;
    }

    pub fn is_urgent(&self) -> bool {
        self.is_urgent
    }
}

impl Drop for Mapped {
    fn drop(&mut self) {
        remove_pre_commit_hook(self.toplevel().wl_surface(), &self.pre_commit_hook);
    }
}

impl LayoutElement for Mapped {
    type Id = Window;

    fn id(&self) -> &Self::Id {
        &self.window
    }

    fn update_config(&mut self, blur_config: zen_config::Blur) {
        self.blur_config = blur_config;
    }

    fn size(&self) -> Size<i32, Logical> {
        self.window.geometry().size
    }

    fn buf_loc(&self) -> Point<i32, Logical> {
        Point::from((0, 0)) - self.window.geometry().loc
    }

    fn is_in_input_region(&self, point: Point<f64, Logical>) -> bool {
        let surface_local = point + self.window.geometry().loc.to_f64();
        self.window.is_in_input_region(&surface_local)
    }

    fn render_normal<R: ZenRenderer>(
        &self,
        ctx: RenderCtx<R>,
        location: Point<f64, Logical>,
        scale: Scale<f64>,
        alpha: f32,
        push: &mut dyn FnMut(LayoutElementRenderElement<R>),
    ) {
        if ctx.target.should_block_out(self.rules.block_out_from) {
            let mut buffer = self.block_out_buffer.borrow_mut();
            buffer.resize(self.window.geometry().size.to_f64());
            let elem =
                SolidColorRenderElement::from_buffer(&buffer, location, alpha, Kind::Unspecified);
            push(elem.into());
        } else {
            let buf_pos = location - self.window.geometry().loc.to_f64();
            let surface = self.toplevel().wl_surface();
            let mut push = |elem: WaylandSurfaceRenderElement<R>| push(elem.into());
            push_elements_from_surface_tree(
                ctx.renderer,
                surface,
                buf_pos.to_physical_precise_round(scale),
                scale,
                alpha,
                Kind::ScanoutCandidate,
                &mut push,
            )
        }
    }

    fn render_popups<R: ZenRenderer>(
        &self,
        mut ctx: RenderCtx<R>,
        location: Point<f64, Logical>,
        scale: Scale<f64>,
        alpha: f32,
        xray_pos: XrayPos,
        push: &mut dyn FnMut(LayoutElementRenderElement<R>),
    ) {
        if ctx.target.should_block_out(self.rules.block_out_from) {
            return;
        }

        let surface = self.toplevel().wl_surface();
        for (popup, offset) in PopupManager::popups_for_surface(surface) {
            let popup_rules = match popup {
                PopupKind::Xdg(_) => self.rules.popups,
                PopupKind::InputMethod(_) => zen_config::ResolvedPopupsRules::default(),
            };
            let alpha = alpha * popup_rules.opacity.unwrap_or(1.).clamp(0., 1.);

            let surface = popup.wl_surface();
            let popup_geo = popup.geometry();
            let surface_loc = location + (offset - popup.geometry().loc).to_f64();

            push_elements_from_surface_tree(
                ctx.renderer,
                surface,
                surface_loc.to_physical_precise_round(scale),
                scale,
                alpha,
                Kind::ScanoutCandidate,
                &mut |elem| push(elem.into()),
            );

            let geometry = Rectangle::new(location + offset.to_f64(), popup_geo.size.to_f64());
            let surface_off = popup_geo.loc.upscale(-1).to_f64();
            let surface_anim_scale = Scale::from(1.);
            let mut effect = popup_rules.background_effect;
            if effect.xray.is_none() {
                effect.xray = Some(false);
            }
            let xray_pos = xray_pos.offset(offset.to_f64());
            background_effect::render_for_tile(
                ctx.as_gles(),
                None,
                geometry,
                scale.x,
                false,
                surface,
                surface_off,
                surface_anim_scale,
                self.blur_config,
                popup_rules.geometry_corner_radius.unwrap_or_default(),
                effect,
                false,
                xray_pos,
                &mut |elem| push(elem.into()),
            );
        }
    }

    fn render_background_effect(
        &self,
        ctx: RenderCtx<GlesRenderer>,
        geometry: Rectangle<f64, Logical>,
        scale: f64,
        clip_to_geometry: bool,
        surface_anim_scale: Scale<f64>,
        radius: CornerRadius,
        xray_pos: XrayPos,
        push: &mut dyn FnMut(BackgroundEffectElement),
    ) {
        let should_block_out = ctx.target.should_block_out(self.rules.block_out_from);
        background_effect::render_for_tile(
            ctx,
            None,
            geometry,
            scale,
            clip_to_geometry,
            self.toplevel().wl_surface(),
            self.buf_loc().to_f64(),
            surface_anim_scale,
            self.blur_config,
            radius,
            self.rules.background_effect,
            should_block_out,
            xray_pos,
            push,
        );
    }

    fn request_size(
        &mut self,
        size: Size<i32, Logical>,
        mode: SizingMode,
        animate: bool,
        transaction: Option<Transaction>,
    ) {
        if mode == SizingMode::Fullscreen {
            self.is_pending_windowed_fullscreen = false;

            if self.is_windowed_fullscreen {
                self.needs_configure = true;
            }
        }

        self.is_pending_maximized = mode == SizingMode::Maximized;
        if self.is_maximized != self.is_pending_maximized {
            self.needs_configure = true;
        }

        let changed = self.toplevel().with_pending_state(|state| {
            let changed = state.size != Some(size);
            state.size = Some(size);

            if mode.is_fullscreen() || self.is_pending_windowed_fullscreen {
                state.states.set(xdg_toplevel::State::Fullscreen);
                state.states.unset(xdg_toplevel::State::Maximized);
            } else if mode.is_maximized() {
                state.states.unset(xdg_toplevel::State::Fullscreen);
                state.states.set(xdg_toplevel::State::Maximized);
            } else {
                state.states.unset(xdg_toplevel::State::Fullscreen);
                state.states.unset(xdg_toplevel::State::Maximized);
            }

            changed
        });

        if changed && animate {
            self.animate_next_configure = true;
        }

        self.request_size_once = None;

        if let Some(transaction) = transaction {
            self.transaction_for_next_configure = Some(transaction);
        }
    }

    fn request_size_once(&mut self, size: Size<i32, Logical>, animate: bool) {
        self.transaction_for_next_configure = None;

        self.is_pending_maximized = false;
        if self.is_maximized != self.is_pending_maximized {
            self.needs_configure = true;
        }

        let already_sent = with_toplevel_last_uncommitted_configure(self.toplevel(), |configure| {
            let ToplevelConfigure { state, serial } = configure?;

            let same_size = state.size.unwrap_or_default() == size;
            let has_fullscreen = state.states.contains(xdg_toplevel::State::Fullscreen);
            let same_fullscreen = has_fullscreen == self.is_pending_windowed_fullscreen;
            let has_maximized = state.states.contains(xdg_toplevel::State::Maximized);
            let same_maximized = !has_maximized;
            (same_size && same_fullscreen && same_maximized).then_some(*serial)
        });

        if let Some(serial) = already_sent {
            let current_serial = with_states(self.toplevel().wl_surface(), |states| {
                states
                    .cached_state
                    .get::<ToplevelCachedState>()
                    .current()
                    .last_acked
                    .as_ref()
                    .map(|c| c.serial)
            });
            if let Some(current_serial) = current_serial {
                if !current_serial.is_no_older_than(&serial) {
                    self.request_size_once = Some(RequestSizeOnce::WaitingForCommit(serial));
                } else {
                    self.request_size_once = Some(RequestSizeOnce::UseWindowSize);
                }
            } else {
                warn!("no current serial; did the surface not ack the initial configure?");
                self.request_size_once = Some(RequestSizeOnce::UseWindowSize);
            };
            return;
        }

        let changed = self.toplevel().with_pending_state(|state| {
            let changed = state.size != Some(size);
            state.size = Some(size);
            if !self.is_pending_windowed_fullscreen {
                state.states.unset(xdg_toplevel::State::Fullscreen);
            }
            state.states.unset(xdg_toplevel::State::Maximized);
            changed
        });

        if changed && animate {
            self.animate_next_configure = true;
        }

        self.request_size_once = Some(RequestSizeOnce::WaitingForConfigure);
    }

    fn min_size(&self) -> Size<i32, Logical> {
        let min_size = with_states(self.toplevel().wl_surface(), |state| {
            let mut guard = state.cached_state.get::<SurfaceCachedState>();
            guard.current().min_size
        });

        self.rules.apply_min_size(min_size)
    }

    fn max_size(&self) -> Size<i32, Logical> {
        let max_size = with_states(self.toplevel().wl_surface(), |state| {
            let mut guard = state.cached_state.get::<SurfaceCachedState>();
            guard.current().max_size
        });

        self.rules.apply_max_size(max_size)
    }

    fn is_wl_surface(&self, wl_surface: &WlSurface) -> bool {
        self.toplevel().wl_surface() == wl_surface
    }

    fn set_preferred_scale_transform(&self, scale: output::Scale, transform: Transform) {
        self.window.with_surfaces(|surface, data| {
            send_scale_transform(surface, data, scale, transform);
        });
    }

    fn has_ssd(&self) -> bool {
        let toplevel = self.toplevel();
        let mode = self
            .toplevel()
            .with_committed_state(|current| current.and_then(|s| s.decoration_mode));

        match mode {
            Some(zxdg_toplevel_decoration_v1::Mode::ServerSide) => true,
            None => with_states(toplevel.wl_surface(), |states| {
                states
                    .data_map
                    .get::<KdeDecorationsModeState>()
                    .map(KdeDecorationsModeState::is_server)
                    == Some(true)
            }),
            _ => false,
        }
    }

    fn output_enter(&self, output: &Output) {
        let overlap = Rectangle::from_size(Size::from((i32::MAX, i32::MAX)));
        self.window.output_enter(output, overlap)
    }

    fn output_leave(&self, output: &Output) {
        self.window.output_leave(output)
    }

    fn set_offscreen_data(&self, data: Option<OffscreenData>) {
        let Some(data) = data else {
            self.offscreen_data.replace(None);
            return;
        };

        let mut offscreen_data = self.offscreen_data.borrow_mut();
        match &mut *offscreen_data {
            None => {
                *offscreen_data = Some(data);
            }
            Some(existing) => {
                existing.id = data.id;
                existing.states.states.extend(data.states.states);
            }
        }
    }

    fn is_urgent(&self) -> bool {
        self.is_urgent
    }

    fn set_activated(&mut self, active: bool) {
        let changed = self.toplevel().with_pending_state(|state| {
            if active {
                state.states.set(xdg_toplevel::State::Activated)
            } else {
                state.states.unset(xdg_toplevel::State::Activated)
            }
        });
        self.need_to_recompute_rules |= changed;
    }

    fn set_active_in_column(&mut self, active: bool) {
        let changed = self.is_active_in_column != active;
        self.is_active_in_column = active;
        self.need_to_recompute_rules |= changed;
    }

    fn set_floating(&mut self, floating: bool) {
        let changed = self.is_floating != floating;
        self.is_floating = floating;
        self.need_to_recompute_rules |= changed;
    }

    fn set_bounds(&self, bounds: Size<i32, Logical>) {
        self.toplevel().with_pending_state(|state| {
            state.bounds = Some(bounds);
        });
    }

    fn configure_intent(&self) -> ConfigureIntent {
        let _span =
            trace_span!("configure_intent", surface = ?self.toplevel().wl_surface().id()).entered();

        if self.needs_configure {
            trace!("the window needs_configure");
            return ConfigureIntent::ShouldSend;
        }

        with_toplevel_role_and_current(self.toplevel(), |attributes, current_committed| {
            if let Some(server_pending) = &attributes.server_pending {
                let current_server = attributes.current_server_state();
                if *server_pending != current_server {
                    let mut current_server_same_size = current_server.clone();
                    current_server_same_size.size = server_pending.size;
                    if current_server_same_size == *server_pending {
                        let Some(current_committed) = current_committed else {
                            error!("mapped must have had initial commit");
                            return ConfigureIntent::ShouldSend;
                        };

                        if current_committed.size == current_server.size {
                            trace!(
                                "current size matches server size: {:?}",
                                current_committed.size
                            );
                            ConfigureIntent::CanSend
                        } else {
                            trace!("throttling resize");
                            ConfigureIntent::Throttled
                        }
                    } else {
                        trace!("something changed other than the size");
                        ConfigureIntent::ShouldSend
                    }
                } else {
                    ConfigureIntent::NotNeeded
                }
            } else {
                ConfigureIntent::NotNeeded
            }
        })
    }

    fn send_pending_configure(&mut self) {
        let toplevel = self.toplevel();
        let _span =
            trace_span!("send_pending_configure", surface = ?toplevel.wl_surface().id()).entered();

        let has_pending_changes = self.needs_configure
            || with_toplevel_role(self.toplevel(), |role| {
                if role.server_pending.is_none() {
                    return false;
                }

                let current_server_size = role.current_server_state().size;
                let server_pending = role.server_pending.as_mut().unwrap();

                if let Some(RequestSizeOnce::UseWindowSize) = self.request_size_once {
                    server_pending.size = current_server_size;
                }

                let server_pending = role.server_pending.as_ref().unwrap();
                *server_pending != role.current_server_state()
            });

        if has_pending_changes {
            if let Some(RequestSizeOnce::UseWindowSize) = self.request_size_once {
                let size = self.window.geometry().size;
                toplevel.with_pending_state(|state| {
                    state.size = Some(size);
                });
            }

            let serial = toplevel.send_configure();
            trace!(?serial, "sending configure");

            self.needs_configure = false;

            self.needs_frame_callback = true;

            if self.animate_next_configure {
                self.animate_serials.push(serial);
            }

            if let Some(transaction) = self.transaction_for_next_configure.take() {
                self.pending_transactions.push((serial, transaction));
            }

            self.interactive_resize = match self.interactive_resize.take() {
                Some(InteractiveResize::WaitingForLastConfigure(data)) => {
                    Some(InteractiveResize::WaitingForLastCommit { data, serial })
                }
                x => x,
            };

            if let Some(RequestSizeOnce::WaitingForConfigure) = self.request_size_once {
                self.request_size_once = Some(RequestSizeOnce::WaitingForCommit(serial));
            }

            let last_sent_windowed_fullscreen = self
                .uncommitted_windowed_fullscreen
                .last()
                .map(|(_, value)| *value)
                .unwrap_or(self.is_windowed_fullscreen);
            if last_sent_windowed_fullscreen != self.is_pending_windowed_fullscreen {
                self.uncommitted_windowed_fullscreen
                    .push((serial, self.is_pending_windowed_fullscreen));
            }

            let last_sent_maximized = self
                .uncommitted_maximized
                .last()
                .map(|(_, value)| *value)
                .unwrap_or(self.is_maximized);
            if last_sent_maximized != self.is_pending_maximized {
                self.uncommitted_maximized
                    .push((serial, self.is_pending_maximized));
            }
        } else {
            self.interactive_resize = match self.interactive_resize.take() {
                Some(InteractiveResize::WaitingForLastConfigure { .. }) => None,
                x => x,
            };
        }

        self.animate_next_configure = false;
        self.transaction_for_next_configure = None;
    }

    fn sizing_mode(&self) -> SizingMode {
        if self.is_windowed_fullscreen {
            return if self.is_maximized {
                SizingMode::Maximized
            } else {
                SizingMode::Normal
            };
        }

        self.toplevel().with_committed_state(|state| {
            let Some(state) = state else {
                return SizingMode::Normal;
            };

            if state.states.contains(xdg_toplevel::State::Fullscreen) {
                SizingMode::Fullscreen
            } else if state.states.contains(xdg_toplevel::State::Maximized) {
                SizingMode::Maximized
            } else {
                SizingMode::Normal
            }
        })
    }

    fn pending_sizing_mode(&self) -> SizingMode {
        if self.is_pending_windowed_fullscreen {
            return if self.is_pending_maximized {
                SizingMode::Maximized
            } else {
                SizingMode::Normal
            };
        }

        self.toplevel().with_pending_state(|state| {
            if state.states.contains(xdg_toplevel::State::Fullscreen) {
                SizingMode::Fullscreen
            } else if state.states.contains(xdg_toplevel::State::Maximized) {
                SizingMode::Maximized
            } else {
                SizingMode::Normal
            }
        })
    }

    fn is_ignoring_opacity_window_rule(&self) -> bool {
        self.ignore_opacity_window_rule
    }

    fn requested_size(&self) -> Option<Size<i32, Logical>> {
        self.toplevel().with_pending_state(|state| state.size)
    }

    fn expected_size(&self) -> Option<Size<i32, Logical>> {
        let current_size = (self.sizing_mode().is_normal()).then(|| self.window.geometry().size);

        if let Some(RequestSizeOnce::UseWindowSize) = self.request_size_once {
            return current_size;
        }

        let pending = with_states(self.toplevel().wl_surface(), |states| {
            let role = states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap();

            let server_pending = role.server_pending.as_ref()?;

            let current_server = role.current_server_state();
            if server_pending.size != current_server.size {
                return Some((
                    server_pending.size.unwrap_or_default(),
                    server_pending
                        .states
                        .contains(xdg_toplevel::State::Fullscreen),
                    server_pending
                        .states
                        .contains(xdg_toplevel::State::Maximized),
                ));
            }

            None
        })
        .or_else(|| {
            with_toplevel_last_uncommitted_configure(self.toplevel(), |configure| {
                let ToplevelConfigure { state, .. } = configure?;

                Some((
                    state.size.unwrap_or_default(),
                    state.states.contains(xdg_toplevel::State::Fullscreen),
                    state.states.contains(xdg_toplevel::State::Maximized),
                ))
            })
        });

        if let Some((mut size, fullscreen, maximized)) = pending {
            if maximized
                || (fullscreen
                    && (!self.is_pending_windowed_fullscreen || self.is_pending_maximized))
            {
                return None;
            }

            if size.w == 0 {
                size.w = current_size?.w;
            }
            if size.h == 0 {
                size.h = current_size?.h;
            }

            Some(size)
        } else {
            current_size
        }
    }

    fn is_windowed_fullscreen(&self) -> bool {
        self.is_windowed_fullscreen
    }

    fn is_pending_windowed_fullscreen(&self) -> bool {
        self.is_pending_windowed_fullscreen
    }

    fn request_windowed_fullscreen(&mut self, value: bool) {
        if self.is_pending_windowed_fullscreen == value {
            return;
        }

        self.is_pending_windowed_fullscreen = value;

        self.toplevel().with_pending_state(|state| {
            if value {
                state.states.set(xdg_toplevel::State::Fullscreen);
                state.states.unset(xdg_toplevel::State::Maximized);
            } else {
                state.states.unset(xdg_toplevel::State::Fullscreen);

                if self.is_pending_maximized {
                    state.states.set(xdg_toplevel::State::Maximized);
                }
            }
        });

        self.needs_configure = true;
    }

    fn is_child_of(&self, parent: &Self) -> bool {
        self.toplevel().parent().as_ref() == Some(parent.toplevel().wl_surface())
    }

    fn refresh(&self) {
        self.window.refresh();
    }

    fn rules(&self) -> &ResolvedWindowRules {
        &self.rules
    }

    fn take_animation_snapshot(&mut self) -> Option<LayoutElementRenderSnapshot> {
        self.animation_snapshot.take()
    }

    fn set_interactive_resize(&mut self, data: Option<InteractiveResizeData>) {
        self.toplevel().with_pending_state(|state| {
            if data.is_some() {
                state.states.set(xdg_toplevel::State::Resizing);
            } else {
                state.states.unset(xdg_toplevel::State::Resizing);
            }
        });

        if let Some(data) = data {
            self.interactive_resize = Some(InteractiveResize::Ongoing(data));
        } else {
            self.interactive_resize = match self.interactive_resize.take() {
                Some(InteractiveResize::Ongoing(data)) => {
                    Some(InteractiveResize::WaitingForLastConfigure(data))
                }
                x => x,
            }
        }
    }

    fn cancel_interactive_resize(&mut self) {
        self.set_interactive_resize(None);
        self.interactive_resize = None;
    }

    fn interactive_resize_data(&self) -> Option<InteractiveResizeData> {
        Some(self.interactive_resize.as_ref()?.data())
    }

    fn on_commit(&mut self, commit_serial: Serial) {
        if let Some(InteractiveResize::WaitingForLastCommit { serial, .. }) =
            &self.interactive_resize
        {
            if commit_serial.is_no_older_than(serial) {
                self.interactive_resize = None;
            }
        }

        if let Some(RequestSizeOnce::WaitingForCommit(serial)) = &self.request_size_once {
            if commit_serial.is_no_older_than(serial) {
                self.request_size_once = Some(RequestSizeOnce::UseWindowSize);
            }
        }

        self.uncommitted_windowed_fullscreen
            .retain_mut(|(serial, value)| {
                if commit_serial.is_no_older_than(serial) {
                    self.is_windowed_fullscreen = *value;
                    false
                } else {
                    true
                }
            });

        self.uncommitted_maximized.retain_mut(|(serial, value)| {
            if commit_serial.is_no_older_than(serial) {
                self.is_maximized = *value;
                false
            } else {
                true
            }
        });
    }
}
