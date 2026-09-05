use std::cell::Cell;

use calloop::Interest;
use zen_config::PresetSize;
use smithay::desktop::{
    find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output, utils, LayerSurface,
    PopupKeyboardGrab, PopupKind, PopupManager, PopupPointerGrab, PopupUngrabStrategy, Window,
    WindowSurfaceType,
};
use smithay::input::pointer::Focus;
use smithay::input::tablet::TabletSeatTrait;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_positioner::ConstraintAdjustment;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::{self};
use smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration;
use smithay::reexports::wayland_server::protocol::wl_output;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{self, Resource, WEnum};
use smithay::utils::{Logical, Rectangle, Serial};
use smithay::wayland::compositor::{
    add_blocker, add_pre_commit_hook, with_states, BufferAssignment, CompositorHandler as _,
    HookId, SurfaceAttributes,
};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::input_method::InputMethodSeat;
use smithay::wayland::shell::kde::decoration::{KdeDecorationHandler, KdeDecorationState};
use smithay::wayland::shell::wlr_layer::{self, Layer};
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::xdg_foreign::{XdgForeignHandler, XdgForeignState};
use tracing::field::Empty;

use crate::input::move_grab::MoveGrab;
use crate::input::resize_grab::ResizeGrab;
use crate::input::{AnyStartData, DOUBLE_CLICK_TIME};
use crate::layout::ActivateWindow;
use crate::state::{CastTarget, PopupGrabState, State};
use crate::utils::transaction::Transaction;
use crate::utils::{
    get_monotonic_time, output_matches_name, send_scale_transform, update_tiled_state, ResizeEdge,
};
use crate::window::{InitialConfigureState, ResolvedWindowRules, Unmapped, WindowRef};

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.zen.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface().clone();
        let unmapped = Unmapped::new(Window::new_wayland_window(surface));
        let existing = self.zen.unmapped_windows.insert(wl_surface, unmapped);
        assert!(existing.is_none());
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let popup = PopupKind::Xdg(surface);
        self.unconstrain_popup(&popup);

        if let Err(err) = self.zen.popups.track_popup(popup) {
            warn!("error tracking popup: {err:?}");
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, _seat: WlSeat, serial: Serial) {
        let wl_surface = surface.wl_surface();

        let mut grab_start_data = None;

        let pointer = self.zen.seat.get_pointer().unwrap();
        pointer.with_grab(|grab_serial, grab| {
            if grab_serial == serial {
                let start_data = grab.start_data();
                if let Some((focus, _)) = &start_data.focus {
                    if focus.id().same_client_as(&wl_surface.id()) {
                        let is_dnd_grab = Self::is_dnd_grab(grab.as_any());

                        if !is_dnd_grab {
                            grab_start_data = Some(AnyStartData::Pointer(start_data.clone()));
                        }
                    }
                }
            }
        });

        if let Some(touch) = self.zen.seat.get_touch() {
            touch.with_grab(|grab_serial, grab| {
                if grab_serial == serial {
                    let start_data = grab.start_data();
                    if let Some((focus, _)) = &start_data.focus {
                        if focus.id().same_client_as(&wl_surface.id()) {
                            let is_dnd_grab = Self::is_dnd_grab(grab.as_any());

                            if !is_dnd_grab {
                                grab_start_data = Some(AnyStartData::Touch(start_data.clone()));
                            }
                        }
                    }
                }
            });
        }

        let mut tablet_tool = None;
        self.zen.seat.tablet_seat().with_tools(|tools| {
            for tool in tools.values() {
                let found = tool.with_grab(|grab_serial, grab| {
                    if grab_serial == serial {
                        let start_data = grab.start_data();
                        if let Some((focus, _)) = &start_data.focus {
                            if focus.id().same_client_as(&wl_surface.id()) {
                                let is_dnd_grab = Self::is_dnd_grab(grab.as_any());

                                if !is_dnd_grab {
                                    grab_start_data =
                                        Some(AnyStartData::TabletTool(start_data.clone()));
                                    tablet_tool = Some(tool.clone());
                                    return true;
                                }
                            }
                        }
                    }
                    false
                });
                if found == Some(true) {
                    break;
                }
            }
        });

        let Some(start_data) = grab_start_data else {
            return;
        };

        let Some((mapped, output)) = self.zen.layout.find_window_and_output(wl_surface) else {
            return;
        };

        let Some(output) = output else {
            return;
        };

        let window = mapped.window.clone();
        let output = output.clone();

        match &start_data {
            AnyStartData::Pointer(_) => {
                if let Some(grab) = MoveGrab::new(self, start_data, window.clone(), true, None) {
                    pointer.set_grab(self, grab, serial, Focus::Clear);
                }
            }
            AnyStartData::Touch(_) => {
                let touch = self.zen.seat.get_touch().unwrap();
                if let Some(grab) = MoveGrab::new(self, start_data, window.clone(), true, None) {
                    touch.set_grab(self, grab, serial);
                }
            }
            AnyStartData::TabletTool(_) => {
                if let Some(grab) = MoveGrab::new(self, start_data, window.clone(), true, None) {
                    let time = get_monotonic_time().as_millis() as u32;
                    tablet_tool
                        .unwrap()
                        .set_grab(self, grab, time, serial, Focus::Clear);
                }
            }
        }

        self.zen.queue_redraw(&output);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        _seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let wl_surface = surface.wl_surface();

        let mut grab_start_data = None;

        let pointer = self.zen.seat.get_pointer().unwrap();
        if pointer.has_grab(serial) {
            if let Some(start_data) = pointer.grab_start_data() {
                if let Some((focus, _)) = &start_data.focus {
                    if focus.id().same_client_as(&wl_surface.id()) {
                        grab_start_data = Some(AnyStartData::Pointer(start_data));
                    }
                }
            }
        }

        if let Some(touch) = self.zen.seat.get_touch() {
            if touch.has_grab(serial) {
                if let Some(start_data) = touch.grab_start_data() {
                    if let Some((focus, _)) = &start_data.focus {
                        if focus.id().same_client_as(&wl_surface.id()) {
                            grab_start_data = Some(AnyStartData::Touch(start_data));
                        }
                    }
                }
            }
        }

        let mut tablet_tool = None;
        self.zen.seat.tablet_seat().with_tools(|tools| {
            'outer: for tool in tools.values() {
                if tool.has_grab(serial) {
                    if let Some(start_data) = tool.grab_start_data() {
                        if let Some((focus, _)) = &start_data.focus {
                            if focus.id().same_client_as(&wl_surface.id()) {
                                grab_start_data = Some(AnyStartData::TabletTool(start_data));
                                tablet_tool = Some(tool.clone());
                                break 'outer;
                            }
                        }
                    }
                }
            }
        });

        let Some(start_data) = grab_start_data else {
            return;
        };

        let Some((mapped, _)) = self.zen.layout.find_window_and_output(wl_surface) else {
            return;
        };

        let edges = ResizeEdge::from(edges);
        let window = mapped.window.clone();

        let time = get_monotonic_time();
        let last_cell = mapped.last_interactive_resize_start();
        let mut last = last_cell.get();
        last_cell.set(Some((time, edges)));

        if mapped.is_floating() {
            last = None;
            last_cell.set(None);
        }

        if let Some((last_time, last_edges)) = last {
            if time.saturating_sub(last_time) <= DOUBLE_CLICK_TIME {
                last_cell.set(None);

                let intersection = edges.intersection(last_edges);
                if intersection.intersects(ResizeEdge::LEFT_RIGHT) {
                    self.zen.layout.activate_window(&window);
                    self.zen.layer_shell_on_demand_focus = None;
                    self.zen.layout.toggle_full_width();
                }
                if intersection.intersects(ResizeEdge::TOP_BOTTOM) {
                    self.zen.layer_shell_on_demand_focus = None;
                    self.zen.layout.reset_window_height(Some(&window));
                }
                self.zen.queue_redraw_all();
                return;
            }
        }

        if !self
            .zen
            .layout
            .interactive_resize_begin(window.clone(), edges)
        {
            return;
        }

        match start_data {
            AnyStartData::Pointer(_) => {
                let grab = ResizeGrab::new(start_data, window);
                pointer.set_grab(self, grab, serial, Focus::Clear);
            }
            AnyStartData::Touch(_) => {
                let touch = self.zen.seat.get_touch().unwrap();
                let grab = ResizeGrab::new(start_data, window);
                touch.set_grab(self, grab, serial);
            }
            AnyStartData::TabletTool(_) => {
                let grab = ResizeGrab::new(start_data, window);
                let time = get_monotonic_time().as_millis() as u32;
                tablet_tool
                    .unwrap()
                    .set_grab(self, grab, time, serial, Focus::Clear);
            }
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&PopupKind::Xdg(surface.clone()));
        surface.send_repositioned(token);
    }

    fn grab(&mut self, surface: PopupSurface, _seat: WlSeat, serial: Serial) {
        let popup = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&popup) else {
            trace!("ignoring popup grab because no root surface");
            return;
        };

        if self.zen.exit_confirm_dialog.is_open() {
            trace!("ignoring popup grab because the exit confirm dialog is open");
            let _ = PopupManager::dismiss_popup(&root, &popup);
            return;
        } else if self.zen.is_locked() {
            if Some(&root) != self.zen.lock_surface_focus().as_ref() {
                trace!("ignoring popup grab because the session is locked");
                let _ = PopupManager::dismiss_popup(&root, &popup);
                return;
            }
        } else if self.zen.screenshot_ui.is_open() {
            trace!("ignoring popup grab because the screenshot UI is open");
            let _ = PopupManager::dismiss_popup(&root, &popup);
            return;
        } else if let Some(output) = self.zen.layout.active_output() {
            let layers = layer_map_for_output(output);

            if let Some(layer) = layers.layer_for_surface(&root, WindowSurfaceType::TOPLEVEL) {
                if let Some(mapped) = self.zen.mapped_layer_surfaces.get(layer) {
                    if mapped.place_within_backdrop() {
                        trace!("ignoring popup grab for a layer surface within overview backdrop");
                        let _ = PopupManager::dismiss_popup(&root, &popup);
                        return;
                    }
                }
            } else {
                if layers.layers_on(Layer::Overlay).any(|l| {
                    (l.cached_state().keyboard_interactivity
                        == wlr_layer::KeyboardInteractivity::Exclusive
                        || Some(l) == self.zen.layer_shell_on_demand_focus.as_ref())
                        && self.zen.mapped_layer_surfaces.contains_key(l)
                }) {
                    trace!("ignoring toplevel popup grab because the overlay layer has focus");
                    let _ = PopupManager::dismiss_popup(&root, &popup);
                    return;
                }

                let mon = self.zen.layout.monitor_for_output(output).unwrap();
                if !mon.render_above_top_layer()
                    && layers.layers_on(Layer::Top).any(|l| {
                        (l.cached_state().keyboard_interactivity
                            == wlr_layer::KeyboardInteractivity::Exclusive
                            || Some(l) == self.zen.layer_shell_on_demand_focus.as_ref())
                            && self.zen.mapped_layer_surfaces.contains_key(l)
                    })
                {
                    trace!("ignoring toplevel popup grab because the top layer has focus");
                    let _ = PopupManager::dismiss_popup(&root, &popup);
                    return;
                }

                let layout_focus = self.zen.layout.focus();
                if Some(&root) != layout_focus.map(|win| win.toplevel().wl_surface()) {
                    trace!("ignoring toplevel popup grab because another window has focus");
                    let _ = PopupManager::dismiss_popup(&root, &popup);
                    return;
                }
            }
        } else {
            trace!("ignoring popup grab because no output is active");
            let _ = PopupManager::dismiss_popup(&root, &popup);
            return;
        }

        let seat = &self.zen.seat;
        let mut grab = match self
            .zen
            .popups
            .grab_popup(root.clone(), popup, seat, serial)
        {
            Ok(grab) => grab,
            Err(err) => {
                trace!("ignoring popup grab: {err:?}");
                return;
            }
        };

        let keyboard = seat.get_keyboard().unwrap();
        let pointer = seat.get_pointer().unwrap();

        let can_receive_keyboard_focus = !self.zen.seat.input_method().keyboard_grabbed()
            && self
                .zen
                .layout
                .active_output()
                .and_then(|output| {
                    layer_map_for_output(output)
                        .layer_for_surface(&root, WindowSurfaceType::TOPLEVEL)
                        .map(|layer_surface| layer_surface.can_receive_keyboard_focus())
                })
                .unwrap_or(true);

        let keyboard_grab_mismatches = keyboard.is_grabbed()
            && !(keyboard.has_grab(serial)
                || grab.previous_serial().is_none_or(|s| keyboard.has_grab(s)));
        let pointer_grab_mismatches = pointer.is_grabbed()
            && !(pointer.has_grab(serial)
                || grab.previous_serial().is_none_or(|s| pointer.has_grab(s)));
        if (can_receive_keyboard_focus && keyboard_grab_mismatches) || pointer_grab_mismatches {
            trace!("ignoring popup grab because of current grab mismatch");
            grab.ungrab(PopupUngrabStrategy::All);
            return;
        }

        trace!("new grab for root {:?}", root);
        if can_receive_keyboard_focus {
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        self.zen.popup_grab = Some(PopupGrabState {
            root,
            grab,
            has_keyboard_grab: can_receive_keyboard_focus,
        });
    }

    fn maximize_request(&mut self, toplevel: ToplevelSurface) {
        if let Some((mapped, _)) = self
            .zen
            .layout
            .find_window_and_output_mut(toplevel.wl_surface())
        {
            mapped.set_needs_configure();

            let window = mapped.window.clone();
            self.zen.layout.set_maximized(&window, true);
        } else if let Some(unmapped) = self.zen.unmapped_windows.get_mut(toplevel.wl_surface()) {
            match &mut unmapped.state {
                InitialConfigureState::NotConfigured {
                    wants_maximized, ..
                } => {
                    *wants_maximized = true;
                }
                InitialConfigureState::Configured {
                    rules,
                    output,
                    is_pending_maximized,
                    ..
                } => {
                    let mon = output
                        .as_ref()
                        .and_then(|o| self.zen.layout.monitor_for_output(o))
                        .map(|mon| (mon, false))
                        .or_else(|| {
                            toplevel
                                .parent()
                                .and_then(|parent| self.zen.layout.find_window_and_output(&parent))
                                .and_then(|(_win, output)| output)
                                .and_then(|o| self.zen.layout.monitor_for_output(o))
                                .map(|mon| (mon, true))
                        })
                        .or_else(|| {
                            self.zen
                                .layout
                                .active_monitor_ref()
                                .map(|mon| (mon, false))
                        });

                    *output = mon
                        .filter(|(_, parent)| !parent)
                        .map(|(mon, _)| mon.output().clone());
                    let mon = mon.map(|(mon, _)| mon);

                    let ws = mon
                        .map(|mon| mon.active_workspace_ref())
                        .or_else(|| self.zen.layout.active_workspace());

                    if let Some(ws) = ws {
                        *is_pending_maximized = true;
                        toplevel.with_pending_state(|state| {
                            if !state.states.contains(xdg_toplevel::State::Fullscreen) {
                                state.states.set(xdg_toplevel::State::Maximized);
                            }
                        });
                        ws.configure_new_window(&unmapped.window, None, None, false, rules);
                    }

                    toplevel.send_configure();
                }
            }
        } else {
            error!("couldn't find the toplevel in maximize_request()");
            toplevel.send_configure();
        }
    }

    fn unmaximize_request(&mut self, toplevel: ToplevelSurface) {
        let opens_on_canvas = self.zen.layout.opens_on_canvas();
        if let Some((mapped, _)) = self
            .zen
            .layout
            .find_window_and_output_mut(toplevel.wl_surface())
        {
            mapped.set_needs_configure();

            let window = mapped.window.clone();
            self.zen.layout.set_maximized(&window, false);
        } else if let Some(unmapped) = self.zen.unmapped_windows.get_mut(toplevel.wl_surface()) {
            match &mut unmapped.state {
                InitialConfigureState::NotConfigured {
                    wants_maximized, ..
                } => {
                    *wants_maximized = false;
                }
                InitialConfigureState::Configured {
                    rules,
                    width,
                    height,
                    floating_width,
                    floating_height,
                    is_full_width,
                    output,
                    workspace_name,
                    is_pending_maximized,
                } => {
                    let mon = workspace_name
                        .as_deref()
                        .and_then(|name| self.zen.layout.monitor_for_workspace(name))
                        .map(|mon| (mon, false));

                    let mon = mon.or_else(|| {
                        output
                            .as_ref()
                            .and_then(|o| self.zen.layout.monitor_for_output(o))
                            .map(|mon| (mon, false))
                            .or_else(|| {
                                toplevel
                                    .parent()
                                    .and_then(|parent| {
                                        self.zen.layout.find_window_and_output(&parent)
                                    })
                                    .and_then(|(_win, output)| output)
                                    .and_then(|o| self.zen.layout.monitor_for_output(o))
                                    .map(|mon| (mon, true))
                            })
                            .or_else(|| {
                                self.zen
                                    .layout
                                    .active_monitor_ref()
                                    .map(|mon| (mon, false))
                            })
                    });

                    *output = mon
                        .filter(|(_, parent)| !parent)
                        .map(|(mon, _)| mon.output().clone());
                    let mon = mon.map(|(mon, _)| mon);

                    let ws = workspace_name
                        .as_deref()
                        .and_then(|name| mon.map(|mon| mon.find_named_workspace(name)))
                        .unwrap_or_else(|| {
                            mon.map(|mon| mon.active_workspace_ref())
                                .or_else(|| self.zen.layout.active_workspace())
                        });

                    if let Some(ws) = ws {
                        *is_pending_maximized = false;
                        toplevel.with_pending_state(|state| {
                            state.states.unset(xdg_toplevel::State::Maximized);
                        });

                        let is_floating = rules
                            .compute_open_floating(&toplevel, opens_on_canvas);
                        let configure_width = if is_floating {
                            *floating_width
                        } else if *is_full_width {
                            Some(PresetSize::Proportion(1.))
                        } else {
                            *width
                        };
                        let configure_height = if is_floating {
                            *floating_height
                        } else {
                            *height
                        };
                        ws.configure_new_window(
                            &unmapped.window,
                            configure_width,
                            configure_height,
                            is_floating,
                            rules,
                        );
                    }

                    toplevel.send_configure();
                }
            }
        } else {
            error!("couldn't find the toplevel in unmaximize_request()");
            toplevel.send_configure();
        }
    }

    fn fullscreen_request(
        &mut self,
        toplevel: ToplevelSurface,
        wl_output: Option<wl_output::WlOutput>,
    ) {
        let requested_output = wl_output.and_then(|o| self.zen.output_from_resource(&o));

        if let Some((mapped, current_output)) = self
            .zen
            .layout
            .find_window_and_output_mut(toplevel.wl_surface())
        {
            mapped.set_needs_configure();

            let window = mapped.window.clone();

            if let Some(requested_output) = requested_output {
                if Some(&requested_output) != current_output {
                    self.zen.layout.move_to_output(
                        Some(&window),
                        &requested_output,
                        None,
                        ActivateWindow::Smart,
                    );
                }
            }

            self.zen.layout.set_fullscreen(&window, true);
        } else if let Some(unmapped) = self.zen.unmapped_windows.get_mut(toplevel.wl_surface()) {
            match &mut unmapped.state {
                InitialConfigureState::NotConfigured {
                    wants_fullscreen, ..
                } => {
                    *wants_fullscreen = Some(requested_output);
                }
                InitialConfigureState::Configured { rules, output, .. } => {
                    let mon = requested_output
                        .as_ref()
                        .or(output.as_ref())
                        .and_then(|o| self.zen.layout.monitor_for_output(o))
                        .map(|mon| (mon, false))
                        .or_else(|| {
                            toplevel
                                .parent()
                                .and_then(|parent| self.zen.layout.find_window_and_output(&parent))
                                .and_then(|(_win, output)| output)
                                .and_then(|o| self.zen.layout.monitor_for_output(o))
                                .map(|mon| (mon, true))
                        })
                        .or_else(|| {
                            self.zen
                                .layout
                                .active_monitor_ref()
                                .map(|mon| (mon, false))
                        });

                    *output = mon
                        .filter(|(_, parent)| !parent)
                        .map(|(mon, _)| mon.output().clone());
                    let mon = mon.map(|(mon, _)| mon);

                    let ws = mon
                        .map(|mon| mon.active_workspace_ref())
                        .or_else(|| self.zen.layout.active_workspace());

                    if let Some(ws) = ws {
                        toplevel.with_pending_state(|state| {
                            state.states.set(xdg_toplevel::State::Fullscreen);
                            state.states.unset(xdg_toplevel::State::Maximized);
                        });
                        ws.configure_new_window(&unmapped.window, None, None, false, rules);
                    }

                    toplevel.send_configure();
                }
            }
        } else {
            error!("couldn't find the toplevel in fullscreen_request()");
            toplevel.send_configure();
        }
    }

    fn unfullscreen_request(&mut self, toplevel: ToplevelSurface) {
        let opens_on_canvas = self.zen.layout.opens_on_canvas();
        if let Some((mapped, _)) = self
            .zen
            .layout
            .find_window_and_output_mut(toplevel.wl_surface())
        {
            mapped.set_needs_configure();

            let window = mapped.window.clone();
            self.zen.layout.set_fullscreen(&window, false);
        } else if let Some(unmapped) = self.zen.unmapped_windows.get_mut(toplevel.wl_surface()) {
            match &mut unmapped.state {
                InitialConfigureState::NotConfigured {
                    wants_fullscreen, ..
                } => {
                    *wants_fullscreen = None;
                }
                InitialConfigureState::Configured {
                    rules,
                    width,
                    height,
                    floating_width,
                    floating_height,
                    is_full_width,
                    output,
                    workspace_name,
                    is_pending_maximized,
                } => {
                    let mon = workspace_name
                        .as_deref()
                        .and_then(|name| self.zen.layout.monitor_for_workspace(name))
                        .map(|mon| (mon, false));

                    let mon = mon.or_else(|| {
                        output
                            .as_ref()
                            .and_then(|o| self.zen.layout.monitor_for_output(o))
                            .map(|mon| (mon, false))
                            .or_else(|| {
                                toplevel
                                    .parent()
                                    .and_then(|parent| {
                                        self.zen.layout.find_window_and_output(&parent)
                                    })
                                    .and_then(|(_win, output)| output)
                                    .and_then(|o| self.zen.layout.monitor_for_output(o))
                                    .map(|mon| (mon, true))
                            })
                            .or_else(|| {
                                self.zen
                                    .layout
                                    .active_monitor_ref()
                                    .map(|mon| (mon, false))
                            })
                    });

                    *output = mon
                        .filter(|(_, parent)| !parent)
                        .map(|(mon, _)| mon.output().clone());
                    let mon = mon.map(|(mon, _)| mon);

                    let ws = workspace_name
                        .as_deref()
                        .and_then(|name| mon.map(|mon| mon.find_named_workspace(name)))
                        .unwrap_or_else(|| {
                            mon.map(|mon| mon.active_workspace_ref())
                                .or_else(|| self.zen.layout.active_workspace())
                        });

                    if let Some(ws) = ws {
                        toplevel.with_pending_state(|state| {
                            state.states.unset(xdg_toplevel::State::Fullscreen);

                            if *is_pending_maximized {
                                state.states.set(xdg_toplevel::State::Maximized);
                            }
                        });

                        let is_floating = rules
                            .compute_open_floating(&toplevel, opens_on_canvas);
                        let configure_width = if is_floating {
                            *floating_width
                        } else if *is_full_width {
                            Some(PresetSize::Proportion(1.))
                        } else {
                            *width
                        };
                        let configure_height = if is_floating {
                            *floating_height
                        } else {
                            *height
                        };
                        ws.configure_new_window(
                            &unmapped.window,
                            configure_width,
                            configure_height,
                            is_floating,
                            rules,
                        );
                    }

                    toplevel.send_configure();
                }
            }
        } else {
            error!("couldn't find the toplevel in unfullscreen_request()");
            toplevel.send_configure();
        }
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        if self
            .zen
            .unmapped_windows
            .remove(surface.wl_surface())
            .is_some()
        {
            return;
        }

        let win_out = self
            .zen
            .layout
            .find_window_and_output(surface.wl_surface());

        let Some((mapped, output)) = win_out else {
            error!("toplevel missing from both unmapped_windows and layout");
            return;
        };
        let window = mapped.window.clone();
        let output = output.cloned();

        let id = mapped.id();
        self.zen
            .stop_casts_for_target(CastTarget::Window { id: id.get() });

        self.store_unmap_snapshot(&window, output.as_ref());

        let transaction = Transaction::new();
        let blocker = transaction.blocker();
        self.backend.with_primary_renderer(|renderer| {
            self.zen
                .layout
                .start_close_animation_for_window(renderer, &window, blocker);
        });

        let active_window = self.zen.layout.focus().map(|m| &m.window);
        let was_active = active_window == Some(&window);

        self.zen.window_mru_ui.remove_window(id);
        self.zen.layout.remove_window(&window, transaction.clone());

        let surface = surface.wl_surface();
        if surface.is_alive() {
            self.add_default_dmabuf_pre_commit_hook(surface);
        }

        if !transaction.is_last() {
            transaction.register_deadline_timer(&self.zen.event_loop);
        }

        if was_active {
            self.maybe_warp_cursor_to_focus();
        }

        if let Some(output) = output {
            self.zen.queue_redraw(&output);
            self.zen.queue_redraw_mru_output();
        }
    }

    fn popup_destroyed(&mut self, surface: PopupSurface) {
        if let Some(output) = self.output_for_popup(&PopupKind::Xdg(surface)) {
            self.zen.queue_redraw(&output.clone());
        }
    }

    fn app_id_changed(&mut self, toplevel: ToplevelSurface) {
        self.update_window_rules(&toplevel);
    }

    fn title_changed(&mut self, toplevel: ToplevelSurface) {
        self.update_window_rules(&toplevel);
    }

    fn parent_changed(&mut self, toplevel: ToplevelSurface) {
        let Some(parent) = toplevel.parent() else {
            return;
        };

        if let Some((mapped, output)) = self.zen.layout.find_window_and_output_mut(&parent) {
            let output = output.cloned();
            let window = mapped.window.clone();
            if self.zen.layout.descendants_added(&window) {
                if let Some(output) = output {
                    self.zen.queue_redraw(&output);
                }
            }
        }
    }
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
        });
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: zxdg_toplevel_decoration_v1::Mode) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });

        if toplevel.is_initial_configure_sent() {
            let surface = toplevel.wl_surface();
            if let Some((mapped, _)) = self.zen.layout.find_window_and_output_mut(surface) {
                mapped.set_needs_configure();
            } else {
                toplevel.send_configure();
            }
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
        });

        if toplevel.is_initial_configure_sent() {
            let surface = toplevel.wl_surface();
            if let Some((mapped, _)) = self.zen.layout.find_window_and_output_mut(surface) {
                mapped.set_needs_configure();
            } else {
                toplevel.send_configure();
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct KdeDecorationsModeState {
    server: Cell<bool>,
}

impl KdeDecorationsModeState {
    pub fn is_server(&self) -> bool {
        self.server.get()
    }
}

impl KdeDecorationHandler for State {
    fn kde_decoration_state(&self) -> &KdeDecorationState {
        &self.zen.kde_decoration_state
    }

    fn request_mode(
        &mut self,
        surface: &WlSurface,
        decoration: &org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration,
        mode: wayland_server::WEnum<org_kde_kwin_server_decoration::Mode>,
    ) {
        let WEnum::Value(mode) = mode else {
            return;
        };

        decoration.mode(mode);

        with_states(surface, |states| {
            let state = states
                .data_map
                .get_or_insert(KdeDecorationsModeState::default);
            state
                .server
                .set(mode == org_kde_kwin_server_decoration::Mode::Server);
        });
    }
}

impl XdgForeignHandler for State {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.zen.xdg_foreign_state
    }
}

impl State {
    pub fn send_initial_configure(&mut self, toplevel: &ToplevelSurface) {
        let _span = tracy_client::span!("State::send_initial_configure");

        let Some(unmapped) = self.zen.unmapped_windows.get_mut(toplevel.wl_surface()) else {
            error!("window must be present in unmapped_windows in send_initial_configure()");
            return;
        };

        let config = self.zen.config.borrow();
        let rules = ResolvedWindowRules::compute(
            &config.window_rules,
            WindowRef::Unmapped(unmapped),
            self.zen.is_at_startup,
        );

        let Unmapped { window, state, .. } = unmapped;

        let InitialConfigureState::NotConfigured {
            wants_fullscreen,
            wants_maximized,
        } = state
        else {
            error!("window must not be already configured in send_initial_configure()");
            return;
        };

        let mon = rules
            .open_on_workspace
            .as_deref()
            .and_then(|name| self.zen.layout.monitor_for_workspace(name));

        let mon = mon.or_else(|| {
            rules
                .open_on_output
                .as_deref()
                .and_then(|name| {
                    self.zen
                        .global_space
                        .outputs()
                        .find(|output| output_matches_name(output, name))
                })
                .and_then(|o| self.zen.layout.monitor_for_output(o))
        });

        let mon = mon.or_else(|| {
            wants_fullscreen
                .as_ref()
                .and_then(|x| x.as_ref())
                .and_then(|o| self.zen.layout.monitor_for_output(o))
        });

        let mon = mon.map(|mon| (mon, false)).or_else(|| {
            toplevel
                .parent()
                .and_then(|parent| self.zen.layout.find_window_and_output(&parent))
                .and_then(|(_win, output)| output)
                .and_then(|o| self.zen.layout.monitor_for_output(o))
                .map(|mon| (mon, true))
        });

        let mon = mon.or_else(|| {
            self.zen
                .layout
                .active_monitor_ref()
                .map(|mon| (mon, false))
        });

        let output = mon
            .filter(|(_, parent)| !parent)
            .map(|(mon, _)| mon.output().clone());
        let mon = mon.map(|(mon, _)| mon);

        let mut width = None;
        let mut floating_width = None;
        let mut height = None;
        let mut floating_height = None;
        let is_full_width = rules.open_maximized.unwrap_or(false);
        let is_floating =
            rules.compute_open_floating(toplevel, self.zen.layout.opens_on_canvas());

        let ws = rules
            .open_on_workspace
            .as_deref()
            .and_then(|name| mon.map(|mon| mon.find_named_workspace(name)))
            .unwrap_or_else(|| {
                mon.map(|mon| mon.active_workspace_ref())
                    .or_else(|| self.zen.layout.active_workspace())
            });

        let mut is_pending_maximized = false;
        if let Some(ws) = ws {
            is_pending_maximized = (*wants_maximized && rules.open_maximized_to_edges.is_none())
                || rules.open_maximized_to_edges == Some(true);

            if (wants_fullscreen.is_some() && rules.open_fullscreen.is_none())
                || rules.open_fullscreen == Some(true)
            {
                toplevel.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Fullscreen);
                });
            } else if is_pending_maximized {
                toplevel.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Maximized);
                });
            }

            width = ws.resolve_default_width(rules.default_width, false);
            floating_width = ws.resolve_default_width(rules.default_width, true);
            height = ws.resolve_default_height(rules.default_height, false);
            floating_height = ws.resolve_default_height(rules.default_height, true);

            let configure_width = if is_floating {
                floating_width
            } else if is_full_width {
                Some(PresetSize::Proportion(1.))
            } else {
                width
            };
            let configure_height = if is_floating { floating_height } else { height };
            ws.configure_new_window(
                window,
                configure_width,
                configure_height,
                is_floating,
                &rules,
            );
        }

        update_tiled_state(toplevel, config.prefer_no_csd, rules.tiled_state);

        *state = InitialConfigureState::Configured {
            rules,
            width,
            height,
            floating_width,
            floating_height,
            is_full_width,
            output,
            workspace_name: ws.and_then(|w| w.name().cloned()),
            is_pending_maximized,
        };

        trace!(surface = %toplevel.wl_surface().id(), "sending initial configure");
        toplevel.send_configure();
    }

    pub fn queue_initial_configure(&self, toplevel: ToplevelSurface) {
        self.zen.event_loop.insert_idle(move |state| {
            if !toplevel.alive() {
                return;
            }

            if let Some(unmapped) = state.zen.unmapped_windows.get(toplevel.wl_surface()) {
                if unmapped.needs_initial_configure() {
                    state.send_initial_configure(&toplevel);
                }
            }
        });
    }

    pub fn popups_handle_commit(&mut self, surface: &WlSurface) {
        self.zen.popups.commit(surface);

        if let Some(popup) = self.zen.popups.find_popup(surface) {
            match popup {
                PopupKind::Xdg(ref popup) => {
                    if !popup.is_initial_configure_sent() {
                        if let Some(output) = self.output_for_popup(&PopupKind::Xdg(popup.clone()))
                        {
                            let scale = output.current_scale();
                            let transform = output.current_transform();
                            with_states(surface, |data| {
                                send_scale_transform(surface, data, scale, transform);
                            });
                        }
                        popup.send_configure().expect("initial configure failed");
                    }
                }
                PopupKind::InputMethod(_) => {
                    self.unconstrain_popup(&popup);
                }
            }
        }
    }

    pub fn output_for_popup(&self, popup: &PopupKind) -> Option<&Output> {
        let root = find_popup_root_surface(popup).ok()?;
        self.zen.output_for_root(&root)
    }

    pub fn unconstrain_popup(&self, popup: &PopupKind) {
        let _span = tracy_client::span!("Zen::unconstrain_popup");

        let Ok(root) = find_popup_root_surface(popup) else {
            return;
        };

        if let Some((mapped, _)) = self.zen.layout.find_window_and_output(&root) {
            self.unconstrain_window_popup(popup, &mapped.window);
        } else if let Some((layer_surface, output)) = self.zen.layout.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            let layer_surface = map.layer_for_surface(&root, WindowSurfaceType::TOPLEVEL)?;
            Some((layer_surface.clone(), o))
        }) {
            self.unconstrain_layer_shell_popup(popup, &layer_surface, output);
        }
    }

    fn unconstrain_window_popup(&self, popup: &PopupKind, window: &Window) {
        let mut target = self.zen.layout.popup_target_rect(window);
        target.loc -= get_popup_toplevel_coords(popup).to_f64();

        self.position_popup_within_rect(popup, target, true);
    }

    pub fn unconstrain_layer_shell_popup(
        &self,
        popup: &PopupKind,
        layer_surface: &LayerSurface,
        output: &Output,
    ) {
        let output_geo = self.zen.global_space.output_geometry(output).unwrap();
        let map = layer_map_for_output(output);
        let Some(layer_geo) = map.layer_geometry(layer_surface) else {
            return;
        };

        let mut target = Rectangle::from_size(output_geo.size);

        if matches!(layer_surface.layer(), Layer::Background | Layer::Bottom) {
            target = map.non_exclusive_zone();
        }

        target.loc -= layer_geo.loc;
        target.loc -= get_popup_toplevel_coords(popup);

        self.position_popup_within_rect(popup, target.to_f64(), false);
    }

    fn position_popup_within_rect(
        &self,
        popup: &PopupKind,
        target: Rectangle<f64, Logical>,
        padding: bool,
    ) {
        match popup {
            PopupKind::Xdg(popup) => {
                popup.with_pending_state(|state| {
                    state.geometry = if padding {
                        unconstrain_with_padding(state.positioner, target)
                    } else {
                        state
                            .positioner
                            .get_unconstrained_geometry(target.to_i32_round())
                    };
                });
            }
            PopupKind::InputMethod(popup) => {
                let text_input_rectangle = popup.text_input_rectangle();
                let mut bbox =
                    utils::bbox_from_surface_tree(popup.wl_surface(), text_input_rectangle.loc)
                        .to_f64();

                let overflow_x = (bbox.loc.x + bbox.size.w) - (target.loc.x + target.size.w);
                if overflow_x > 0. {
                    bbox.loc.x -= overflow_x;
                }

                bbox.loc.x = f64::max(bbox.loc.x, target.loc.x);

                let mut below = bbox;
                below.loc.y += f64::from(text_input_rectangle.size.h);

                let mut above = bbox;
                above.loc.y -= bbox.size.h;

                if target.loc.y + target.size.h >= below.loc.y + below.size.h {
                    popup.set_location(below.loc.to_i32_round());
                } else {
                    popup.set_location(above.loc.to_i32_round());
                }
            }
        }
    }

    pub fn update_reactive_popups(&self, window: &Window) {
        let _span = tracy_client::span!("Zen::update_reactive_popups");

        for (popup, _) in PopupManager::popups_for_surface(
            window.toplevel().expect("no x11 support").wl_surface(),
        ) {
            match &popup {
                xdg_popup @ PopupKind::Xdg(popup) => {
                    if popup.with_pending_state(|state| state.positioner.reactive) {
                        self.unconstrain_window_popup(xdg_popup, window);
                        if let Err(err) = popup.send_pending_configure() {
                            warn!("error re-configuring reactive popup: {err:?}");
                        }
                    }
                }
                PopupKind::InputMethod(_) => (),
            }
        }
    }

    pub fn update_window_rules(&mut self, toplevel: &ToplevelSurface) {
        let config = self.zen.config.borrow();
        let window_rules = &config.window_rules;

        if let Some(unmapped) = self.zen.unmapped_windows.get_mut(toplevel.wl_surface()) {
            let new_rules = ResolvedWindowRules::compute(
                window_rules,
                WindowRef::Unmapped(unmapped),
                self.zen.is_at_startup,
            );
            if let InitialConfigureState::Configured { rules, .. } = &mut unmapped.state {
                *rules = new_rules;
            }
        } else if let Some((mapped, output)) = self
            .zen
            .layout
            .find_window_and_output_mut(toplevel.wl_surface())
        {
            if mapped.recompute_window_rules(window_rules, self.zen.is_at_startup) {
                drop(config);
                let output = output.cloned();
                let window = mapped.window.clone();
                self.zen.layout.update_window(&window, None);

                if let Some(output) = output {
                    self.zen.queue_redraw(&output);
                }
            }
        }
    }
}

fn unconstrain_with_padding(
    positioner: PositionerState,
    target: Rectangle<f64, Logical>,
) -> Rectangle<i32, Logical> {
    const PADDING: f64 = 8.;

    let mut padded = target;
    if PADDING * 2. < padded.size.w {
        padded.loc.x += PADDING;
        padded.size.w -= PADDING * 2.;
    }
    if PADDING * 2. < padded.size.h {
        padded.loc.y += PADDING;
        padded.size.h -= PADDING * 2.;
    }

    if padded == target {
        return positioner.get_unconstrained_geometry(target.to_i32_round());
    }

    let mut no_resize = positioner;
    no_resize
        .constraint_adjustment
        .remove(ConstraintAdjustment::ResizeX);
    no_resize
        .constraint_adjustment
        .remove(ConstraintAdjustment::ResizeY);

    let geo = no_resize.get_unconstrained_geometry(padded.to_i32_round());
    if padded.contains_rect(geo.to_f64()) {
        return geo;
    }

    positioner.get_unconstrained_geometry(target.to_i32_round())
}

pub fn add_mapped_toplevel_pre_commit_hook(toplevel: &ToplevelSurface) -> HookId {
    add_pre_commit_hook::<State, _>(toplevel.wl_surface(), move |state, _dh, surface| {
        let _span = tracy_client::span!("mapped toplevel pre-commit");
        let span =
            trace_span!("toplevel pre-commit", surface = %surface.id(), serial = Empty).entered();

        let Some((mapped, output)) = state.zen.layout.find_window_and_output_mut(surface) else {
            error!("pre-commit hook for mapped surfaces must be removed upon unmapping");
            return;
        };

        let (got_unmapped, dmabuf, commit_serial) = with_states(surface, |states| {
            let (got_unmapped, dmabuf) = {
                let mut guard = states.cached_state.get::<SurfaceAttributes>();
                match guard.pending().buffer.as_ref() {
                    Some(BufferAssignment::NewBuffer(buffer)) => {
                        let dmabuf = get_dmabuf(buffer).cloned().ok();
                        (false, dmabuf)
                    }
                    Some(BufferAssignment::Removed) => (true, None),
                    None => (false, None),
                }
            };

            let role = states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap();
            let serial = role.last_acked.as_ref().map(|c| c.serial);

            (got_unmapped, dmabuf, serial)
        });

        let mut transaction_for_dmabuf = None;
        let mut animate = false;
        if let Some(serial) = commit_serial {
            if !span.is_disabled() {
                span.record("serial", format!("{serial:?}"));
            }

            if let Some(transaction) = mapped.take_pending_transaction(serial) {
                let disable = state.zen.config.borrow().debug.disable_transactions;
                if !transaction.is_completed() && !disable {
                    transaction.register_deadline_timer(&state.zen.event_loop);

                    let is_last = transaction.is_last();

                    if !is_last {
                        if let Some(client) = surface.client() {
                            transaction.add_notification(
                                state.zen.blocker_cleared_tx.clone(),
                                client.clone(),
                            );
                            add_blocker(surface, transaction.blocker());
                        }
                    }

                    transaction_for_dmabuf = Some(transaction);
                }
            }

            animate = mapped.should_animate_commit(serial);
        } else if !got_unmapped {
            error!("commit on a mapped surface without a configured serial");
        };

        if let Some((blocker, source)) =
            dmabuf.and_then(|dmabuf| dmabuf.generate_blocker(Interest::READ).ok())
        {
            if let Some(client) = surface.client() {
                let res = state
                    .zen
                    .event_loop
                    .insert_source(source, move |_, _, state| {
                        drop(transaction_for_dmabuf.take());

                        let display_handle = state.zen.display_handle.clone();
                        state
                            .client_compositor_state(&client)
                            .blocker_cleared(state, &display_handle);

                        Ok(())
                    });
                if res.is_ok() {
                    add_blocker(surface, blocker);
                    trace!("added dmabuf blocker");
                }
            }
        }

        let window = mapped.window.clone();
        if got_unmapped {
            let output = output.cloned();
            state.store_unmap_snapshot(&window, output.as_ref());
        } else {
            if animate {
                state.backend.with_primary_renderer(|renderer| {
                    mapped.store_animation_snapshot(renderer);
                });
            }

            state.zen.layout.clear_unmap_snapshot(&window);
        }
    })
}
