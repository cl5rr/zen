use smithay::desktop::{layer_map_for_output, LayerSurface, PopupKind, WindowSurfaceType};
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{add_pre_commit_hook, get_parent, with_states, HookId};
use smithay::wayland::shell::wlr_layer::{
    self, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceCachedState, LayerSurfaceData,
    WlrLayerShellHandler, WlrLayerShellState,
};
use smithay::wayland::shell::xdg::PopupSurface;

use crate::layer::{MappedLayer, ResolvedLayerRules};
use crate::state::State;
use crate::utils::{is_mapped, output_size, send_scale_transform};

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.zen.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        wl_output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = if let Some(wl_output) = &wl_output {
            self.zen.output_from_resource(wl_output)
        } else {
            self.zen.layout.active_output().cloned()
        };
        let Some(output) = output else {
            warn!("no output for new layer surface, closing");
            surface.send_close();
            return;
        };

        let wl_surface = surface.wl_surface().clone();
        let is_new = self.zen.unmapped_layer_surfaces.insert(wl_surface);
        assert!(is_new);

        let mut map = layer_map_for_output(&output);
        map.map_layer(&LayerSurface::new(surface, namespace))
            .unwrap();
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let wl_surface = surface.wl_surface();
        self.zen.unmapped_layer_surfaces.remove(wl_surface);

        let output = if let Some((output, mut map, layer)) =
            self.zen.layout.outputs().find_map(|o| {
                let map = layer_map_for_output(o);
                let layer = map
                    .layers()
                    .find(|&layer| layer.layer_surface() == &surface)
                    .cloned();
                layer.map(|layer| (o.clone(), map, layer))
            }) {
            map.unmap_layer(&layer);
            self.zen.mapped_layer_surfaces.remove(&layer);
            Some(output)
        } else {
            None
        };
        if let Some(output) = output {
            self.zen.output_resized(&output);
        }
    }

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        self.unconstrain_popup(&PopupKind::Xdg(popup));
    }
}

impl State {
    pub fn layer_shell_handle_commit(&mut self, surface: &WlSurface) -> bool {
        let mut root_surface = surface.clone();
        while let Some(parent) = get_parent(&root_surface) {
            root_surface = parent;
        }

        let output = self
            .zen
            .layout
            .outputs()
            .find(|o| {
                let map = layer_map_for_output(o);
                map.layer_for_surface(&root_surface, WindowSurfaceType::TOPLEVEL)
                    .is_some()
            })
            .cloned();
        let Some(output) = output else {
            return false;
        };

        if surface != &root_surface {
            self.zen.queue_redraw(&output);
            return true;
        }

        let mut map = layer_map_for_output(&output);

        map.arrange();

        let layer = map
            .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .unwrap();

        if is_mapped(surface) {
            let was_unmapped = self.zen.unmapped_layer_surfaces.remove(surface);

            if was_unmapped {
                let config = self.zen.config.borrow();

                let rules = &config.layer_rules;
                let rules = ResolvedLayerRules::compute(rules, layer, self.zen.is_at_startup);

                let output_size = output_size(&output);
                let scale = output.current_scale().fractional_scale();

                let hook = add_mapped_layer_pre_commit_hook(layer);
                let mapped = MappedLayer::new(
                    layer.clone(),
                    hook,
                    rules,
                    output_size,
                    scale,
                    self.zen.clock.clone(),
                    &config,
                );

                let prev = self
                    .zen
                    .mapped_layer_surfaces
                    .insert(layer.clone(), mapped);
                if prev.is_some() {
                    error!("MappedLayer was present for an unmapped surface");
                }
            } else {
                if let Some(mapped) = self.zen.mapped_layer_surfaces.get_mut(layer) {
                    if mapped.take_recompute_rules_on_commit() {
                        let config = self.zen.config.borrow();
                        if mapped
                            .recompute_layer_rules(&config.layer_rules, self.zen.is_at_startup)
                        {
                            mapped.update_config(&config);
                        }
                    }
                } else {
                    error!("MappedLayer missing for a mapped surface");
                }
            }

            let on_demand = layer.cached_state().keyboard_interactivity
                == wlr_layer::KeyboardInteractivity::OnDemand;
            if was_unmapped && on_demand {
                self.zen.layer_shell_on_demand_focus = Some(layer.clone());
            }
        } else {
            if self.zen.mapped_layer_surfaces.remove(layer).is_some() {
                self.zen.unmapped_layer_surfaces.insert(surface.clone());
            } else {
                let initial_configure_sent = with_states(surface, |states| {
                    states
                        .data_map
                        .get::<LayerSurfaceData>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .initial_configure_sent
                });
                if !initial_configure_sent {
                    let scale = output.current_scale();
                    let transform = output.current_transform();
                    with_states(surface, |data| {
                        send_scale_transform(surface, data, scale, transform);
                    });

                    layer.layer_surface().send_configure();
                }
            }
        }

        drop(map);

        self.zen.output_resized(&output);

        true
    }
}

fn add_mapped_layer_pre_commit_hook(layer: &LayerSurface) -> HookId {
    add_pre_commit_hook::<State, _>(layer.wl_surface(), move |state, _dh, surface| {
        let layer_changed = with_states(surface, |states| {
            let mut guard = states.cached_state.get::<LayerSurfaceCachedState>();
            let pending_layer = guard.pending().layer;
            let current_layer = guard.current().layer;
            pending_layer != current_layer
        });

        if layer_changed {
            for mapped in state.zen.mapped_layer_surfaces.values_mut() {
                if mapped.surface().wl_surface() == surface {
                    mapped.set_recompute_rules_on_commit();
                    break;
                }
            }
        }
    })
}
