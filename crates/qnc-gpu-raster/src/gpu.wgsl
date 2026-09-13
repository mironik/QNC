// Plane layout and color policy are prepared from saved metadata.
struct Spec {
    sw: u32, sh: u32, cw: u32, ch: u32,
    dw: u32, dh: u32, depth: u32, limited: u32,
    transfer: u32, u_start: u32, v_start: u32, reserved: u32,
}
@group(0) @binding(0) var<uniform> spec: Spec;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> output_pixels: array<u32>;

fn sample_at(start: u32, size: vec2<u32>, pos: vec2<i32>) -> f32 {
    let xy = vec2<u32>(clamp(pos, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1)));
    let sample_index = start + xy.y * size.x + xy.x;
    let byte_index = sample_index * (spec.depth / 8u);
    let word = source[byte_index / 4u];
    let mask = select(255u, 65535u, spec.depth == 16u);
    return f32((word >> ((byte_index % 4u) * 8u)) & mask);
}

fn plane(start: u32, size: vec2<u32>, dest: vec2<u32>) -> f32 {
    // Integer decimation, no vertical blend. 1080p50 -> 540 must keep even
    // lines together; bilinear 2:1 averages adjacent lines and looks like a
    // wrong field weave on progressive (and on PsF) content.
    let sx = (dest.x * size.x) / spec.dw;
    let sy = (dest.y * size.y) / spec.dh;
    return sample_at(start, size, vec2<i32>(i32(sx), i32(sy)));
}

fn to_srgb(value: f32) -> f32 {
    let encoded = clamp(value, 0.0, 1.0);
    if spec.transfer == 0u { return encoded; }
    var linear = encoded / 4.5;
    if encoded >= 0.081 { linear = pow((encoded + 0.099) / 1.099, 1.0 / 0.45); }
    if linear <= 0.0031308 { return linear * 12.92; }
    return 1.055 * pow(linear, 1.0 / 2.4) - 0.055;
}

@compute @workgroup_size(8, 8)
fn convert(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= spec.dw || id.y >= spec.dh { return; }
    let dest = id.xy;
    let scale = select(1.0, 4.0, spec.depth == 16u);
    let maximum = select(255.0, 1023.0, spec.depth == 16u);
    let y_bias = select(0.0, 16.0 * scale, spec.limited == 1u);
    let y_range = select(maximum, 219.0 * scale, spec.limited == 1u);
    let c_range = select(maximum, 224.0 * scale, spec.limited == 1u);
    let y = (plane(0u, vec2<u32>(spec.sw, spec.sh), dest) - y_bias) / y_range;
    let u = (plane(spec.u_start, vec2<u32>(spec.cw, spec.ch), dest) - 128.0 * scale) / c_range;
    let v = (plane(spec.v_start, vec2<u32>(spec.cw, spec.ch), dest) - 128.0 * scale) / c_range;
    let kr = 0.2126;
    let kb = 0.0722;
    let kg = 1.0 - kr - kb;
    let r = y + 2.0 * (1.0 - kr) * v;
    let b = y + 2.0 * (1.0 - kb) * u;
    let g = y - 2.0 * kb * (1.0 - kb) / kg * u - 2.0 * kr * (1.0 - kr) / kg * v;
    output_pixels[id.y * spec.dw + id.x] = pack4x8unorm(vec4<f32>(to_srgb(r), to_srgb(g), to_srgb(b), 1.0));
}
