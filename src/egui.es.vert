#version 300 es

in vec3 Vertex_Position;
in vec2 Vertex_Uv;
in vec4 Vertex_Color;

out vec2 v_Uv;
out vec4 v_Color;

layout(std140) uniform EguiTransform {
    vec2 scale;
    vec2 translation;
};

void main() {
    v_Uv = Vertex_Uv;
    v_Color = Vertex_Color;
    v_Color.a = 1.0;
    gl_Position = vec4(Vertex_Position * vec3(scale, 1.0) + vec3(translation, 0.0), 1.0);
}
