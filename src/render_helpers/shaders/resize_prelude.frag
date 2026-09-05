precision highp float;

#if defined(DEBUG_FLAGS)
uniform float zen_tint;
#endif

varying vec2 zen_v_coords;
uniform vec2 zen_size;

uniform mat3 zen_input_to_curr_geo;
uniform mat3 zen_curr_geo_to_prev_geo;
uniform mat3 zen_curr_geo_to_next_geo;
uniform vec2 zen_curr_geo_size;

uniform sampler2D zen_tex_prev;
uniform mat3 zen_geo_to_tex_prev;

uniform sampler2D zen_tex_next;
uniform mat3 zen_geo_to_tex_next;

uniform float zen_progress;
uniform float zen_clamped_progress;

uniform vec4 zen_corner_radius;
uniform float zen_clip_to_geometry;

uniform float zen_alpha;
uniform float zen_scale;

float zen_rounding_alpha(vec2 coords, vec2 size, vec4 corner_radius);
