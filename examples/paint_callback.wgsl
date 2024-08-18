const POS = array(
    vec2f(0, -1) * 0.5,
    vec2f(1, 1) * 0.5,
    vec2f(-1, 1) * 0.5,
);

@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    if vertex_index == 0 {
        return vec4f(POS[0], 0, 1);
    } else if vertex_index == 1 {
        return vec4f(POS[1], 0, 1);
    } else {
        return vec4f(POS[2], 0, 1);
    }
}

@fragment
fn fragment() -> @location(0) vec4f {
    return vec4f(1, 1, 0, 1);
}
