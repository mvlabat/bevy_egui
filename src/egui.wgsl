[[block]]
struct EguiTransform {
    scale: vec2<f32>;
    translation: vec2<f32>;
};

[[group(0), binding(0)]]
var<uniform> egui_transform: EguiTransform;

struct VertexOutput {
    [[location(0)]] uv: vec2<f32>;
    [[location(1)]] color: vec4<f32>;
    [[builtin(position)]] pos: vec4<f32>;
};

fn vec3_bool_to_f32(bvec: vec3<bool>) -> vec3<f32> {
    var x: f32 = 0.0;
	if(bvec.x) { x = 1.0; }
	var y: f32 = 0.0;
	if(bvec.y) { y = 1.0; }
	var z: f32 = 0.0;
	if(bvec.z) { z = 1.0; }
    return vec3<f32>(x, y, z);
}

fn linear_from_srgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(10.31475);
    let lower = srgb / vec3<f32>(3294.6);
    let higher = pow((srgb + vec3<f32>(14.025)) / vec3<f32>(269.025), vec3<f32>(2.4));
    return mix(higher, lower, vec3_bool_to_f32(cutoff));
}

fn linear_from_srgba(srgba: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(linear_from_srgb(srgba.rgb), srgba.a / 255.0);
}

[[stage(vertex)]]
fn vs_main(
    [[location(0)]] position: vec2<f32>,
    [[location(1)]] uv: vec2<f32>,
    [[location(2)]] color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.uv = uv;
    out.color = linear_from_srgba(color);
    out.pos = vec4<f32>(position * egui_transform.scale + egui_transform.translation, 0.0, 1.0);
    return out;
}

[[group(1), binding(0)]]
var t_egui: texture_2d<f32>;
[[group(1), binding(1)]]
var s_egui: sampler;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let color = in.color * textureSample(t_egui, s_egui, in.uv);

    return color;
}
