use glam::Vec3;
use smithay::utils::{Logical, Rectangle};

use crate::render_helpers::framebuffer_effect::geo_to_mask;

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

fn map(m: glam::Mat3, x: f32, y: f32) -> (f32, f32) {
    let v = m * Vec3::new(x, y, 1.);
    (v.x, v.y)
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
}

#[test]
fn a_panel_the_size_of_its_glass_maps_onto_itself() {
    let r = rect(100., 50., 200., 100.);
    let m = geo_to_mask(r, r, false);
    for p in [(0., 0.), (1., 1.), (0.5, 0.25), (1., 0.)] {
        assert!(close(map(m, p.0, p.1), p), "{p:?} moved to {:?}", map(m, p.0, p.1));
    }
}

#[test]
fn a_panel_inside_a_larger_glass_rect_lands_where_it_is_drawn() {
    let clip = rect(100., 50., 200., 100.);
    let panel = rect(150., 50., 100., 100.);
    let m = geo_to_mask(clip, panel, false);

    assert!(close(map(m, 0.25, 0.), (0., 0.)), "the panel's left edge");
    assert!(close(map(m, 0.75, 1.), (1., 1.)), "the panel's far corner");
    assert!(close(map(m, 0.5, 0.5), (0.5, 0.5)), "the middle");
    assert!(map(m, 0., 0.).0 < 0., "left of the panel falls outside it");
}

#[test]
fn a_y_inverted_buffer_is_read_upside_down() {
    let r = rect(0., 0., 300., 200.);
    let m = geo_to_mask(r, r, true);
    assert!(close(map(m, 0.2, 0.), (0.2, 1.)));
    assert!(close(map(m, 0.2, 1.), (0.2, 0.)));
}

#[test]
fn an_empty_panel_cannot_divide_by_zero() {
    let m = geo_to_mask(rect(0., 0., 100., 100.), rect(0., 0., 0., 0.), false);
    let (x, y) = map(m, 0.5, 0.5);
    assert!(x.is_finite() && y.is_finite());
}

#[test]
fn alpha_mask_is_a_background_effect_option() {
    let config = zen_config::Config::parse_mem(
        r#"
        layer-rule {
            match namespace="^zen-shell$"
            background-effect {
                glass true
                alpha-mask true
            }
        }
        "#,
    )
    .unwrap();
    let effect = config.layer_rules[0].background_effect;
    assert_eq!(effect.glass, Some(true));
    assert_eq!(effect.alpha_mask, Some(true));
}

// glass off
fn effect_for(glass_off: bool, rule: zen_config::BackgroundEffect) -> crate::render_helpers::background_effect::Options {
    use crate::render_helpers::background_effect::BackgroundEffect;

    let mut e = BackgroundEffect::new();
    let glass = zen_config::Glass {
        off: glass_off,
        ..Default::default()
    };
    e.update_config(zen_config::Blur::default(), glass);
    e.update_render_elements(Default::default(), rule, false);
    e.options()
}

fn shell_rule() -> zen_config::BackgroundEffect {
    zen_config::BackgroundEffect {
        glass: Some(true),
        blur: Some(true),
        alpha_mask: Some(true),
        ..Default::default()
    }
}

#[test]
fn turning_glass_off_keeps_the_mask() {
    let o = effect_for(true, shell_rule());
    assert!(
        !o.xray,
        "with glass off the rule fell back to x-ray, which paints the wallpaper over the whole \
         layer and hides everything between"
    );
    assert!(o.alpha_mask, "the blur still has to follow the shape the layer paints");
    assert!(o.clear, "and without the glass itself: no tint, bend or rim");
}

#[test]
fn with_glass_on_nothing_changes() {
    let o = effect_for(false, shell_rule());
    assert!(o.glass && o.alpha_mask && !o.clear && !o.xray);
}

#[test]
fn a_rule_with_nothing_left_to_draw_draws_nothing() {
    let rule = zen_config::BackgroundEffect {
        glass: Some(true),
        blur: Some(false),
        alpha_mask: Some(true),
        ..Default::default()
    };
    let o = effect_for(true, rule);
    assert!(!o.glass && !o.blur && !o.alpha_mask, "{o:?}");
}
