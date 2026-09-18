//! The card stack (LLP 1014 D5, the second night demo): the northbound
//! departures as cards — each a direct child of the canvas, laid out by the
//! kernel and captured by the host on its own — placed by the surface in
//! depth: a deck when closed, a fan when open, the focused card forward.
//! Every placement is a 4×4 in canvas points with a little perspective; the
//! same mapping goes back to the host as a homography, so a tap on a fanned
//! card lands on that card. Poses move on critically damped springs over
//! the frame clock. Inputs, in `canvas surface=stack(board, focus, deck,
//! nowMs)` order: the board (records `[id, train, service, headsign, at]`),
//! the focused departure's id or none, whether the deck is open, the clock.

use exact_gpu::json::{list, number, text};
use exact_gpu::wgpu;
use exact_gpu::{Frame, Placement, Surface, SurfaceError, Value};

use crate::shaders::stack::{entry, module, Card as CardUniforms, CARD, GROUP_0, PICTURE, SMP};

/// Perspective: the eye's distance from the canvas plane, in points.
const FOCAL: f32 = 900.0;
/// The spring: stiffness and critical damping.
const STIFFNESS: f32 = 170.0;
/// Cards past this many on a closed deck are tucked away.
const DECK_DEPTH: usize = 6;
/// Cards past this many on an open fan are tucked away.
const FAN_DEPTH: usize = 7;
/// The spring integrates in steps no longer than this, in seconds, so a
/// clock that jumps — the agent's — settles the same way a display link
/// would have.
const STEP: f32 = 1.0 / 120.0;

/// A card's pose in canvas points: its top-left, its depth (toward the
/// eye), a tilt about the horizontal axis, a scale, and an opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pose {
    x: f32,
    y: f32,
    z: f32,
    tilt: f32,
    scale: f32,
    alpha: f32,
}

impl Pose {
    const ZERO: Pose = Pose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        tilt: 0.0,
        scale: 1.0,
        alpha: 1.0,
    };
    fn fields(&self) -> [f32; 6] {
        [self.x, self.y, self.z, self.tilt, self.scale, self.alpha]
    }
    fn from_fields(f: [f32; 6]) -> Pose {
        Pose {
            x: f[0],
            y: f[1],
            z: f[2],
            tilt: f[3],
            scale: f[4],
            alpha: f[5],
        }
    }
}

struct CardState {
    texture: Option<wgpu::TextureView>,
    frame: [f32; 4],
    pose: Pose,
    velocity: [f32; 6],
    target: Pose,
    placement: Option<Placement>,
    uniforms: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
}

impl CardState {
    fn new() -> CardState {
        CardState {
            texture: None,
            frame: [0.0; 4],
            pose: Pose::ZERO,
            velocity: [0.0; 6],
            target: Pose::ZERO,
            placement: None,
            uniforms: None,
            bind_group: None,
        }
    }
}

/// The stack surface.
#[derive(Default)]
pub struct StackSurface {
    ids: Vec<String>,
    ats: Vec<f64>,
    now: f64,
    focus: Option<String>,
    open: bool,
    cards: Vec<CardState>,
    gpu: Option<Gpu>,
    last_ms: Option<f64>,
    settled: bool,
}

struct Gpu {
    format: wgpu::TextureFormat,
    /// The shader generation the pipeline was built at (LLP 1030 D8).
    generation: u32,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// A 4×4, row major.
type Mat = [[f32; 4]; 4];

fn identity() -> Mat {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn translate(x: f32, y: f32, z: f32) -> Mat {
    let mut m = identity();
    m[0][3] = x;
    m[1][3] = y;
    m[2][3] = z;
    m
}

fn scale(s: f32) -> Mat {
    let mut m = identity();
    m[0][0] = s;
    m[1][1] = s;
    m
}

/// A tilt about the horizontal axis: the top of the card away from the eye
/// for a positive angle (y down, z toward the eye).
fn tilt(a: f32) -> Mat {
    let (s, c) = a.sin_cos();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, c, -s, 0.0],
        [0.0, s, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Perspective in canvas points: a point at depth `z` (toward the eye)
/// appears scaled by `f / (f - z)` about the canvas's centre.
fn perspective(cx: f32, cy: f32) -> Mat {
    [
        [1.0, 0.0, -cx / FOCAL, 0.0],
        [0.0, 1.0, -cy / FOCAL, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, -1.0 / FOCAL, 1.0],
    ]
}

/// Canvas points to clip space: x right, y down, the last component the
/// perspective divide. Depth is not tested — cards are drawn back to front —
/// so clip z is a constant inside the visible range, or a receded or tilted
/// card would be clipped away.
fn clip(width: f32, height: f32) -> Mat {
    [
        [2.0 / width, 0.0, 0.0, -1.0],
        [0.0, -2.0 / height, 0.0, 1.0],
        [0.0, 0.0, 0.0, 0.5],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

impl StackSurface {
    /// A fresh surface.
    pub fn new() -> StackSurface {
        StackSurface::default()
    }

    /// The pose every visible card is heading for, from the inputs: a deck
    /// when closed, a fan when open, the focus forward.
    fn targets(&mut self) {
        let mut shown = 0usize;
        for (i, card) in self.cards.iter_mut().enumerate() {
            let past = self.ats.get(i).is_some_and(|at| *at < self.now);
            let h = card.frame[3].max(1.0);
            let focused = self
                .focus
                .as_deref()
                .is_some_and(|f| self.ids.get(i).map(String::as_str) == Some(f));
            card.target = if past {
                Pose {
                    alpha: 0.0,
                    z: -400.0,
                    ..card.pose
                }
            } else if !self.open {
                StackSurface::stacked(shown)
            } else if shown >= FAN_DEPTH {
                Pose {
                    alpha: 0.0,
                    z: -400.0,
                    ..StackSurface::stacked(shown)
                }
            } else {
                // The fan: the soonest train nearest and lowest, later ones
                // stepping up and away, so every card's top — its countdown —
                // shows above the one in front of it.
                let n = shown as f32;
                let mut p = Pose {
                    x: 0.0,
                    y: 8.0 + (FAN_DEPTH as f32 - 1.0 - n) * (h * 0.56),
                    z: -34.0 * n,
                    tilt: -0.30,
                    scale: 1.0,
                    alpha: 1.0,
                };
                if focused {
                    p.z += 120.0;
                    p.tilt = 0.0;
                    p.scale = 1.04;
                } else if self.focus.is_some() {
                    p.alpha = 0.82;
                }
                p
            };
            if !past {
                shown += 1;
            }
        }
    }

    /// A card's place on the closed deck: the `n`th from the top.
    fn stacked(n: usize) -> Pose {
        let n = n as f32;
        Pose {
            x: 0.0,
            y: 12.0 * n,
            z: -46.0 * n,
            tilt: 0.0,
            scale: 1.0,
            alpha: if (n as usize) < DECK_DEPTH { 1.0 } else { 0.0 },
        }
    }

    /// Springs toward the targets over `dt` seconds.
    fn step(&mut self, dt: f32) {
        let damping = 2.0 * STIFFNESS.sqrt();
        for card in &mut self.cards {
            let mut p = card.pose.fields();
            let t = card.target.fields();
            for k in 0..6 {
                let a = STIFFNESS * (t[k] - p[k]) - damping * card.velocity[k];
                card.velocity[k] += a * dt;
                p[k] += card.velocity[k] * dt;
            }
            card.pose = Pose::from_fields(p);
        }
    }

    /// Whether any card is still short of its target or moving.
    fn unsettled(&self) -> bool {
        self.cards.iter().any(|card| {
            let p = card.pose.fields();
            let t = card.target.fields();
            (0..6).any(|k| (t[k] - p[k]).abs() > 0.01 || card.velocity[k].abs() > 0.5)
        })
    }

    /// A card's model matrix in canvas points (before the clip transform).
    fn model(card: &CardState, cx: f32, cy: f32) -> Mat {
        let [_, _, w, h] = card.frame;
        let p = card.pose;
        let centre = translate(p.x + w / 2.0, p.y + h / 2.0, p.z);
        let m = mul(
            &centre,
            &mul(
                &tilt(p.tilt),
                &mul(&scale(p.scale), &translate(-w / 2.0, -h / 2.0, 0.0)),
            ),
        );
        mul(&perspective(cx, cy), &m)
    }

    /// The homography (child points → canvas points) of a model matrix: the
    /// z column dropped, since the card is flat.
    fn homography(m: &Mat) -> [f32; 9] {
        [
            m[0][0], m[0][1], m[0][3], m[1][0], m[1][1], m[1][3], m[3][0], m[3][1], m[3][3],
        ]
    }
}

impl Surface for StackSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        let [board, focus, open, now] = inputs else {
            return Err(SurfaceError(format!(
                "stack: expected 4 inputs, got {}",
                inputs.len()
            )));
        };
        let rows = list(board, "board")?;
        self.ids = Vec::with_capacity(rows.len());
        self.ats = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let fields = list(row, "departure")?;
            self.ids.push(text(
                fields.first().unwrap_or(&Value::Unit),
                "departure.id",
            )?);
            self.ats.push(number(
                fields.get(4).unwrap_or(&Value::Unit),
                "departure.at",
            )?);
        }
        self.focus = match focus {
            Value::Unit | Value::Option(None) => None,
            Value::Option(Some(v)) => Some(text(v, "focus")?),
            Value::List(items) if items.is_empty() => None,
            Value::List(items) => Some(text(&items[0], "focus")?),
            other => Some(text(other, "focus")?),
        };
        self.open = match open {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            other => text(other, "deck")? == "open",
        };
        self.now = number(now, "nowMs")?;
        self.targets();
        self.settled = false;
        Ok(())
    }

    fn wants_children_each(&self) -> bool {
        true
    }

    fn child(&mut self, index: usize, texture: Option<&wgpu::TextureView>, frame: [f32; 4]) {
        if self.cards.len() <= index {
            self.cards.resize_with(index + 1, CardState::new);
        }
        let fresh = self.cards[index].texture.is_none();
        let card = &mut self.cards[index];
        card.frame = frame;
        if fresh {
            // A new card starts where the closed deck would hold it and
            // springs from there.
            card.pose = StackSurface::stacked(index);
            card.velocity = [0.0; 6];
        }
        if let Some(t) = texture {
            card.texture = Some(t.clone());
            card.bind_group = None;
        } else {
            card.texture = None;
            card.bind_group = None;
        }
        self.targets();
        self.settled = false;
    }

    fn children_count(&mut self, count: usize) {
        self.cards.truncate(count);
        self.targets();
        self.settled = false;
    }

    fn placement(&self, index: usize) -> Option<Placement> {
        self.cards.get(index).and_then(|c| c.placement)
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        if self.gpu.as_ref().map(|g| (g.format, g.generation))
            != Some((format, frame.shader_generation))
        {
            self.gpu = Some(build(device, format, frame.shader_generation));
            for card in &mut self.cards {
                card.bind_group = None;
            }
        }
        let mut dt = self
            .last_ms
            .map(|last| ((frame.now_ms - last) / 1000.0).clamp(0.0, 2.0) as f32)
            .unwrap_or(0.0);
        self.last_ms = Some(frame.now_ms);
        while dt > 0.0 {
            let h = dt.min(STEP);
            self.step(h);
            dt -= h;
        }
        self.settled = !self.unsettled();
        let (cx, cy) = (frame.width / 2.0, frame.height / 2.0);
        let to_clip = clip(frame.width, frame.height);
        // Poses, uniforms, placements; then draw back to front.
        let mut order: Vec<(usize, f32)> = Vec::with_capacity(self.cards.len());
        for (i, card) in self.cards.iter_mut().enumerate() {
            let model = StackSurface::model(card, cx, cy);
            let hidden = card.pose.alpha < 0.01;
            let mut homography = StackSurface::homography(&model);
            if hidden {
                // Off the canvas: nothing hits a card that is not there.
                homography[2] += 100_000.0;
            }
            card.placement = Some(Placement {
                hidden: false,
                homography,
                depth: card.pose.z,
            });
            let m = mul(&to_clip, &model);
            let gpu = self.gpu.as_ref().unwrap();
            let uniforms = card.uniforms.get_or_insert_with(|| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("card"),
                    size: CardUniforms::SIZE as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            });
            let dim = if hidden { 0.0 } else { 1.0 };
            let column = |k: usize| [m[0][k], m[1][k], m[2][k], m[3][k]];
            let u = CardUniforms {
                c0: column(0),
                c1: column(1),
                c2: column(2),
                c3: column(3),
                tint: [1.0, 1.0, 1.0, card.pose.alpha * dim],
                size: [card.frame[2], card.frame[3]],
                pad: [0.0, 0.0],
            };
            queue.write_buffer(uniforms, 0, &u.bytes());
            if card.bind_group.is_none() {
                if let Some(texture) = &card.texture {
                    card.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("card"),
                        layout: &gpu.layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: CARD.binding,
                                resource: uniforms.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: PICTURE.binding,
                                resource: wgpu::BindingResource::TextureView(texture),
                            },
                            wgpu::BindGroupEntry {
                                binding: SMP.binding,
                                resource: wgpu::BindingResource::Sampler(&gpu.sampler),
                            },
                        ],
                    }));
                }
            }
            if !hidden && card.bind_group.is_some() {
                order.push((i, card.pose.z));
            }
        }
        order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let gpu = self.gpu.as_ref().unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stack"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&gpu.pipeline);
            for (i, _) in &order {
                pass.set_bind_group(0, self.cards[*i].bind_group.as_ref().unwrap(), &[]);
                pass.draw(0..6, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
        !self.settled
    }
}

fn build(device: &wgpu::Device, format: wgpu::TextureFormat, generation: u32) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("card"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("stack"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("stack"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(entry::VS),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(entry::FS),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    Gpu {
        format,
        generation,
        pipeline,
        layout,
        sampler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_homography_agrees_with_the_matrix_at_rest() {
        // A card at rest maps its own points to its frame, through perspective.
        let mut card = CardState::new();
        card.frame = [10.0, 20.0, 100.0, 50.0];
        card.pose = Pose {
            x: 10.0,
            y: 20.0,
            ..Pose::ZERO
        };
        let m = StackSurface::model(&card, 200.0, 150.0);
        let h = StackSurface::homography(&m);
        let map = |x: f32, y: f32| {
            let w = h[6] * x + h[7] * y + h[8];
            (
                (h[0] * x + h[1] * y + h[2]) / w,
                (h[3] * x + h[4] * y + h[5]) / w,
            )
        };
        let (x0, y0) = map(0.0, 0.0);
        let (x1, y1) = map(100.0, 50.0);
        assert!(
            (x0 - 10.0).abs() < 1e-3 && (y0 - 20.0).abs() < 1e-3,
            "{x0} {y0}"
        );
        assert!(
            (x1 - 110.0).abs() < 1e-3 && (y1 - 70.0).abs() < 1e-3,
            "{x1} {y1}"
        );
    }
}
