precision highp float;

#if defined(DEBUG_FLAGS)
uniform float zen_tint;
#endif

varying vec2 zen_v_coords;
uniform vec2 zen_size;

uniform mat3 zen_input_to_geo;
uniform vec2 zen_geo_size;

uniform sampler2D zen_tex;
uniform mat3 zen_geo_to_tex;

uniform float zen_progress;
uniform float zen_clamped_progress;
uniform float zen_random_seed;

uniform float zen_alpha;
uniform float zen_scale;

