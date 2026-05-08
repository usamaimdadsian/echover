#version 450

layout(location = 0) in vec4 v_color;
layout(location = 1) in vec2 v_pos;
layout(location = 2) in vec2 v_rect_min;
layout(location = 3) in vec2 v_rect_max;
layout(location = 4) in float v_radius;
layout(location = 5) flat in uint v_kind;
layout(location = 6) in vec2 v_uv;

layout(set = 0, binding = 0) uniform sampler2D u_atlas;

layout(location = 0) out vec4 out_color;

// Signed distance to a rounded rectangle.
// d > 0 outside, d < 0 inside, d == 0 on the edge.
float sd_rounded_rect(vec2 p, vec2 center, vec2 half_size, float radius) {
    vec2 q = abs(p - center) - half_size + vec2(radius);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

void main() {
    if (v_kind == 1u) {
        // Glyph quad: the atlas is R8, alpha lives in the red channel.
        float coverage = texture(u_atlas, v_uv).r;
        if (coverage <= 0.0) {
            discard;
        }
        out_color = vec4(v_color.rgb, v_color.a * coverage);
        return;
    }

    vec2 center = 0.5 * (v_rect_min + v_rect_max);
    vec2 half_size = 0.5 * (v_rect_max - v_rect_min);
    float dist = sd_rounded_rect(v_pos, center, half_size, v_radius);

    // Antialiased coverage: feather over a single pixel around the edge.
    float alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
    if (alpha <= 0.0) {
        discard;
    }
    out_color = vec4(v_color.rgb, v_color.a * alpha);
}
