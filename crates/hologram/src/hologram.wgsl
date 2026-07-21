// The holographic composite pass: takes the egui-rendered UI as a texture and
// applies the five effects from docs/decision-records/0007-holographic-wgpu-shell.md
// and docs/UI-DESIGN-BRIEF.md §8, as a full-screen post-process over the UI frame.

struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var ui_texture: texture_2d<f32>;
@group(0) @binding(2) var ui_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Full-screen triangle — avoids a vertex/index buffer for a single quad pass.
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[vertex_index];
    var out: VertexOutput;
    out.clip_position = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>((p.x + 1.0) * 0.5, 1.0 - (p.y + 1.0) * 0.5);
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = uniforms.time;
    let uv = in.uv;
    let resolution = vec2<f32>(uniforms.width, uniforms.height);

    // 1. Chromatic aberration — per-channel UV shift driven by a sine-wave time offset.
    let aberration = 0.0025 * (0.5 + 0.5 * sin(t * 0.7));
    let shift = vec2<f32>(aberration, 0.0);
    let r = textureSample(ui_texture, ui_sampler, uv + shift).r;
    let g = textureSample(ui_texture, ui_sampler, uv).g;
    let b = textureSample(ui_texture, ui_sampler, uv - shift).b;
    let a = textureSample(ui_texture, ui_sampler, uv).a;

    // 2. Procedural glitch/noise — vertical coordinate offsets mapped to an erratic
    // time frequency; rare per-row displacement plus a constant low-level grain.
    let row = floor(uv.y * 220.0);
    let glitch_gate = step(0.985, hash(vec2<f32>(row, floor(t * 12.0))));
    let glitch_offset = (hash(vec2<f32>(row, t)) - 0.5) * 0.02 * glitch_gate;
    let grain = (hash(uv * resolution + t) - 0.5) * 0.03;

    var color = vec3<f32>(r, g, b) + grain;

    // 3. Moving scanlines — horizontal brightness wave over clip-space Y, dense and
    // translated by time.
    let scanline = 0.9 + 0.1 * sin(uv.y * resolution.y * 1.5 - t * 6.0);
    color = color * scanline;

    // 4. Fresnel glow rim — this pass composites a flat 2D UI quad rather than 3D
    // geometry, so distance-from-center stands in for a silhouette normal/view-angle
    // term: panels glow brighter toward the edges, dim at the center, same as a rim
    // light would on curved geometry facing away from the camera.
    let center_dist = distance(uv, vec2<f32>(0.5, 0.5));
    let fresnel = pow(clamp(center_dist * 1.4, 0.0, 1.0), 2.0);
    let rim_color = vec3<f32>(0.4, 0.9, 1.0);
    color = color + rim_color * fresnel * 0.35;

    // 5. Ambient screen flicker — combined high-frequency + slow macro oscillation,
    // emulating plasma instability.
    let flicker = 0.95 + 0.05 * sin(t * 37.0) * sin(t * 5.3 + 1.7);
    color = color * flicker;

    return vec4<f32>(color + glitch_offset, a);
}
