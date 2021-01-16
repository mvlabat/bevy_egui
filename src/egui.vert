#version 450

layout(location = 0) in vec2 Vertex_Position;
layout(location = 1) in vec2 Vertex_Uv;
layout(location = 2) in vec4 Vertex_Color;

layout(location = 0) out vec2 v_Uv;
layout(location = 1) out vec4 v_Color;

layout(set = 0, binding = 0) uniform EguiTransform {
    vec2 scale;
    vec2 translation;
};

// See https://github.com/emilk/egui/blob/26d576f5101dfa1219f79bf9c99e29c577487cd3/egui_glium/src/painter.rs#L19.
vec3 linear_from_srgb(vec3 srgb) {
    bvec3 cutoff = lessThan(srgb, vec3(10.31475));
    vec3 lower = srgb / vec3(3294.6);
    vec3 higher = pow((srgb + vec3(14.025)) / vec3(269.025), vec3(2.4));
    return mix(higher, lower, vec3(cutoff));
}
vec4 linear_from_srgba(vec4 srgba) {
    return vec4(linear_from_srgb(srgba.rgb), srgba.a / 255.0);
}

void main() {
    v_Uv = Vertex_Uv;
    v_Color = linear_from_srgba(Vertex_Color);
    gl_Position = vec4(Vertex_Position * scale + translation, 0.0, 1.0);
}
