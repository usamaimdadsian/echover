#version 450

layout(location = 0) in vec2 in_pos;
layout(location = 1) in vec4 in_color;
layout(location = 2) in vec2 in_rect_min;
layout(location = 3) in vec2 in_rect_max;
layout(location = 4) in float in_radius;
layout(location = 5) in uint in_kind;
layout(location = 6) in vec2 in_uv;

layout(push_constant) uniform PushConstants {
    vec2 screen_size;
} pc;

layout(location = 0) out vec4 v_color;
layout(location = 1) out vec2 v_pos;
layout(location = 2) out vec2 v_rect_min;
layout(location = 3) out vec2 v_rect_max;
layout(location = 4) out float v_radius;
layout(location = 5) flat out uint v_kind;
layout(location = 6) out vec2 v_uv;

void main() {
    vec2 ndc = (in_pos / pc.screen_size) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_color = in_color;
    v_pos = in_pos;
    v_rect_min = in_rect_min;
    v_rect_max = in_rect_max;
    v_radius = in_radius;
    v_kind = in_kind;
    v_uv = in_uv;
}
