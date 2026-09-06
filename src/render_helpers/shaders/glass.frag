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

// glass
uniform float refraction_strength;
uniform float refraction_falloff;
uniform float squircle_n;
uniform vec4 glass_tint;
uniform float spec_strength;
uniform float spec_power;
uniform vec2 light_dir;

float zen_rounding_alpha(vec2 coords, vec2 size, vec4 corner_radius);
vec4 postprocess(vec4 color);

// sdf
float smax(float a, float b, float k) {
    float h = clamp(0.5 + 0.5 * (a - b) / k, 0.0, 1.0);
    return mix(b, a, h) + k * h * (1.0 - h);
}

float sd_squircle(vec2 p, vec2 half_size, float r, float n, float k) {
    vec2 q = abs(p) - (half_size - vec2(r));
    vec2 m = max(q, vec2(0.0));
    float outside = pow(pow(m.x, n) + pow(m.y, n), 1.0 / n);
    float inside = min(smax(q.x, q.y, k), 0.0);
    return outside + inside - r;
}

vec2 sd_normal(vec2 p, vec2 half_size, float r, float n, float k) {
    float e = 1.0;
    float dx = sd_squircle(p + vec2(e, 0.0), half_size, r, n, k)
             - sd_squircle(p - vec2(e, 0.0), half_size, r, n, k);
    float dy = sd_squircle(p + vec2(0.0, e), half_size, r, n, k)
             - sd_squircle(p - vec2(0.0, e), half_size, r, n, k);
    vec2 g = vec2(dx, dy);
    float len = length(g);
    return len > 0.0001 ? g / len : vec2(0.0);
}

void main() {
    vec3 coords_geo = input_to_geo * vec3(v_coords, 1.0);

    if (coords_geo.x < 0.0 || 1.0 < coords_geo.x || coords_geo.y < 0.0 || 1.0 < coords_geo.y) {
        gl_FragColor = vec4(0.0);
        return;
    }

    vec2 half_size = geo_size * 0.5;
    vec2 p = coords_geo.xy * geo_size - half_size;

    float r = max(max(corner_radius.x, corner_radius.y),
                  max(corner_radius.z, corner_radius.w));

    float falloff = max(refraction_falloff, 0.5);
    float n = max(squircle_n, 2.0);
    float k = max(falloff, 1.0);

    float d = sd_squircle(p, half_size, r, n, k);
    vec2 normal = sd_normal(p, half_size, r, n, k);

    // refraction
    float edge = clamp(exp(d / falloff), 0.0, 1.0);

    vec2 offset_px = normal * edge * refraction_strength;
    vec2 offset_geo = offset_px / max(geo_size, vec2(1.0));

    mat2 lin = mat2(input_to_geo[0].xy, input_to_geo[1].xy);
    float det = lin[0][0] * lin[1][1] - lin[0][1] * lin[1][0];
    vec2 offset_uv = vec2(0.0);
    if (abs(det) > 0.000001) {
        mat2 inv = mat2(lin[1][1], -lin[0][1], -lin[1][0], lin[0][0]) / det;
        offset_uv = inv * offset_geo;
    }

    // The capture is clipped to the output, so a window straddling a screen edge has a
    // texture that stops short of its own geometry. The refraction offset is largest at
    // exactly that edge, so without this it samples past the end of the capture and gets
    // whatever the wrap mode gives back, which reads as a pale smear hanging off screen.
    vec2 uv = clamp(v_coords + offset_uv, vec2(0.0), vec2(1.0));
    vec4 color = texture2D(tex, uv);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif

    color = postprocess(color);

    // tint
    if (glass_tint.a > 0.0) {
        color.rgb = mix(color.rgb, glass_tint.rgb * color.a, glass_tint.a);
    }

    // specular
    if (spec_strength > 0.0) {
        float facing = max(dot(normal, normalize(light_dir)), 0.0);
        float spec = pow(facing, max(spec_power, 1.0)) * edge * spec_strength;
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
