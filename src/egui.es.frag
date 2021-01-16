#version 300 es
precision mediump float;

in vec2 v_Uv;
in vec4 v_Color;

out vec4 o_Target;

uniform sampler2D EguiTexture_texture;

vec4 encodeSRGB(vec4 linearRGB_in) {
    vec3 linearRGB = linearRGB_in.rgb;
    vec3 a = 12.92 * linearRGB;
    vec3 b = 1.055 * pow(linearRGB, vec3(1.0 / 2.4)) - 0.055;
    vec3 c = step(vec3(0.0031308), linearRGB);
    return vec4(mix(a, b, c), linearRGB_in.a);
}

void main() {
    o_Target = encodeSRGB(v_Color * texture(EguiTexture_texture, v_Uv));
}
