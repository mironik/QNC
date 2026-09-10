@group(0) @binding(0) var pixels: texture_2d<f32>;
@group(0) @binding(1) var pixel_sampler: sampler;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var vertex: Vertex;
    vertex.position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    vertex.uv = uv;
    return vertex;
}

@fragment fn fs_main(vertex: Vertex) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(pixels, pixel_sampler, vertex.uv).rgb, 1.0);
}
