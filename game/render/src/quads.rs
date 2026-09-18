//! Separate sprite/particle arenas and one total order shared with blended models.
use crate::{
    buffers::{bytes, Buffer},
    FrameInput, RenderError,
};
use exact_game::{asset::AlphaMode, Emitter, Entity, Sprite, Transform, Visible, World};
use exact_gpu::wgpu;
use glam::Vec3;
use std::{collections::BTreeMap, ops::Range};

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Model(usize, u32),
    Particle(bool),
    Sprite(usize),
}
#[derive(Clone, Copy)]
pub(crate) struct Order {
    pub kind: Kind,
    pub depth: f32,
    pub layer: i32,
    pub slot: u32,
    pub index: usize,
}
pub(crate) struct Draw {
    pub kind: Kind,
    pub range: Range<u32>,
}
struct Item<T> {
    entity: Entity,
    poses: [Transform; 2],
    value: T,
}
struct Quad {
    words: [f32; 20],
}
struct Arena {
    buffer: Buffer,
    words: Vec<f32>,
}
impl Arena {
    fn new(d: &wgpu::Device) -> Self {
        Self {
            buffer: Buffer::new(d, 80, wgpu::BufferUsages::VERTEX, "game quad instances"),
            words: Vec::new(),
        }
    }
    fn upload(&mut self, d: &wgpu::Device, q: &wgpu::Queue) {
        self.buffer.grow(d, q, (self.words.len() * 4) as u64);
        self.buffer.write(q, 0, bytes(&self.words));
    }
}
pub(crate) struct Quads {
    particles: Vec<Item<Emitter>>,
    sprites: Vec<Item<Sprite>>,
    particle_data: Vec<Quad>,
    particle_arena: Arena,
    sprite_arena: Arena,
    pub order: Vec<Order>,
    pub draws: Vec<Draw>,
    layout: wgpu::BindGroupLayout,
    bind: wgpu::BindGroup,
    particle_pipelines: Option<[wgpu::RenderPipeline; 2]>,
    sprite_pipelines: Option<[wgpu::RenderPipeline; 3]>,
    texture_layout: Option<wgpu::BindGroupLayout>,
    textures: BTreeMap<String, (u64, wgpu::BindGroup, [u32; 2])>,
    hz: u32,
}
impl Quads {
    pub fn new(d: &wgpu::Device, uniform: &wgpu::Buffer) -> Self {
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game quad frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game quad frame"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            particles: Vec::new(),
            sprites: Vec::new(),
            particle_data: Vec::new(),
            particle_arena: Arena::new(d),
            sprite_arena: Arena::new(d),
            order: Vec::new(),
            draws: Vec::new(),
            layout,
            bind,
            particle_pipelines: None,
            sprite_pipelines: None,
            texture_layout: None,
            textures: BTreeMap::new(),
            hz: 60,
        }
    }
    pub fn feed<const ASSETS: bool>(
        &mut self,
        w: &World,
        initial: bool,
        next_tick: bool,
        parent_changed: bool,
    ) -> Result<(), RenderError> {
        self.hz = w.hz();
        feed(w, &mut self.particles, initial, next_tick, parent_changed);
        for item in &self.particles {
            item.value.validate().map_err(RenderError::scene)?;
        }
        if ASSETS {
            feed(w, &mut self.sprites, initial, next_tick, parent_changed);
            for item in &self.sprites {
                item.value.validate().map_err(RenderError::scene)?;
            }
        } else if let Some((_, s)) = w.query::<&Sprite>().iter().next() {
            return Err(RenderError::scene(format!(
                "Sprite `{}` requires game.assets: true",
                s.texture
            )));
        }
        Ok(())
    }
    pub fn texture(&mut self, d: &wgpu::Device, name: &str, t: &crate::models::Texture) {
        self.prepare_sprites(d);
        if self
            .textures
            .get(name)
            .is_some_and(|(digest, _, _)| *digest == t.digest)
        {
            return;
        }
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game sprite texture"),
            layout: self.texture_layout.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&t.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&t.sampler),
                },
            ],
        });
        self.textures.insert(name.into(), (t.digest, bind, t.size));
    }
    pub fn retire_texture(&mut self, name: &str) {
        self.textures.remove(name);
    }
    pub fn has_texture(&self, name: &str) -> bool {
        self.textures.contains_key(name)
    }
    fn prepare_sprites(&mut self, d: &wgpu::Device) {
        if self.sprite_pipelines.is_some() {
            return;
        }
        let texture = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game sprite texture"),
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
        let shader = source(d, include_str!("shaders/sprite.wgsl"));
        self.sprite_pipelines = Some(std::array::from_fn(|i| {
            pipeline(d, &shader, &[&self.layout, &texture], i)
        }));
        self.texture_layout = Some(texture);
    }
    pub fn prepare(&mut self, d: &wgpu::Device) {
        if !self.particles.is_empty() && self.particle_pipelines.is_none() {
            let shader = source(d, include_str!("shaders/particle.wgsl"));
            self.particle_pipelines = Some(std::array::from_fn(|i| {
                pipeline(d, &shader, &[&self.layout], i + 2)
            }));
        }
    }
    pub fn frame<const ASSETS: bool>(
        &mut self,
        d: &wgpu::Device,
        q: &wgpu::Queue,
        f: &FrameInput<'_>,
    ) {
        self.order.clear();
        self.draws.clear();
        self.particle_data.clear();
        self.particle_arena.words.clear();
        self.sprite_arena.words.clear();
        let inv = f.view.inverse();
        let right = inv.x_axis.truncate().normalize();
        let up = inv.y_axis.truncate().normalize();
        let mut left = exact_game::emitter::PARTICLE_BUDGET;
        for item in &self.particles {
            let t = crate::world::scene::interpolate(item.poses, f.alpha);
            let e = &item.value;
            e.particles(self.hz, f.alpha, |p| {
                if left == 0 {
                    return;
                }
                left -= 1;
                let position = t.position + t.rotation * (t.scale * p.position);
                let index = self.particle_data.len();
                self.particle_data.push(quad(
                    position,
                    right * p.size * t.scale.x.abs(),
                    up * p.size * t.scale.y.abs(),
                    p.color,
                    [0., 0., 1., 1.],
                    3.,
                    0.,
                ));
                self.order.push(Order {
                    kind: Kind::Particle(e.additive),
                    depth: -f.view.transform_point3(position).z,
                    layer: e.layer,
                    slot: item.entity.index(),
                    index,
                });
            });
        }
        if ASSETS {
            for (index, item) in self.sprites.iter().enumerate() {
                let s = &item.value;
                let Some((_, _, size)) = self.textures.get(&s.texture) else {
                    continue;
                };
                let t = crate::world::scene::interpolate(item.poses, f.alpha);
                let x = right * (s.size.x * t.scale.x.abs());
                let y = up * (s.size.y * t.scale.y.abs());
                let center = t.position + x * (0.5 - s.anchor.x) + y * (0.5 - s.anchor.y);
                let [fx, fy, fw, fh] = s.frame.map(f32::from);
                let mut uv = if fw == 0. || fh == 0. {
                    [0., 0., 1., 1.]
                } else {
                    [
                        fx / size[0] as f32,
                        fy / size[1] as f32,
                        fw / size[0] as f32,
                        fh / size[1] as f32,
                    ]
                };
                for i in 0..2 {
                    if s.flip[i] {
                        uv[i] += uv[i + 2];
                        uv[i + 2] = -uv[i + 2];
                    }
                }
                let mode = match s.alpha {
                    AlphaMode::Opaque => 0.,
                    AlphaMode::Mask => 1.,
                    AlphaMode::Blend => 2.,
                };
                let at = self.sprite_arena.words.len() / 20;
                self.sprite_arena
                    .words
                    .extend(quad(center, x, y, s.color, uv, mode, s.cutoff).words);
                if s.alpha == AlphaMode::Blend {
                    self.order.push(Order {
                        kind: Kind::Sprite(index),
                        depth: -f.view.transform_point3(center).z,
                        layer: s.layer,
                        slot: item.entity.index(),
                        index: at,
                    });
                } else {
                    self.draws.push(Draw {
                        kind: Kind::Sprite(index),
                        range: at as u32..at as u32 + 1,
                    });
                }
            }
            self.sprite_arena.upload(d, q);
        }
    }
    pub fn order(&mut self, d: &wgpu::Device, q: &wgpu::Queue) {
        self.order.sort_unstable_by(|a, b| {
            b.depth
                .total_cmp(&a.depth)
                .then(a.layer.cmp(&b.layer))
                .then(a.slot.cmp(&b.slot))
                .then(a.index.cmp(&b.index))
        });
        for o in &self.order {
            let at = match o.kind {
                Kind::Particle(_) => {
                    let at = self.particle_arena.words.len() / 20;
                    self.particle_arena
                        .words
                        .extend(self.particle_data[o.index].words);
                    at as u32
                }
                _ => o.index as u32,
            };
            if let Kind::Particle(blend) = o.kind {
                if let Some(Draw {
                    kind: Kind::Particle(b),
                    range,
                }) = self.draws.last_mut()
                {
                    if blend == *b && range.end == at {
                        range.end += 1;
                        continue;
                    }
                }
            }
            self.draws.push(Draw {
                kind: o.kind,
                range: at..at + 1,
            });
        }
        self.particle_arena.upload(d, q);
    }
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, draw: &Draw) {
        pass.set_bind_group(0, &self.bind, &[]);
        match draw.kind {
            Kind::Particle(additive) => {
                pass.set_pipeline(
                    &self.particle_pipelines.as_ref().unwrap()[usize::from(additive)],
                );
                pass.set_vertex_buffer(0, self.particle_arena.buffer.raw.slice(..));
            }
            Kind::Sprite(i) => {
                let s = &self.sprites[i].value;
                let variant = match s.alpha {
                    AlphaMode::Opaque => 0,
                    AlphaMode::Mask => 1,
                    AlphaMode::Blend => 2,
                };
                pass.set_pipeline(&self.sprite_pipelines.as_ref().unwrap()[variant]);
                pass.set_bind_group(1, &self.textures[&s.texture].1, &[]);
                pass.set_vertex_buffer(0, self.sprite_arena.buffer.raw.slice(..));
            }
            Kind::Model(..) => unreachable!(),
        }
        pass.draw(0..6, draw.range.clone());
    }
    pub fn opaque(&self, draw: &Draw) -> bool {
        matches!(draw.kind,Kind::Sprite(i) if self.sprites[i].value.alpha!=AlphaMode::Blend)
    }
    pub fn instances(&self) -> u64 {
        ((self.particle_arena.words.len() + self.sprite_arena.words.len()) / 20) as u64
    }
}
fn feed<T: exact_game::Component + Clone>(
    w: &World,
    items: &mut Vec<Item<T>>,
    initial: bool,
    next_tick: bool,
    parent_changed: bool,
) {
    let mut at = 0;
    for (entity, value) in w.query::<&T>().iter() {
        if w.get::<Visible>(entity).is_some_and(|v| !v.0) {
            continue;
        }
        let Some(pose) = crate::world::scene::pose(w, entity) else {
            continue;
        };
        while at < items.len() && items[at].entity.index() < entity.index() {
            items.remove(at);
        }
        if items.get(at).is_some_and(|i| i.entity == entity) {
            let i = &mut items[at];
            if next_tick {
                i.poses[0] = i.poses[1];
            }
            i.poses[1] = pose;
            if initial || crate::world::scene::snap(w, entity, parent_changed) {
                i.poses[0] = pose;
            }
            i.value.clone_from(value);
        } else {
            items.insert(
                at,
                Item {
                    entity,
                    poses: [pose; 2],
                    value: value.clone(),
                },
            );
        }
        at += 1;
    }
    items.truncate(at);
}
fn quad(p: Vec3, x: Vec3, y: Vec3, c: [f32; 4], uv: [f32; 4], mode: f32, cutoff: f32) -> Quad {
    Quad {
        words: [
            p.x, p.y, p.z, cutoff, x.x, x.y, x.z, mode, y.x, y.y, y.z, 0., c[0], c[1], c[2], c[3],
            uv[0], uv[1], uv[2], uv[3],
        ],
    }
}
fn source(d: &wgpu::Device, fragment: &str) -> wgpu::ShaderModule {
    d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("game quad"),
        source: wgpu::ShaderSource::Wgsl(
            [
                include_str!("shaders/frame.wgsl"),
                include_str!("shaders/quad.wgsl"),
                fragment,
            ]
            .concat()
            .into(),
        ),
    })
}
fn pipeline(
    d: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layouts: &[&wgpu::BindGroupLayout],
    mode: usize,
) -> wgpu::RenderPipeline {
    let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("game quad"),
        bind_group_layouts: &layouts.iter().map(|l| Some(*l)).collect::<Vec<_>>(),
        immediate_size: 0,
    });
    let attrs =
        wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32x4,3=>Float32x4,4=>Float32x4];
    let blend = if mode == 3 {
        Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        })
    } else {
        (mode == 2).then_some(wgpu::BlendState::ALPHA_BLENDING)
    };
    d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("game quad"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("quad_vs"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 80,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &attrs,
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("quad_fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(mode < 2),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 4,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}
