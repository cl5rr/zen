use zen_config::layer_rule::{LayerRule, Match};
use zen_config::utils::MergeWith as _;
use zen_config::{BackgroundEffect, BlockOutFrom, CornerRadius, ResolvedPopupsRules, ShadowRule};
use smithay::desktop::LayerSurface;
use smithay::wayland::shell::wlr_layer::Layer;

pub mod mapped;
pub use mapped::MappedLayer;

#[derive(Debug, Default, PartialEq)]
pub struct ResolvedLayerRules {
    pub opacity: Option<f32>,

    pub block_out_from: Option<BlockOutFrom>,

    pub shadow: ShadowRule,

    pub geometry_corner_radius: Option<CornerRadius>,

    pub place_within_backdrop: bool,

    pub baba_is_float: bool,

    pub background_effect: BackgroundEffect,

    pub popups: ResolvedPopupsRules,
}

impl ResolvedLayerRules {
    pub fn compute(rules: &[LayerRule], surface: &LayerSurface, is_at_startup: bool) -> Self {
        let _span = tracy_client::span!("ResolvedLayerRules::compute");

        let mut resolved = ResolvedLayerRules::default();

        for rule in rules {
            let matches = |m: &Match| {
                if let Some(at_startup) = m.at_startup {
                    if at_startup != is_at_startup {
                        return false;
                    }
                }

                surface_matches(surface, m)
            };

            if !(rule.matches.is_empty() || rule.matches.iter().any(matches)) {
                continue;
            }

            if rule.excludes.iter().any(matches) {
                continue;
            }

            if let Some(x) = rule.opacity {
                resolved.opacity = Some(x);
            }
            if let Some(x) = rule.block_out_from {
                resolved.block_out_from = Some(x);
            }
            if let Some(x) = rule.geometry_corner_radius {
                resolved.geometry_corner_radius = Some(x);
            }
            if let Some(x) = rule.place_within_backdrop {
                resolved.place_within_backdrop = x;
            }
            if let Some(x) = rule.baba_is_float {
                resolved.baba_is_float = x;
            }

            resolved.shadow.merge_with(&rule.shadow);

            resolved
                .background_effect
                .merge_with(&rule.background_effect);

            resolved.popups.merge_with(&rule.popups);
        }

        resolved
    }
}

fn surface_matches(surface: &LayerSurface, m: &Match) -> bool {
    if let Some(namespace_re) = &m.namespace {
        if !namespace_re.0.is_match(surface.namespace()) {
            return false;
        }
    }

    if let Some(layer) = m.layer {
        let surface_layer = match surface.layer() {
            Layer::Background => zen_ipc::Layer::Background,
            Layer::Bottom => zen_ipc::Layer::Bottom,
            Layer::Top => zen_ipc::Layer::Top,
            Layer::Overlay => zen_ipc::Layer::Overlay,
        };
        if layer != surface_layer {
            return false;
        }
    }

    true
}
