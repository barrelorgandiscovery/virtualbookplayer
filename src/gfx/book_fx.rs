//! WGPU paint callback for the virtual-book roll: instanced note quads +
//! soft particles spawned when a hole crosses the playhead.

use std::sync::Arc;

use eframe::egui_wgpu::{self, wgpu};
use egui::{Color32, Pos2, Rect};
use log::{info, warn};

const MAX_PARTICLES: usize = 1024;
const VERTS_PER_QUAD: usize = 6;
const INITIAL_QUAD_CAPACITY: usize = 2048;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[derive(Clone, Copy)]
struct Particle {
    pos: Pos2,
    vel: egui::Vec2,
    color: [f32; 4],
    life: f32,
    max_life: f32,
    size: f32,
}

/// CPU-side FX state that persists across frames (particles, playhead tracking).
pub struct BookFxState {
    particles: Vec<Particle>,
    last_playhead_us: Option<i64>,
    book_generation: u64,
    rng_state: u32,
    /// True after GPU pipelines were registered in egui-wgpu callback resources.
    gpu_ready: bool,
}

impl Default for BookFxState {
    fn default() -> Self {
        Self {
            particles: Vec::with_capacity(256),
            last_playhead_us: None,
            book_generation: 0,
            rng_state: 0xA5A5_1234,
            gpu_ready: false,
        }
    }
}

impl BookFxState {
    pub fn set_gpu_ready(&mut self, ready: bool) {
        self.gpu_ready = ready;
    }

    pub fn gpu_ready(&self) -> bool {
        self.gpu_ready
    }

    pub fn has_alive_particles(&self) -> bool {
        !self.particles.is_empty()
    }

    /// Call when the loaded book changes so note-start detection resets.
    pub fn on_book_changed(&mut self) {
        self.book_generation = self.book_generation.wrapping_add(1);
        self.last_playhead_us = None;
        self.particles.clear();
    }

    fn next_rand_f32(&mut self) -> f32 {
        // xorshift32
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = if x == 0 { 0xA5A5_1234 } else { x };
        (self.rng_state as f32) / (u32::MAX as f32)
    }

    fn spawn_burst(&mut self, at: Pos2, base: Color32) {
        if self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let count = 10;
        let room = MAX_PARTICLES - self.particles.len();
        let n = count.min(room);
        let (r, g, b, _) = base.to_tuple();
        let mut new_particles = Vec::with_capacity(n);
        for _ in 0..n {
            // Keep sparks near the blue bar: mostly vertical burst, little X drift.
            let angle = -std::f32::consts::FRAC_PI_2
                + (self.next_rand_f32() - 0.5) * 0.9;
            let speed = 30.0 + self.next_rand_f32() * 90.0;
            let life = 0.28 + self.next_rand_f32() * 0.40;
            let boost = 0.55 + self.next_rand_f32() * 0.45;
            let size = 8.0 + self.next_rand_f32() * 12.0;
            new_particles.push(Particle {
                pos: at,
                vel: egui::vec2(angle.cos() * speed * 0.25, angle.sin() * speed),
                color: [
                    (r as f32 / 255.0) * boost + 0.2,
                    (g as f32 / 255.0) * boost + 0.05,
                    (b as f32 / 255.0) * boost,
                    0.95,
                ],
                life,
                max_life: life,
                size,
            });
        }
        self.particles.extend(new_particles);
    }

    /// Advance particles and detect holes that crossed the playhead since last frame.
    pub fn update_play(
        &mut self,
        dt: f32,
        playing: bool,
        playhead_us: i64,
        note_starts: &[(i64, Pos2, Color32)],
    ) {
        if !playing {
            self.last_playhead_us = Some(playhead_us);
            // Still age particles so they fade out after stop.
        } else {
            match self.last_playhead_us {
                Some(prev) if playhead_us >= prev => {
                    // Spawn for holes whose start lies in (prev, playhead].
                    for &(ts, pos, color) in note_starts {
                        if ts > prev && ts <= playhead_us {
                            self.spawn_burst(pos, color);
                        }
                    }
                }
                _ => {
                    // Seek / first frame: don't explode every on-screen note.
                }
            }
            self.last_playhead_us = Some(playhead_us);
        }

        let dt = dt.clamp(0.0, 0.05);
        self.particles.retain_mut(|p| {
            p.life -= dt;
            if p.life <= 0.0 {
                return false;
            }
            p.vel.y += 90.0 * dt; // gravity-ish
            p.pos += p.vel * dt;
            p.vel *= 0.98;
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            p.color[3] = t * t;
            true
        });
    }
}

/// GPU resources shared across all book paint callbacks (stored in egui-wgpu).
struct BookFxGpu {
    note_pipeline: wgpu::RenderPipeline,
    particle_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
}

impl BookFxGpu {
    fn ensure_vertex_capacity(&mut self, device: &wgpu::Device, needed_verts: usize) {
        if needed_verts <= self.vertex_capacity {
            return;
        }
        let mut cap = self.vertex_capacity.max(INITIAL_QUAD_CAPACITY * VERTS_PER_QUAD);
        while cap < needed_verts {
            cap *= 2;
        }
        self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("book_fx_vertices"),
            size: (cap * std::mem::size_of::<GpuVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.vertex_capacity = cap;
    }
}

/// Paint payload for one frame of the book canvas.
pub(crate) struct BookFxCallback {
    pub(crate) note_verts: Arc<Vec<GpuVertex>>,
    pub(crate) particle_verts: Arc<Vec<GpuVertex>>,
}

impl egui_wgpu::CallbackTrait for BookFxCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<BookFxGpu>() else {
            return Vec::new();
        };

        let total = self.note_verts.len() + self.particle_verts.len();
        if total == 0 {
            return Vec::new();
        }

        gpu.ensure_vertex_capacity(device, total);

        queue.write_buffer(
            &gpu.vertex_buffer,
            0,
            bytemuck::cast_slice(self.note_verts.as_slice()),
        );
        if !self.particle_verts.is_empty() {
            let offset = (self.note_verts.len() * std::mem::size_of::<GpuVertex>()) as u64;
            queue.write_buffer(
                &gpu.vertex_buffer,
                offset,
                bytemuck::cast_slice(self.particle_verts.as_slice()),
            );
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(gpu) = resources.get::<BookFxGpu>() else {
            return;
        };

        if !self.note_verts.is_empty() {
            render_pass.set_pipeline(&gpu.note_pipeline);
            render_pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
            render_pass.draw(0..self.note_verts.len() as u32, 0..1);
        }

        if !self.particle_verts.is_empty() {
            let start = self.note_verts.len() as u32;
            let end = start + self.particle_verts.len() as u32;
            render_pass.set_pipeline(&gpu.particle_pipeline);
            render_pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
            render_pass.draw(start..end, 0..1);
        }
    }
}

/// Register GPU pipelines with eframe's wgpu renderer. Returns whether FX is available.
pub fn init_book_fx_gpu(cc: &eframe::CreationContext<'_>) -> bool {
    let Some(wgpu_render_state) = cc.wgpu_render_state.as_ref() else {
        warn!("No wgpu render state — virtual book will use CPU egui shapes");
        return false;
    };

    let device = &wgpu_render_state.device;
    let target_format = wgpu_render_state.target_format;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("book_fx"),
        source: wgpu::ShaderSource::Wgsl(include_str!("book_fx.wgsl").into()),
    });

    const VERTEX_ATTRS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
    let vertex_buffer_layouts = [wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRS,
    }];

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("book_fx_pipeline_layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    let note_blend = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;
    let particle_blend = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    };

    let make_pipeline = |label: &str, fs: &str, blend: wgpu::BlendState| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(vertex_buffer_layouts[0].clone())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };

    let note_pipeline = make_pipeline("book_fx_notes", "fs_note", note_blend);
    let particle_pipeline = make_pipeline("book_fx_particles", "fs_particle", particle_blend);

    let vertex_capacity = INITIAL_QUAD_CAPACITY * VERTS_PER_QUAD;
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("book_fx_vertices"),
        size: (vertex_capacity * std::mem::size_of::<GpuVertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    wgpu_render_state
        .renderer
        .write()
        .callback_resources
        .insert(BookFxGpu {
            note_pipeline,
            particle_pipeline,
            vertex_buffer,
            vertex_capacity,
        });

    info!("Virtual book GPU FX pipelines ready ({target_format:?})");
    true
}

// —— geometry helpers used by VirtualBookComponent ————————————————

fn color32_to_rgba(c: Color32) -> [f32; 4] {
    let (r, g, b, a) = c.to_tuple();
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

/// Map an egui point into the paint-callback NDC space.
///
/// egui-wgpu sets the wgpu viewport to [`PaintCallbackInfo::viewport`]; that
/// rect's edges are exactly clip-space ±1. Using the full window size here
/// shifts GPU notes/FX relative to egui-drawn shapes (the blue bar).
fn points_to_clip(pos: Pos2, viewport: Rect) -> [f32; 2] {
    let w = viewport.width().max(1.0);
    let h = viewport.height().max(1.0);
    [
        ((pos.x - viewport.min.x) / w) * 2.0 - 1.0,
        1.0 - ((pos.y - viewport.min.y) / h) * 2.0,
    ]
}

fn push_quad(out: &mut Vec<GpuVertex>, rect: Rect, color: Color32, uv: Rect, viewport: Rect) {
    let c = color32_to_rgba(color);
    let color = [c[0], c[1], c[2], c[3]];
    let tl = points_to_clip(rect.left_top(), viewport);
    let tr = points_to_clip(rect.right_top(), viewport);
    let br = points_to_clip(rect.right_bottom(), viewport);
    let bl = points_to_clip(rect.left_bottom(), viewport);

    let verts = [
        (tl, [uv.min.x, uv.min.y]),
        (tr, [uv.max.x, uv.min.y]),
        (br, [uv.max.x, uv.max.y]),
        (tl, [uv.min.x, uv.min.y]),
        (br, [uv.max.x, uv.max.y]),
        (bl, [uv.min.x, uv.max.y]),
    ];
    for (pos, uv) in verts {
        out.push(GpuVertex {
            position: pos,
            uv,
            color,
        });
    }
}

/// One note rectangle with book-space UVs (scroll with the carton) and playhead hit.
pub(crate) struct NoteDraw {
    pub rect: Rect,
    pub color: Color32,
    pub book_uv: Rect,
    pub active: bool,
    /// Screen-space point on the playhead where this note is read (for FX).
    pub playhead_hit: Option<Pos2>,
}

/// Build GPU vertex lists for notes + particles for this frame.
///
/// `viewport` must be the same rect passed to [`paint_book_fx`] (the paint
/// callback rect whose edges are clip-space ±1).
pub(crate) fn build_frame_meshes(
    notes_in: &[NoteDraw],
    fx: &BookFxState,
    viewport: Rect,
) -> (Vec<GpuVertex>, Vec<GpuVertex>) {
    let mut notes = Vec::with_capacity(notes_in.len() * VERTS_PER_QUAD);
    let mut particles = Vec::with_capacity(fx.particles.len() * VERTS_PER_QUAD + 8);

    for note in notes_in {
        push_quad(&mut notes, note.rect, note.color, note.book_uv, viewport);

        // Thin additive focus centered on the blue bar.
        if let Some(hit) = note.playhead_hit {
            let focus = Rect::from_center_size(
                hit,
                egui::vec2(4.0, (note.rect.height() + 4.0).max(8.0)),
            );
            push_quad(
                &mut particles,
                focus,
                Color32::from_rgba_unmultiplied(255, 140, 50, 180),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                viewport,
            );
        }
    }

    for p in &fx.particles {
        let half = p.size * 0.5;
        let rect = Rect::from_center_size(p.pos, egui::vec2(p.size.max(half), p.size.max(half)));
        let color = Color32::from_rgba_unmultiplied(
            (p.color[0] * 255.0).clamp(0.0, 255.0) as u8,
            (p.color[1] * 255.0).clamp(0.0, 255.0) as u8,
            (p.color[2] * 255.0).clamp(0.0, 255.0) as u8,
            (p.color[3] * 255.0).clamp(0.0, 255.0) as u8,
        );
        push_quad(
            &mut particles,
            rect,
            color,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            viewport,
        );
    }

    (notes, particles)
}

pub(crate) fn paint_book_fx(
    painter: &egui::Painter,
    rect: Rect,
    note_verts: Vec<GpuVertex>,
    particle_verts: Vec<GpuVertex>,
) {
    painter.add(egui_wgpu::Callback::new_paint_callback(
        rect,
        BookFxCallback {
            note_verts: Arc::new(note_verts),
            particle_verts: Arc::new(particle_verts),
        },
    ));
}
