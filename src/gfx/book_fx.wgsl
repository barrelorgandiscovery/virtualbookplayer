// Colored quads for book notes + soft particles.
// Vertex positions are already in clip space.

struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_note(in: VsOut) -> @location(0) vec4<f32> {
    // Book-space UV scrolls with the roll offset (passed from CPU).
    // Procedural grain/stripes give a moving texture as the carton advances.
    let stripe = sin(in.uv.x * 48.0 + in.uv.y * 6.0) * 0.5 + 0.5;
    let grain = sin(in.uv.x * 130.0) * sin(in.uv.y * 95.0) * 0.10;
    let shade = 0.82 + stripe * 0.18 + grain;

    // Soften only the geometric edges of the hole (local 0..1 still in .zw via unused — use frac for edge)
    // Local edge: CPU packs local UV in color.a unused… use sharp rect; shade only.
    let c = in.color.rgb * shade;
    let a = in.color.a;
    return vec4<f32>(c * a, a);
}

@fragment
fn fs_particle(in: VsOut) -> @location(0) vec4<f32> {
    // Soft radial glow sprite.
    let d = length(in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
    let soft = clamp(1.0 - d, 0.0, 1.0);
    let glow = soft * soft;
    let a = in.color.a * glow;
    return vec4<f32>(in.color.rgb * a, a);
}
