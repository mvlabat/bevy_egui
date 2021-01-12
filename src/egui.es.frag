#version 300 es
precision mediump float;

in vec2 v_Uv;
in vec4 v_Color;

out vec4 o_Target;

uniform sampler2D EguiTexture_texture;

// See https://github.com/emilk/egui/blob/f3b011a8cdc44e2f764d54b51907474395a6e83b/egui_web/src/webgl.rs#L47.

// 0-255 sRGB  from  0-1 linear
vec3 srgb_from_linear(vec3 rgb) {
    bvec3 cutoff = lessThan(rgb, vec3(0.0031308));
    vec3 lower = rgb * vec3(3294.6);
    vec3 higher = vec3(269.025) * pow(rgb, vec3(1.0 / 2.4)) - vec3(14.025);
    return mix(higher, lower, vec3(cutoff));
}
vec4 srgba_from_linear(vec4 rgba) {
    return vec4(srgb_from_linear(rgba.rgb), 255.0 * rgba.a);
}
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
    vec4 texture_rgba = linear_from_srgba(texture(EguiTexture_texture, v_Uv) * 255.0);
    o_Target = srgba_from_linear(v_Color * texture_rgba) / 255.0;
}
