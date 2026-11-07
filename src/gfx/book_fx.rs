//! WGPU paint callback for the virtual-book roll: instanced note quads +
//! soft particles spawned when a hole crosses the playhead.

use std::sync::Arc;

use eframe::egui_wgpu::{self, wgpu};
use egui::{Color32, Pos2, Rect};
use log::{info, warn};

use super::effect::{BookEffectKind, ParticleDrawStyle};

const MAX_PARTICLES: usize = 1024;
const VERTS_PER_QUAD: usize = 6;
const INITIAL_QUAD_CAPACITY: usize = 2048;
/// Reference hole length (µs) for a "medium" note — scales bursts around this.
const REF_NOTE_LENGTH_US: f32 = 350_000.0;

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
    base_alpha: f32,
    life: f32,
    max_life: f32,
    size: f32,
    /// Downward acceleration (points/s²). Sparkle uses stronger gravity.
    gravity: f32,
    /// Horizontal drag / air resistance factor per frame scale.
    drag: f32,
    /// Current orientation (radians).
    rotation: f32,
    /// Angular velocity (radians/s).
    spin: f32,
    /// Size growth rate (points/s) — used by Ripple rings.
    size_grow: f32,
}

/// CPU-side FX state that persists across frames (particles, playhead tracking).
pub struct BookFxState {
    particles: Vec<Particle>,
    last_playhead_us: Option<i64>,
    book_generation: u64,
    rng_state: u32,
    /// True after GPU pipelines were registered in egui-wgpu callback resources.
    gpu_ready: bool,
    /// Active effect from preferences.
    effect: BookEffectKind,
    /// User scale for effect impact (count, size, radius, strength). 1.0 = default.
    effect_scale: f32,
}

impl Default for BookFxState {
    fn default() -> Self {
        Self {
            particles: Vec::with_capacity(256),
            last_playhead_us: None,
            book_generation: 0,
            rng_state: 0xA5A5_1234,
            gpu_ready: false,
            effect: BookEffectKind::None,
            effect_scale: 1.0,
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

    pub fn effect(&self) -> BookEffectKind {
        self.effect
    }

    /// Apply the preference selection; clears particles when leaving a particle effect.
    pub fn set_effect(&mut self, effect: BookEffectKind) {
        if self.effect != effect {
            self.particles.clear();
            self.last_playhead_us = None;
            self.effect = effect;
        }
    }

    pub fn set_effect_scale(&mut self, scale: f32) {
        self.effect_scale = scale.clamp(0.0, 3.0);
    }

    pub fn effect_scale(&self) -> f32 {
        self.effect_scale
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

    /// Soft scale from hole length: short notes stay crisp, long ones bloom more.
    fn length_weight(length_us: i64) -> f32 {
        let ratio = (length_us.max(1) as f32 / REF_NOTE_LENGTH_US).sqrt();
        ratio.clamp(0.35, 2.6)
    }

    /// Length weight × user effect scale (strength / radius / count).
    fn impact(&self, length_us: i64) -> f32 {
        Self::length_weight(length_us) * self.effect_scale.max(0.0)
    }

    fn scale_count(&self, base: f32) -> usize {
        (base * self.effect_scale.max(0.0)).round().clamp(0.0, 64.0) as usize
    }

    fn spawn_fire_particles(
        &mut self,
        at: Pos2,
        base: Color32,
        count: usize,
        size_mul: f32,
        life_mul: f32,
        speed_mul: f32,
        alpha: f32,
    ) {
        if count == 0 || self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let room = MAX_PARTICLES - self.particles.len();
        let n = count.min(room);
        let (r, g, b, _) = base.to_tuple();
        let mut new_particles = Vec::with_capacity(n);
        for _ in 0..n {
            // Warm bloom: mostly upward, little X drift.
            let angle = -std::f32::consts::FRAC_PI_2 + (self.next_rand_f32() - 0.5) * 0.9;
            let speed = (25.0 + self.next_rand_f32() * 85.0) * speed_mul;
            let life = (0.22 + self.next_rand_f32() * 0.38) * life_mul;
            let boost = 0.55 + self.next_rand_f32() * 0.45;
            let size = (6.0 + self.next_rand_f32() * 11.0) * size_mul;
            new_particles.push(Particle {
                pos: at,
                vel: egui::vec2(angle.cos() * speed * 0.25, angle.sin() * speed),
                color: [
                    (r as f32 / 255.0) * boost + 0.2,
                    (g as f32 / 255.0) * boost + 0.05,
                    (b as f32 / 255.0) * boost,
                    alpha,
                ],
                base_alpha: alpha,
                life,
                max_life: life,
                size,
                gravity: 90.0,
                drag: 0.98,
                rotation: 0.0,
                spin: 0.0,
                size_grow: 0.0,
            });
        }
        self.particles.extend(new_particles);
    }

    /// Faceted gold star sprites falling under gravity; `intensity` scales count/size/speed.
    /// Soft white fog is drawn each frame locked to each star's pose.
    fn spawn_sparkle_particles(&mut self, at: Pos2, count: usize, intensity: f32) {
        if count == 0 || self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let room = MAX_PARTICLES - self.particles.len();
        let n = count.min(room);
        if n == 0 {
            return;
        }
        let mut new_particles = Vec::with_capacity(n);
        for _ in 0..n {
            let vx = (self.next_rand_f32() - 0.5) * 50.0 * intensity;
            // Small upward jump first (egui Y grows downward → negative vy),
            // then gravity pulls the sparkle into a fall.
            let jump = -(55.0 + self.next_rand_f32() * (70.0 + 40.0 * intensity));
            let life = 0.55 + self.next_rand_f32() * (0.55 + 0.35 * intensity);
            // Compact textured star sprites.
            let size = (5.0 + self.next_rand_f32() * 4.5) * (0.85 + 0.35 * intensity);
            // Push the gold atlas toward a brighter yellow.
            let r = 1.0;
            let g = 0.95 + 0.05 * self.next_rand_f32();
            let b = 0.35 + 0.15 * self.next_rand_f32();
            let pos = Pos2::new(
                at.x + (self.next_rand_f32() - 0.5) * 4.0,
                at.y + (self.next_rand_f32() - 0.5) * 2.0,
            );

            // Random start angle + tumble while falling.
            // Fog is drawn each frame at this particle's pose (see build_frame_meshes).
            let rotation = self.next_rand_f32() * std::f32::consts::TAU;
            let spin = (self.next_rand_f32() - 0.5) * (8.0 + 6.0 * intensity);
            new_particles.push(Particle {
                pos,
                vel: egui::vec2(vx, jump),
                color: [r, g, b, 1.0],
                base_alpha: 1.0,
                life,
                max_life: life,
                size,
                gravity: 320.0 + 140.0 * intensity,
                drag: 0.994,
                rotation,
                spin,
                size_grow: 0.0,
            });
        }
        self.particles.extend(new_particles);
    }

    /// Piano-viz expanding ripples from the hit point.
    fn spawn_ripple(&mut self, at: Pos2, intensity: f32) {
        if self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let s = self.effect_scale.max(0.0);
        let rings = (1 + (intensity * 1.5) as usize)
            .max(1)
            .min((3.0 * s).ceil() as usize)
            .min(5);
        for i in 0..rings {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }
            let delay = i as f32 * 0.06;
            let life = 0.55 + 0.25 * intensity - delay * 0.3;
            let tint = self.next_rand_f32();
            self.particles.push(Particle {
                pos: at,
                vel: egui::vec2(0.0, 0.0),
                color: [0.35 + 0.25 * tint, 0.75 + 0.2 * tint, 1.0, 0.85],
                base_alpha: (0.55 + 0.25 * s).min(1.0),
                life: life.max(0.25),
                max_life: life.max(0.25),
                size: (10.0 + i as f32 * 6.0) * s.sqrt().max(0.35),
                gravity: 0.0,
                drag: 1.0,
                rotation: 0.0,
                spin: 0.0,
                size_grow: (140.0 + 80.0 * intensity) * s,
            });
        }
    }

    /// Rhythm-game neon sparks (beams are drawn in build_frame_meshes).
    fn spawn_neon_sparks(&mut self, at: Pos2, intensity: f32, count: usize) {
        if count == 0 || self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let n = count.min(MAX_PARTICLES - self.particles.len());
        for _ in 0..n {
            let up = -(40.0 + self.next_rand_f32() * 90.0 * intensity);
            let side = (self.next_rand_f32() - 0.5) * 40.0;
            let magenta = self.next_rand_f32() > 0.5;
            let (r, g, b) = if magenta {
                (1.0, 0.25 + 0.3 * self.next_rand_f32(), 0.85)
            } else {
                (0.15, 0.9, 1.0)
            };
            let life = 0.25 + self.next_rand_f32() * 0.35;
            let size = (5.0 + self.next_rand_f32() * 7.0)
                * self.effect_scale.sqrt().max(0.35)
                * (0.85 + 0.25 * intensity.min(1.5));
            self.particles.push(Particle {
                pos: at,
                vel: egui::vec2(side, up),
                color: [r, g, b, 0.9],
                base_alpha: (0.7 + 0.2 * self.effect_scale).min(1.0),
                life,
                max_life: life,
                size,
                gravity: 60.0,
                drag: 0.97,
                rotation: 0.0,
                spin: 0.0,
                size_grow: 0.0,
            });
        }
    }

    /// Arcade confetti / prism shards bursting outward.
    fn spawn_arcade_burst(&mut self, at: Pos2, intensity: f32, count: usize) {
        if count == 0 || self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let n = count.min(MAX_PARTICLES - self.particles.len());
        const PALETTE: [[f32; 3]; 6] = [
            [1.0, 0.25, 0.45],
            [1.0, 0.85, 0.2],
            [0.3, 1.0, 0.55],
            [0.35, 0.7, 1.0],
            [0.85, 0.4, 1.0],
            [1.0, 1.0, 1.0],
        ];
        for _ in 0..n {
            let angle = self.next_rand_f32() * std::f32::consts::TAU;
            let speed = 80.0 + self.next_rand_f32() * (160.0 * intensity);
            let life = 0.4 + self.next_rand_f32() * 0.55;
            let c = PALETTE[(self.next_rand_f32() * PALETTE.len() as f32) as usize % PALETTE.len()];
            let size =
                (4.0 + self.next_rand_f32() * 6.0 * intensity) * self.effect_scale.sqrt().max(0.35);
            let rotation = self.next_rand_f32() * std::f32::consts::TAU;
            let spin = (self.next_rand_f32() - 0.5) * 14.0;
            self.particles.push(Particle {
                pos: at,
                vel: egui::vec2(angle.cos() * speed, angle.sin() * speed * 0.85 - 30.0),
                color: [c[0], c[1], c[2], 1.0],
                base_alpha: (0.75 + 0.25 * self.effect_scale).min(1.0),
                life,
                max_life: life,
                size,
                gravity: 180.0,
                drag: 0.98,
                rotation,
                spin,
                size_grow: 0.0,
            });
        }
    }

    /// Attack spark when the hole's leading edge crosses the playhead.
    fn spawn_note_attack(&mut self, at: Pos2, base: Color32, length_us: i64) {
        if self.effect_scale <= 0.001 {
            return;
        }
        let w = self.impact(length_us);
        match self.effect {
            BookEffectKind::NoteFire => {
                let count = self.scale_count(6.0 * w).clamp(1, 40);
                self.spawn_fire_particles(
                    at,
                    base,
                    count,
                    (0.85 + 0.35 * w) * self.effect_scale.sqrt(),
                    0.85 + 0.4 * w,
                    w,
                    0.95,
                );
            }
            BookEffectKind::Sparkle => {
                let count = self.scale_count(6.0 * w).clamp(1, 48);
                self.spawn_sparkle_particles(at, count, w);
            }
            BookEffectKind::Ripple => {
                self.spawn_ripple(at, w);
            }
            BookEffectKind::NeonLane => {
                let count = self.scale_count(5.0 * w).clamp(1, 24);
                self.spawn_neon_sparks(at, w, count);
            }
            BookEffectKind::ArcadeBurst => {
                let count = self.scale_count(14.0 * w).clamp(2, 56);
                self.spawn_arcade_burst(at, w, count);
            }
            BookEffectKind::None => {}
        }
    }

    /// Soft trail while the playhead sits inside a hole — denser for longer notes.
    fn spawn_note_sustain(&mut self, at: Pos2, base: Color32, length_us: i64, dt: f32) {
        if self.effect_scale <= 0.001 {
            return;
        }
        let w = self.impact(length_us);
        let s = self.effect_scale.max(0.0);
        match self.effect {
            BookEffectKind::NoteFire => {
                let rate = 4.0 * w * w;
                let expected = rate * dt;
                let mut count = expected.floor() as usize;
                if self.next_rand_f32() < expected.fract() {
                    count += 1;
                }
                if count == 0 {
                    return;
                }
                self.spawn_fire_particles(
                    at,
                    base,
                    count.min((4.0 * s).ceil() as usize).max(1),
                    (0.55 + 0.2 * w) * s.sqrt(),
                    0.7 + 0.25 * w,
                    0.55 + 0.2 * w,
                    0.55,
                );
            }
            BookEffectKind::Sparkle => {
                let rate = 10.0 * w * w;
                let expected = rate * dt;
                let mut count = expected.floor() as usize;
                if self.next_rand_f32() < expected.fract() {
                    count += 1;
                }
                if count == 0 {
                    return;
                }
                self.spawn_sparkle_particles(at, count.min((6.0 * s).ceil() as usize).max(1), w);
            }
            BookEffectKind::Ripple => {
                let rate = 1.2 * w;
                if self.next_rand_f32() < rate * dt {
                    self.spawn_ripple(at, w * 0.65);
                }
            }
            BookEffectKind::NeonLane => {
                let rate = 8.0 * w;
                let expected = rate * dt;
                let mut count = expected.floor() as usize;
                if self.next_rand_f32() < expected.fract() {
                    count += 1;
                }
                if count > 0 {
                    self.spawn_neon_sparks(at, w, count.min((3.0 * s).ceil() as usize).max(1));
                }
            }
            BookEffectKind::ArcadeBurst => {
                let rate = 6.0 * w;
                let expected = rate * dt;
                let mut count = expected.floor() as usize;
                if self.next_rand_f32() < expected.fract() {
                    count += 1;
                }
                if count > 0 {
                    self.spawn_arcade_burst(
                        at,
                        w * 0.5,
                        count.min((4.0 * s).ceil() as usize).max(1),
                    );
                }
            }
            BookEffectKind::None => {}
        }
    }

    /// Advance particles and spawn FX for notes crossing / under the playhead.
    ///
    /// `notes` entries are `(start_us, length_us, hit_pos, color)`.
    /// `xscale` is µs per screen point — used so sparks scroll left with the carton.
    pub fn update_play(
        &mut self,
        dt: f32,
        playing: bool,
        playhead_us: i64,
        xscale: f64,
        notes: &[(i64, i64, Pos2, Color32)],
    ) {
        let dt = dt.clamp(0.0, 0.05);

        if !self.effect.emits_particles() {
            self.particles.clear();
            self.last_playhead_us = Some(playhead_us);
            return;
        }

        // Carton motion: as playhead time advances, paper (and sparks) move left.
        let scroll_dx = match self.last_playhead_us {
            Some(prev) if playhead_us >= prev && xscale > 0.0 => {
                -((playhead_us - prev) as f64 / xscale) as f32
            }
            _ => 0.0,
        };

        for p in &mut self.particles {
            p.pos.x += scroll_dx;
        }

        if playing {
            match self.last_playhead_us {
                Some(prev) if playhead_us >= prev => {
                    for &(ts, length_us, pos, color) in notes {
                        let end = ts.saturating_add(length_us);
                        // Attack: leading edge crossed since last frame.
                        if ts > prev && ts <= playhead_us {
                            self.spawn_note_attack(pos, color, length_us);
                        }
                        // Sustain: playhead currently inside the hole.
                        if playhead_us > ts && playhead_us <= end {
                            self.spawn_note_sustain(pos, color, length_us, dt);
                        }
                    }
                }
                _ => {
                    // Seek / first frame: don't explode every on-screen note.
                }
            }
        }
        self.last_playhead_us = Some(playhead_us);

        self.particles.retain_mut(|p| {
            p.life -= dt;
            if p.life <= 0.0 {
                return false;
            }
            // Local motion on top of book scroll (gravity + drag + spin + grow).
            p.vel.y += p.gravity * dt;
            p.pos += p.vel * dt;
            p.vel *= p.drag;
            p.rotation += p.spin * dt;
            p.size += p.size_grow * dt;
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            p.color[3] = p.base_alpha * t * t;
            true
        });
    }
}

/// GPU resources shared across all book paint callbacks (stored in egui-wgpu).
struct BookFxGpu {
    note_pipeline: wgpu::RenderPipeline,
    /// Soft additive glow (Note fire / Neon / Arcade).
    particle_pipeline: wgpu::RenderPipeline,
    /// Textured faceted-star sparks (Sparkle) — samples `sparkle_star.png`.
    sparkle_pipeline: wgpu::RenderPipeline,
    /// Keeps the star atlas alive for `sparkle_bind_group`.
    _sparkle_texture: wgpu::Texture,
    sparkle_bind_group: wgpu::BindGroup,
    /// Expanding rings (Ripple).
    ring_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
}

impl BookFxGpu {
    fn ensure_vertex_capacity(&mut self, device: &wgpu::Device, needed_verts: usize) {
        if needed_verts <= self.vertex_capacity {
            return;
        }
        let mut cap = self
            .vertex_capacity
            .max(INITIAL_QUAD_CAPACITY * VERTS_PER_QUAD);
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
    /// Soft white smoke (drawn before stars when sparkle_style).
    pub(crate) smoke_verts: Arc<Vec<GpuVertex>>,
    pub(crate) particle_verts: Arc<Vec<GpuVertex>>,
    pub(crate) draw_style: ParticleDrawStyle,
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

        let total = self.note_verts.len() + self.smoke_verts.len() + self.particle_verts.len();
        if total == 0 {
            return Vec::new();
        }

        gpu.ensure_vertex_capacity(device, total);

        let mut offset = 0u64;
        let stride = std::mem::size_of::<GpuVertex>() as u64;
        if !self.note_verts.is_empty() {
            queue.write_buffer(
                &gpu.vertex_buffer,
                offset,
                bytemuck::cast_slice(self.note_verts.as_slice()),
            );
            offset += self.note_verts.len() as u64 * stride;
        }
        if !self.smoke_verts.is_empty() {
            queue.write_buffer(
                &gpu.vertex_buffer,
                offset,
                bytemuck::cast_slice(self.smoke_verts.as_slice()),
            );
            offset += self.smoke_verts.len() as u64 * stride;
        }
        if !self.particle_verts.is_empty() {
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

        let mut cursor = 0u32;
        if !self.note_verts.is_empty() {
            let end = cursor + self.note_verts.len() as u32;
            render_pass.set_pipeline(&gpu.note_pipeline);
            render_pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
            render_pass.draw(cursor..end, 0..1);
            cursor = end;
        }

        // Smoke behind stars (soft additive white puffs).
        if !self.smoke_verts.is_empty() {
            let end = cursor + self.smoke_verts.len() as u32;
            render_pass.set_pipeline(&gpu.particle_pipeline);
            render_pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
            render_pass.draw(cursor..end, 0..1);
            cursor = end;
        }

        if !self.particle_verts.is_empty() {
            let end = cursor + self.particle_verts.len() as u32;
            match self.draw_style {
                ParticleDrawStyle::Star => {
                    render_pass.set_pipeline(&gpu.sparkle_pipeline);
                    render_pass.set_bind_group(0, &gpu.sparkle_bind_group, &[]);
                }
                ParticleDrawStyle::Ring => {
                    render_pass.set_pipeline(&gpu.ring_pipeline);
                }
                ParticleDrawStyle::Soft => {
                    render_pass.set_pipeline(&gpu.particle_pipeline);
                }
            }
            render_pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
            render_pass.draw(cursor..end, 0..1);
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

    let sparkle_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("book_fx_sparkle_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let sparkle_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("book_fx_sparkle_pipeline_layout"),
        bind_group_layouts: &[Some(&sparkle_bgl)],
        immediate_size: 0,
    });

    let note_blend = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;
    // Note fire: additive soft glow (warm / white-hot bloom on parchment).
    let fire_blend = wgpu::BlendState {
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
    // Sparkle: premultiplied alpha so the faceted gold sprite keeps its metal look.
    let sparkle_blend = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;

    let make_pipeline =
        |label: &str, fs: &str, blend: wgpu::BlendState, layout: &wgpu::PipelineLayout| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
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

    let note_pipeline = make_pipeline("book_fx_notes", "fs_note", note_blend, &pipeline_layout);
    let particle_pipeline = make_pipeline(
        "book_fx_particles",
        "fs_particle",
        fire_blend,
        &pipeline_layout,
    );
    let sparkle_pipeline = make_pipeline(
        "book_fx_sparkle",
        "fs_sparkle",
        sparkle_blend,
        &sparkle_pipeline_layout,
    );
    let ring_pipeline = make_pipeline("book_fx_ring", "fs_ring", fire_blend, &pipeline_layout);

    let (sparkle_texture, sparkle_view) =
        create_sparkle_star_texture(device, &wgpu_render_state.queue);
    let sparkle_samp = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("book_fx_sparkle_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let sparkle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("book_fx_sparkle_bg"),
        layout: &sparkle_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&sparkle_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sparkle_samp),
            },
        ],
    });

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
            sparkle_pipeline,
            _sparkle_texture: sparkle_texture,
            sparkle_bind_group,
            ring_pipeline,
            vertex_buffer,
            vertex_capacity,
        });

    info!("Virtual book GPU FX pipelines ready ({target_format:?})");
    true
}

/// Embed + upload the faceted gold star sprite used by the Sparkle effect.
fn create_sparkle_star_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    let bytes = include_bytes!("../../assets/sparkle_star.png");
    let img = image::load_from_memory(bytes)
        .expect("sparkle_star.png")
        .into_rgba8();
    let (width, height) = img.dimensions();
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("book_fx_sparkle_star"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &img,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
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
    push_quad_rotated(
        out,
        rect.center(),
        rect.width(),
        rect.height(),
        0.0,
        color,
        uv,
        viewport,
    );
}

fn push_quad_rotated(
    out: &mut Vec<GpuVertex>,
    center: Pos2,
    width: f32,
    height: f32,
    rotation: f32,
    color: Color32,
    uv: Rect,
    viewport: Rect,
) {
    let c = color32_to_rgba(color);
    let color = [c[0], c[1], c[2], c[3]];
    let hw = width * 0.5;
    let hh = height * 0.5;
    let (s, cos) = rotation.sin_cos();
    let rotate = |lx: f32, ly: f32| -> Pos2 {
        Pos2::new(center.x + lx * cos - ly * s, center.y + lx * s + ly * cos)
    };
    let tl = points_to_clip(rotate(-hw, -hh), viewport);
    let tr = points_to_clip(rotate(hw, -hh), viewport);
    let br = points_to_clip(rotate(hw, hh), viewport);
    let bl = points_to_clip(rotate(-hw, hh), viewport);

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
) -> (Vec<GpuVertex>, Vec<GpuVertex>, Vec<GpuVertex>) {
    let mut notes = Vec::with_capacity(notes_in.len() * VERTS_PER_QUAD);
    let mut smoke = Vec::with_capacity(fx.particles.len() * VERTS_PER_QUAD / 2);
    let mut particles = Vec::with_capacity(fx.particles.len() * VERTS_PER_QUAD + 8);

    let emit = fx.effect().emits_particles();

    for note in notes_in {
        push_quad(&mut notes, note.rect, note.color, note.book_uv, viewport);

        // Playhead focus / neon beams while a note is active.
        if emit {
            if let Some(hit) = note.playhead_hit {
                match fx.effect() {
                    BookEffectKind::NeonLane => {
                        // Tall neon beam (rhythm-game lane flash), scaled by impact.
                        let s = fx.effect_scale().max(0.25);
                        let beam_h = ((note.rect.height() + 28.0).max(36.0)) * s.sqrt();
                        let beam_w = 7.0 * s;
                        let glow_w = 18.0 * s;
                        let beam = Rect::from_center_size(hit, egui::vec2(beam_w, beam_h));
                        let glow =
                            Rect::from_center_size(hit, egui::vec2(glow_w, beam_h + 10.0 * s));
                        let glow_a = (50.0 * s).clamp(20.0, 120.0) as u8;
                        let beam_a = (200.0 * s.min(1.4)).clamp(80.0, 255.0) as u8;
                        push_quad(
                            &mut smoke,
                            glow,
                            Color32::from_rgba_unmultiplied(40, 200, 255, glow_a),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            viewport,
                        );
                        push_quad(
                            &mut particles,
                            beam,
                            Color32::from_rgba_unmultiplied(180, 80, 255, beam_a),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            viewport,
                        );
                    }
                    BookEffectKind::Sparkle
                    | BookEffectKind::Ripple
                    | BookEffectKind::ArcadeBurst
                    | BookEffectKind::NoteFire => {
                        let s = fx.effect_scale().max(0.25);
                        let w = match fx.effect() {
                            BookEffectKind::Ripple | BookEffectKind::ArcadeBurst => 5.0,
                            _ => 4.0,
                        } * s;
                        let h = ((note.rect.height() + 4.0).max(8.0)) * s.sqrt();
                        let focus = Rect::from_center_size(hit, egui::vec2(w, h));
                        let (r, g, b, a) = match fx.effect() {
                            BookEffectKind::Sparkle => (255, 220, 40, 150),
                            BookEffectKind::Ripple => (120, 210, 255, 140),
                            BookEffectKind::ArcadeBurst => (255, 220, 80, 160),
                            _ => (255, 140, 50, 180),
                        };
                        let a = ((a as f32) * s.min(1.4)).clamp(40.0, 255.0) as u8;
                        push_quad(
                            &mut particles,
                            focus,
                            Color32::from_rgba_unmultiplied(r, g, b, a),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            viewport,
                        );
                    }
                    BookEffectKind::None | BookEffectKind::NeonLane => {}
                }
            }
        }
    }

    for p in &fx.particles {
        let size = p.size.max(1.0);
        let life_t = (p.life / p.max_life).clamp(0.0, 1.0);
        let color = Color32::from_rgba_unmultiplied(
            (p.color[0] * 255.0).clamp(0.0, 255.0) as u8,
            (p.color[1] * 255.0).clamp(0.0, 255.0) as u8,
            (p.color[2] * 255.0).clamp(0.0, 255.0) as u8,
            (p.color[3] * 255.0).clamp(0.0, 255.0) as u8,
        );

        // White fog locked to the star pose (same position / spin, larger soft sprite).
        if fx.effect() == BookEffectKind::Sparkle {
            let fog_alpha = (0.20 * life_t * life_t * 255.0) as u8;
            let fog = Color32::from_rgba_unmultiplied(250, 252, 255, fog_alpha);
            push_quad_rotated(
                &mut smoke,
                p.pos,
                size * 3.2,
                size * 3.2,
                p.rotation * 0.35,
                fog,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                viewport,
            );
        }

        push_quad_rotated(
            &mut particles,
            p.pos,
            size,
            size,
            p.rotation,
            color,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            viewport,
        );
    }

    (notes, smoke, particles)
}

pub(crate) fn paint_book_fx(
    painter: &egui::Painter,
    rect: Rect,
    note_verts: Vec<GpuVertex>,
    smoke_verts: Vec<GpuVertex>,
    particle_verts: Vec<GpuVertex>,
    draw_style: ParticleDrawStyle,
) {
    painter.add(egui_wgpu::Callback::new_paint_callback(
        rect,
        BookFxCallback {
            note_verts: Arc::new(note_verts),
            smoke_verts: Arc::new(smoke_verts),
            particle_verts: Arc::new(particle_verts),
            draw_style,
        },
    ));
}
