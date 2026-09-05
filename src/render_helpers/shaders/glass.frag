#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

uniform float zen_scale;

uniform vec2 geo_size;
uniform vec4 corner_radius;
uniform mat3 input_to_geo;

// How far, in geometry pixels, the surface bends light at its very edge.
uniform float refraction_strength;
// Distance in pixels over which refraction decays inward. Small values keep the effect a rim.
uniform float refraction_falloff;
// Superellipse exponent for the corners. ~2 is a circular arc; 4-5 is the continuous-curvature
// "squircle" that reads as Apple's.
uniform float squircle_n;
// Material tint, with opacity in the alpha.
//
// This is the knob that spans the whole range the material can be: alpha 0 is pure refraction,
// glass you see straight through; intermediate values frost it toward the tint colour; alpha 1
// is fully opaque and the backdrop stops showing at all. Applied *after* refraction, so even a
// mostly-opaque pane still bends its rim rather than becoming a flat rectangle.
uniform vec4 glass_tint;

// Specular rim.
uniform float spec_strength;
uniform float spec_power;
uniform vec2 light_dir;

float zen_rounding_alpha(vec2 coords, vec2 size, vec4 corner_radius);
vec4 postprocess(vec4 color);

// Signed distance to a superellipse-cornered rectangle.
//
// `p` is relative to the centre, `half_size` is half the extent, `r` the corner inset. Negative
// inside, zero on the boundary. At n = 2 this is the usual rounded rect; raising n straightens
// the corner's midpoint while keeping its ends tangent, which is the whole trick behind
// continuous corner curvature.
float sd_squircle(vec2 p, vec2 half_size, float r, float n) {
    vec2 q = abs(p) - (half_size - vec2(r));
    vec2 m = max(q, vec2(0.0));
    float outside = pow(pow(m.x, n) + pow(m.y, n), 1.0 / n);
    float inside = min(max(q.x, q.y), 0.0);
    return outside + inside - r;
}

// Outward normal of the surface, by central difference.
//
// Four extra SDF evaluations. Cheaper than it sounds, and far simpler than deriving the
// analytic gradient of a superellipse.
vec2 sd_normal(vec2 p, vec2 half_size, float r, float n) {
    float e = 1.0;
    float dx = sd_squircle(p + vec2(e, 0.0), half_size, r, n)
             - sd_squircle(p - vec2(e, 0.0), half_size, r, n);
    float dy = sd_squircle(p + vec2(0.0, e), half_size, r, n)
             - sd_squircle(p - vec2(0.0, e), half_size, r, n);
    vec2 g = vec2(dx, dy);
    float len = length(g);
    return len > 0.0001 ? g / len : vec2(0.0);
}

void main() {
    vec3 coords_geo = input_to_geo * vec3(v_coords, 1.0);

    if (coords_geo.x < 0.0 || 1.0 < coords_geo.x || coords_geo.y < 0.0 || 1.0 < coords_geo.y) {
        // Outside the geometry there is nothing to refract.
        gl_FragColor = vec4(0.0);
        return;
    }

    // Position in geometry pixels, relative to the centre.
    vec2 half_size = geo_size * 0.5;
    vec2 p = coords_geo.xy * geo_size - half_size;

    // One radius for the SDF. The per-corner radii still drive the alpha below; this only
    // shapes where light bends, and a rim does not need to be per-corner exact.
    float r = max(max(corner_radius.x, corner_radius.y),
                  max(corner_radius.z, corner_radius.w));

    float d = sd_squircle(p, half_size, r, max(squircle_n, 2.0));
    vec2 normal = sd_normal(p, half_size, r, max(squircle_n, 2.0));

    // Refraction is an edge phenomenon: real glass bends light where it is thick and curved,
    // which for a pane is the rim. Decay inward so the middle stays honest.
    float edge = exp(d / max(refraction_falloff, 0.5));
    edge = clamp(edge, 0.0, 1.0);

    // Displacement in geometry pixels, converted to texture space through the inverse of
    // input_to_geo's linear part -- so the bend is the same size on screen whatever the
    // element's transform happens to be.
    vec2 offset_px = normal * edge * refraction_strength;
    vec2 offset_geo = offset_px / max(geo_size, vec2(1.0));

    mat2 lin = mat2(input_to_geo[0].xy, input_to_geo[1].xy);
    float det = lin[0][0] * lin[1][1] - lin[0][1] * lin[1][0];
    vec2 offset_uv = vec2(0.0);
    if (abs(det) > 0.000001) {
        mat2 inv = mat2(lin[1][1], -lin[0][1], -lin[1][0], lin[0][0]) / det;
        offset_uv = inv * offset_geo;
    }

    vec4 color = texture2D(tex, v_coords + offset_uv);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif

    color = postprocess(color);

    // Tint toward the material colour. Operating on premultiplied colour, so the tint is
    // scaled by coverage rather than bleeding outside the shape.
    if (glass_tint.a > 0.0) {
        color.rgb = mix(color.rgb, glass_tint.rgb * color.a, glass_tint.a);
    }

    // Specular rim: a bright edge where the surface turns toward the light. This is what makes
    // it read as a solid pane rather than a blurred hole, and it is the cheapest half of the
    // impression.
    if (spec_strength > 0.0) {
        float facing = max(dot(normal, normalize(light_dir)), 0.0);
        float spec = pow(facing, max(spec_power, 1.0)) * edge * spec_strength;
        // Premultiplied, so the highlight scales with coverage rather than punching through.
        color.rgb += vec3(spec) * color.a;
    }

    color = color * zen_rounding_alpha(coords_geo.xy * geo_size, geo_size, corner_radius);
    color = color * alpha;

#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif

    gl_FragColor = color;
}
