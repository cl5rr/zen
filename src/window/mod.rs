use std::cmp::{max, min};

use zen_config::utils::MergeWith as _;
use zen_config::window_rule::{Match, OnXdgActivate, WindowRule};
use zen_config::{
    BackgroundEffect, BlockOutFrom, BorderRule, CornerRadius, FloatingPosition, PresetSize,
    ResolvedPopupsRules, ShadowRule, TabIndicatorRule,
};
use zen_ipc::ColumnDisplay;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{
    SurfaceCachedState, ToplevelSurface, XdgToplevelSurfaceRoleAttributes,
};

use crate::utils::with_toplevel_role;

pub mod mapped;
pub use mapped::Mapped;

pub mod unmapped;
pub use unmapped::{InitialConfigureState, Unmapped};

#[derive(Debug, Clone, Copy)]
pub enum WindowRef<'a> {
    Unmapped(&'a Unmapped),
    Mapped(&'a Mapped),
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResolvedWindowRules {
    pub default_width: Option<Option<PresetSize>>,

    pub default_height: Option<Option<PresetSize>>,

    pub default_column_display: Option<ColumnDisplay>,

    pub default_floating_position: Option<FloatingPosition>,

    pub open_on_output: Option<String>,

    pub open_on_workspace: Option<String>,

    pub open_maximized: Option<bool>,

    pub open_maximized_to_edges: Option<bool>,

    pub open_fullscreen: Option<bool>,

    pub open_floating: Option<bool>,

    pub open_focused: Option<bool>,

    pub on_xdg_activate: Option<OnXdgActivate>,

    pub min_width: Option<u16>,
    pub min_height: Option<u16>,
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,

    pub focus_ring: BorderRule,
    pub border: BorderRule,
    pub shadow: ShadowRule,
    pub tab_indicator: TabIndicatorRule,

    pub draw_border_with_background: Option<bool>,

    pub opacity: Option<f32>,

    pub geometry_corner_radius: Option<CornerRadius>,

    pub clip_to_geometry: Option<bool>,

    pub baba_is_float: Option<bool>,

    pub block_out_from: Option<BlockOutFrom>,

    pub variable_refresh_rate: Option<bool>,

    pub scroll_factor: Option<f64>,

    pub tiled_state: Option<bool>,

    pub background_effect: BackgroundEffect,

    pub popups: ResolvedPopupsRules,
}

impl<'a> WindowRef<'a> {
    pub fn toplevel(self) -> &'a ToplevelSurface {
        match self {
            WindowRef::Unmapped(unmapped) => unmapped.toplevel(),
            WindowRef::Mapped(mapped) => mapped.toplevel(),
        }
    }

    pub fn is_focused(self) -> bool {
        match self {
            WindowRef::Unmapped(_) => false,
            WindowRef::Mapped(mapped) => mapped.is_focused(),
        }
    }

    pub fn is_urgent(self) -> bool {
        match self {
            WindowRef::Unmapped(_) => false,
            WindowRef::Mapped(mapped) => mapped.is_urgent(),
        }
    }

    pub fn is_active_in_column(self) -> bool {
        match self {
            WindowRef::Unmapped(_) => true,
            WindowRef::Mapped(mapped) => mapped.is_active_in_column(),
        }
    }

    pub fn is_floating(self) -> bool {
        match self {
            WindowRef::Unmapped(_) => false,
            WindowRef::Mapped(mapped) => mapped.is_floating(),
        }
    }

    pub fn is_window_cast_target(self) -> bool {
        match self {
            WindowRef::Unmapped(_) => false,
            WindowRef::Mapped(mapped) => mapped.is_window_cast_target(),
        }
    }
}

impl ResolvedWindowRules {
    pub fn compute(rules: &[WindowRule], window: WindowRef, is_at_startup: bool) -> Self {
        let _span = tracy_client::span!("ResolvedWindowRules::compute");

        let mut resolved = ResolvedWindowRules::default();

        with_toplevel_role(window.toplevel(), |role| {
            if role.server_pending.is_none() {
                role.server_pending = Some(role.current_server_state().clone());
            }

            let mut open_on_output = None;
            let mut open_on_workspace = None;

            for rule in rules {
                let matches = |m: &Match| {
                    if let Some(at_startup) = m.at_startup {
                        if at_startup != is_at_startup {
                            return false;
                        }
                    }

                    window_matches(window, role, m)
                };

                if !(rule.matches.is_empty() || rule.matches.iter().any(matches)) {
                    continue;
                }

                if rule.excludes.iter().any(matches) {
                    continue;
                }

                if let Some(x) = rule.default_column_width {
                    resolved.default_width = Some(x.0);
                }

                if let Some(x) = rule.default_window_height {
                    resolved.default_height = Some(x.0);
                }

                if let Some(x) = rule.default_column_display {
                    resolved.default_column_display = Some(x);
                }

                if let Some(x) = rule.default_floating_position {
                    resolved.default_floating_position = Some(x);
                }

                if let Some(x) = rule.open_on_output.as_deref() {
                    open_on_output = Some(x);
                }

                if let Some(x) = rule.open_on_workspace.as_deref() {
                    open_on_workspace = Some(x);
                }

                if let Some(x) = rule.open_maximized {
                    resolved.open_maximized = Some(x);
                }

                if let Some(x) = rule.open_maximized_to_edges {
                    resolved.open_maximized_to_edges = Some(x);
                }

                if let Some(x) = rule.open_fullscreen {
                    resolved.open_fullscreen = Some(x);
                }

                if let Some(x) = rule.open_floating {
                    resolved.open_floating = Some(x);
                }

                if let Some(x) = rule.open_focused {
                    resolved.open_focused = Some(x);
                }

                if let Some(x) = rule.on_xdg_activate {
                    resolved.on_xdg_activate = Some(x);
                }

                if let Some(x) = rule.min_width {
                    resolved.min_width = Some(x);
                }
                if let Some(x) = rule.min_height {
                    resolved.min_height = Some(x);
                }
                if let Some(x) = rule.max_width {
                    resolved.max_width = Some(x);
                }
                if let Some(x) = rule.max_height {
                    resolved.max_height = Some(x);
                }

                resolved.focus_ring.merge_with(&rule.focus_ring);
                resolved.border.merge_with(&rule.border);
                resolved.shadow.merge_with(&rule.shadow);
                resolved.tab_indicator.merge_with(&rule.tab_indicator);

                if let Some(x) = rule.draw_border_with_background {
                    resolved.draw_border_with_background = Some(x);
                }
                if let Some(x) = rule.opacity {
                    resolved.opacity = Some(x);
                }
                if let Some(x) = rule.geometry_corner_radius {
                    resolved.geometry_corner_radius = Some(x);
                }
                if let Some(x) = rule.clip_to_geometry {
                    resolved.clip_to_geometry = Some(x);
                }
                if let Some(x) = rule.baba_is_float {
                    resolved.baba_is_float = Some(x);
                }
                if let Some(x) = rule.block_out_from {
                    resolved.block_out_from = Some(x);
                }
                if let Some(x) = rule.variable_refresh_rate {
                    resolved.variable_refresh_rate = Some(x);
                }
                if let Some(x) = rule.scroll_factor {
                    resolved.scroll_factor = Some(x.0);
                }
                if let Some(x) = rule.tiled_state {
                    resolved.tiled_state = Some(x);
                }

                resolved
                    .background_effect
                    .merge_with(&rule.background_effect);

                resolved.popups.merge_with(&rule.popups);
            }

            resolved.open_on_output = open_on_output.map(|x| x.to_owned());
            resolved.open_on_workspace = open_on_workspace.map(|x| x.to_owned());
        });

        resolved
    }

    pub fn apply_min_size(&self, min_size: Size<i32, Logical>) -> Size<i32, Logical> {
        let mut size = min_size;

        if let Some(x) = self.min_width {
            size.w = max(size.w, i32::from(x));
        }
        if let Some(x) = self.min_height {
            size.h = max(size.h, i32::from(x));
        }

        size
    }

    pub fn apply_max_size(&self, max_size: Size<i32, Logical>) -> Size<i32, Logical> {
        let mut size = max_size;

        if let Some(x) = self.max_width {
            if size.w == 0 {
                size.w = i32::from(x);
            } else if x > 0 {
                size.w = min(size.w, i32::from(x));
            }
        }
        if let Some(x) = self.max_height {
            if size.h == 0 {
                size.h = i32::from(x);
            } else if x > 0 {
                size.h = min(size.h, i32::from(x));
            }
        }

        size
    }

    pub fn apply_min_max_size(
        &self,
        min_size: Size<i32, Logical>,
        max_size: Size<i32, Logical>,
    ) -> (Size<i32, Logical>, Size<i32, Logical>) {
        let min_size = self.apply_min_size(min_size);
        let max_size = self.apply_max_size(max_size);
        (min_size, max_size)
    }

    pub fn compute_open_floating(&self, toplevel: &ToplevelSurface, default_on_canvas: bool) -> bool {
        if let Some(res) = self.open_floating {
            return res;
        }

        if toplevel.parent().is_some() {
            return true;
        }

        if default_on_canvas {
            return true;
        }

        let (min_size, max_size) = with_states(toplevel.wl_surface(), |state| {
            let mut guard = state.cached_state.get::<SurfaceCachedState>();
            let current = guard.current();
            (current.min_size, current.max_size)
        });
        let (min_size, max_size) = self.apply_min_max_size(min_size, max_size);

        min_size.h > 0 && min_size.h == max_size.h
    }
}

fn window_matches(window: WindowRef, role: &XdgToplevelSurfaceRoleAttributes, m: &Match) -> bool {
    let server_pending = role.server_pending.as_ref().unwrap();

    if let Some(is_focused) = m.is_focused {
        if window.is_focused() != is_focused {
            return false;
        }
    }

    if let Some(is_urgent) = m.is_urgent {
        if window.is_urgent() != is_urgent {
            return false;
        }
    }

    if let Some(is_active) = m.is_active {
        let pending_activated = server_pending
            .states
            .contains(xdg_toplevel::State::Activated);
        if is_active != pending_activated {
            return false;
        }
    }

    if let Some(app_id_re) = &m.app_id {
        let Some(app_id) = &role.app_id else {
            return false;
        };
        if !app_id_re.0.is_match(app_id) {
            return false;
        }
    }

    if let Some(title_re) = &m.title {
        let Some(title) = &role.title else {
            return false;
        };
        if !title_re.0.is_match(title) {
            return false;
        }
    }

    if let Some(is_active_in_column) = m.is_active_in_column {
        if window.is_active_in_column() != is_active_in_column {
            return false;
        }
    }

    if let Some(is_floating) = m.is_floating {
        if window.is_floating() != is_floating {
            return false;
        }
    }

    if let Some(is_window_cast_target) = m.is_window_cast_target {
        if window.is_window_cast_target() != is_window_cast_target {
            return false;
        }
    }

    true
}
