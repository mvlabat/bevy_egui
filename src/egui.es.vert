#version 300 es

in vec2 Vertex_Position;
in vec2 Vertex_Uv;
in vec4 Vertex_Color;

out vec2 v_Uv;
out vec4 v_Color;

layout(std140) uniform EguiTransform {
    vec2 scale;
    vec2 translation;
};

// See https://github.com/emilk/egui/blob/f3b011a8cdc44e2f764d54b51907474395a6e83b/egui_web/src/webgl.rs#L15.

// 0-1 linear  from  0-255 sRGB
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
