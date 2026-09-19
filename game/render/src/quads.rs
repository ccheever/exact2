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
    #[cfg(not(target_arch = "wasm32"))]
    Child(u16),
}
impl Kind {
    // Cross-kind identity is part of the total translucent order. Placed children
    // join after model, sprite, particle; each owner keeps its own stable ordinal.
    fn rank(self) -> u8 {
        match self {
            Self::Model(..) => 0,
            Self::Sprite(_) => 1,
            Self::Particle(_) => 2,
            #[cfg(not(target_arch = "wasm32"))]
            Self::Child(_) => 3,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Order {
    pub kind: Kind,
    pub depth: f32,
    pub layer: i32,
    pub slot: u32,
    pub index: usize,
}
impl Order {
    fn compare(a: &Self, b: &Self) -> std::cmp::Ordering {
        b.depth
            .total_cmp(&a.depth)
            .then(a.layer.cmp(&b.layer))
            .then(a.slot.cmp(&b.slot))
            .then(a.kind.rank().cmp(&b.kind.rank()))
            .then(a.index.cmp(&b.index))
    }
}
pub(crate) struct Draw {
    pub kind: Kind,
    pub range: Range<u32>,
}
pub(crate) struct Item<T> {
    pub entity: Entity,
    pub poses: [Transform; 2],
    pub value: T,
    revision: u64,
}
struct Quad {
    words: [f32; 20],
}
struct Arena {
    reallocations: u64,
    buffer: Buffer,
    words: Vec<f32>,
}
impl Arena {
    fn new(d: &wgpu::Device, capacity: usize) -> Self {
        Self {
            reallocations: 0,
            buffer: Buffer::new(
                d,
                (capacity.max(1) * 80) as u64,
                wgpu::BufferUsages::VERTEX,
                "game quad instances",
            ),
            words: Vec::with_capacity(capacity * 20),
        }
    }
    fn upload(&mut self, d: &wgpu::Device, q: &wgpu::Queue) {
        self.reallocations += u64::from(self.buffer.grow(d, q, (self.words.len() * 4) as u64));
        self.buffer.write(q, 0, bytes(&self.words));
    }
}
pub(crate) struct Quads {
    particles: Vec<Item<Emitter>>,
    sprites: Vec<Item<Sprite>>,
    particle_data: Vec<Quad>,
    sprite_data: Vec<Quad>,
    particle_arena: Arena,
    sprite_arena: Option<Arena>,
    pub order: Vec<Order>,
    pub draws: Vec<Draw>,
    layout: wgpu::BindGroupLayout,
    bind: wgpu::BindGroup,
    particle_pipelines: Option<[wgpu::RenderPipeline; 2]>,
    sprite_pipelines: Option<[wgpu::RenderPipeline; 3]>,
    texture_layout: Option<wgpu::BindGroupLayout>,
    textures: BTreeMap<String, (u64, wgpu::BindGroup, [u32; 2])>,
    hz: u32,
    #[cfg(not(target_arch = "wasm32"))]
    children: Vec<(u16, crate::placed::Plane)>,
    #[cfg(not(target_arch = "wasm32"))]
    child_textures: BTreeMap<u16, (wgpu::TextureView, wgpu::BindGroup)>,
    #[cfg(not(target_arch = "wasm32"))]
    child_pipeline: Option<wgpu::RenderPipeline>,
}
impl Quads {
    #[cfg(test)]
    pub fn pipeline_count(&self) -> u64 {
        self.work_pipelines()
    }
    #[cfg(test)]
    pub fn particle_capacity(&self) -> usize {
        self.particle_data.capacity()
    }
    pub fn reallocations(&self) -> u64 {
        self.particle_arena.reallocations
            + self.sprite_arena.as_ref().map_or(0, |a| a.reallocations)
    }
    pub fn work_pipelines(&self) -> u64 {
        let count = u64::from(self.particle_pipelines.is_some()) * 2
            + u64::from(self.sprite_pipelines.is_some()) * 3;
        #[cfg(not(target_arch = "wasm32"))]
        let count = count + u64::from(self.child_pipeline.is_some());
        count
    }
    pub fn new<const ASSETS: bool>(
        d: &wgpu::Device,
        q: &wgpu::Queue,
        uniform: &wgpu::Buffer,
    ) -> Self {
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
        let mut result = Self {
            particles: Vec::new(),
            sprites: Vec::new(),
            sprite_data: Vec::with_capacity(if ASSETS { 4096 } else { 0 }),
            particle_data: Vec::with_capacity(exact_game::emitter::PARTICLE_BUDGET as usize),
            particle_arena: Arena::new(d, exact_game::emitter::PARTICLE_BUDGET as usize),
            sprite_arena: (ASSETS || cfg!(not(target_arch = "wasm32")))
                .then(|| Arena::new(d, 4096)),
            order: Vec::with_capacity(exact_game::emitter::PARTICLE_BUDGET as usize + 4096),
            draws: Vec::with_capacity(exact_game::emitter::PARTICLE_BUDGET as usize + 4096),
            layout,
            bind,
            particle_pipelines: None,
            sprite_pipelines: None,
            texture_layout: None,
            textures: BTreeMap::new(),
            hz: 60,
            #[cfg(not(target_arch = "wasm32"))]
            children: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            child_textures: BTreeMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            child_pipeline: None,
        };
        result.prepare(d, q);
        if ASSETS {
            result.prepare_sprites(d);
        }
        #[cfg(not(target_arch = "wasm32"))]
        result.prepare_children(d);
        result
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
        self.particles.retain(|item| match item.value.validate() {
            Ok(()) => true,
            Err(error) => {
                w.log(format!("{error} #{}", item.entity.index()));
                false
            }
        });
        if ASSETS {
            feed(w, &mut self.sprites, initial, next_tick, parent_changed);
            self.sprites.retain(|item| match item.value.validate() {
                Ok(()) => true,
                Err(error) => {
                    w.log(format!("{error} #{}", item.entity.index()));
                    false
                }
            });
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
    #[cfg(not(target_arch = "wasm32"))]
    pub fn children(&mut self, d: &wgpu::Device, placed: &crate::placed::Placements) {
        self.children.clear();
        self.child_textures.retain(|i, _| {
            placed
                .children
                .get(usize::from(*i))
                .is_some_and(|c| c.texture.is_some())
        });
        for (i, child) in placed.children.iter().enumerate() {
            let Ok(i) = u16::try_from(i) else {
                break;
            };
            let Some(texture) = &child.texture else {
                continue;
            };
            let Some(plane) = child.plane.filter(|p| !p.placement.hidden) else {
                continue;
            };
            self.prepare_children(d);
            if self
                .child_textures
                .get(&i)
                .is_none_or(|(view, _)| view != texture)
            {
                let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                });
                let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("game captured child"),
                    layout: self.texture_layout.as_ref().unwrap(),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(texture),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });
                self.child_textures.insert(i, (texture.clone(), bind));
            }
            self.children.push((i, plane));
        }
    }
    pub fn retain_textures(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.textures.retain(|n, _| keep(n));
    }
    pub fn retire_texture(&mut self, name: &str) {
        self.textures.remove(name);
    }
    pub fn has_texture(&self, name: &str) -> bool {
        self.textures.contains_key(name)
    }
    fn texture_layout(&mut self, d: &wgpu::Device) {
        if self.texture_layout.is_some() {
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
        self.texture_layout = Some(texture);
    }
    fn prepare_sprites(&mut self, d: &wgpu::Device) {
        if self.sprite_pipelines.is_some() {
            return;
        }
        self.texture_layout(d);
        let shader = source(d, include_str!("shaders/sprite.wgsl"));
        let texture = self.texture_layout.as_ref().unwrap();
        self.sprite_pipelines = Some(std::array::from_fn(|i| {
            pipeline(d, &shader, &[&self.layout, texture], i)
        }));
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn prepare_children(&mut self, d: &wgpu::Device) {
        if self.child_pipeline.is_some() {
            return;
        }
        self.texture_layout(d);
        let shader = source(d, include_str!("shaders/sprite.wgsl"));
        self.child_pipeline = Some(pipeline(
            d,
            &shader,
            &[&self.layout, self.texture_layout.as_ref().unwrap()],
            4,
        ));
    }
    pub fn prepare(&mut self, d: &wgpu::Device, q: &wgpu::Queue) {
        // Membership-sized sprite capacity is reserved in feed preparation, not draw.
        let sprites = self.sprites.len();
        if let Some(arena) = &mut self.sprite_arena {
            arena.reallocations += u64::from(arena.buffer.grow(d, q, (sprites * 80) as u64));
            arena
                .words
                .reserve((sprites * 20).saturating_sub(arena.words.len()));
        }
        self.sprite_data
            .reserve(sprites.saturating_sub(self.sprite_data.len()));
        if self.particle_pipelines.is_none() {
            let shader = source(d, include_str!("shaders/particle.wgsl"));
            self.particle_pipelines = Some(std::array::from_fn(|i| {
                pipeline(d, &shader, &[&self.layout], i + 2)
            }));
        }
    }
    pub fn frame<const ASSETS: bool>(
        &mut self,
        _d: &wgpu::Device,
        _q: &wgpu::Queue,
        f: &FrameInput<'_>,
    ) {
        self.order.clear();
        self.draws.clear();
        self.particle_data.clear();
        self.sprite_data.clear();
        self.particle_arena.words.clear();
        if let Some(arena) = &mut self.sprite_arena {
            arena.words.clear();
        }
        let inv = f.view.inverse();
        let right = inv.x_axis.truncate().normalize();
        let up = inv.y_axis.truncate().normalize();
        let mut left = exact_game::emitter::PARTICLE_BUDGET;
        for item in &self.particles {
            let t = f.displayed(
                item.entity,
                crate::world::scene::interpolate(item.poses, f.alpha),
            );
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
                let t = f.displayed(
                    item.entity,
                    crate::world::scene::interpolate(item.poses, f.alpha),
                );
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
                let at = self.sprite_data.len();
                self.sprite_data
                    .push(quad(center, x, y, s.color, uv, mode, s.cutoff));
                if s.alpha == AlphaMode::Blend {
                    self.order.push(Order {
                        kind: Kind::Sprite(index),
                        depth: -f.view.transform_point3(center).z,
                        layer: s.layer,
                        slot: item.entity.index(),
                        index: at,
                    });
                } else {
                    let start = (self.sprite_arena.as_mut().unwrap().words.len() / 20) as u32;
                    self.sprite_arena
                        .as_mut()
                        .unwrap()
                        .words
                        .extend(self.sprite_data[at].words);
                    push_draw(&mut self.draws, &self.sprites, Kind::Sprite(index), start);
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        for &(child, p) in &self.children {
            let index = self.sprite_arena.as_mut().unwrap().words.len() / 20;
            self.sprite_arena
                .as_mut()
                .unwrap()
                .words
                .extend(quad(p.center, p.x, p.y, [1.; 4], [0., 0., 1., 1.], 4., 0.).words);
            self.order.push(Order {
                kind: Kind::Child(child),
                depth: -p.placement.depth,
                layer: 0,
                slot: u32::from(child),
                index,
            });
        }
    }
    pub fn order<const ASSETS: bool>(&mut self, d: &wgpu::Device, q: &wgpu::Queue) {
        self.order.sort_unstable_by(Order::compare);
        for o in &self.order {
            let at = match o.kind {
                Kind::Particle(_) => {
                    let at = self.particle_arena.words.len() / 20;
                    self.particle_arena
                        .words
                        .extend(self.particle_data[o.index].words);
                    at as u32
                }
                Kind::Sprite(_) if ASSETS => {
                    let at = self.sprite_arena.as_mut().unwrap().words.len() / 20;
                    self.sprite_arena
                        .as_mut()
                        .unwrap()
                        .words
                        .extend(self.sprite_data[o.index].words);
                    at as u32
                }
                _ => o.index as u32,
            };
            push_draw(&mut self.draws, &self.sprites, o.kind, at);
        }
        if let Some(arena) = &mut self.sprite_arena {
            arena.upload(d, q);
        }
        self.particle_arena.upload(d, q);
    }
    pub fn draw<'a, const ASSETS: bool>(&'a self, pass: &mut wgpu::RenderPass<'a>, draw: &Draw) {
        pass.set_bind_group(0, &self.bind, &[]);
        match draw.kind {
            Kind::Particle(additive) => {
                pass.set_pipeline(
                    &self.particle_pipelines.as_ref().unwrap()[usize::from(additive)],
                );
                pass.set_vertex_buffer(0, self.particle_arena.buffer.raw.slice(..));
            }
            Kind::Sprite(i) if ASSETS => {
                let s = &self.sprites[i].value;
                let variant = match s.alpha {
                    AlphaMode::Opaque => 0,
                    AlphaMode::Mask => 1,
                    AlphaMode::Blend => 2,
                };
                pass.set_pipeline(&self.sprite_pipelines.as_ref().unwrap()[variant]);
                pass.set_bind_group(1, &self.textures[&s.texture].1, &[]);
                pass.set_vertex_buffer(0, self.sprite_arena.as_ref().unwrap().buffer.raw.slice(..));
            }
            #[cfg(not(target_arch = "wasm32"))]
            Kind::Child(i) => {
                pass.set_pipeline(self.child_pipeline.as_ref().unwrap());
                pass.set_bind_group(1, &self.child_textures[&i].1, &[]);
                pass.set_vertex_buffer(0, self.sprite_arena.as_ref().unwrap().buffer.raw.slice(..));
            }
            Kind::Model(..) | Kind::Sprite(_) => unreachable!(),
        }
        pass.draw(0..6, draw.range.clone());
    }
    pub fn opaque(&self, draw: &Draw) -> bool {
        matches!(draw.kind,Kind::Sprite(i) if self.sprites[i].value.alpha!=AlphaMode::Blend)
    }
    pub fn instances(&self) -> u64 {
        ((self.particle_arena.words.len()
            + self.sprite_arena.as_ref().map_or(0, |a| a.words.len()))
            / 20) as u64
    }
}
fn push_draw(draws: &mut Vec<Draw>, sprites: &[Item<Sprite>], kind: Kind, at: u32) {
    if let Some(previous) = draws.last_mut() {
        let compatible = match (previous.kind, kind) {
            (Kind::Particle(a), Kind::Particle(b)) => a == b,
            (Kind::Sprite(a), Kind::Sprite(b)) => {
                let (a, b) = (&sprites[a].value, &sprites[b].value);
                a.texture == b.texture && a.alpha == b.alpha
            }
            _ => false,
        };
        if compatible && previous.range.end == at {
            previous.range.end += 1;
            return;
        }
    }
    draws.push(Draw {
        kind,
        range: at..at + 1,
    });
}
pub(crate) fn feed<T: exact_game::Component + Clone>(
    w: &World,
    items: &mut Vec<Item<T>>,
    initial: bool,
    next_tick: bool,
    parent_changed: bool,
) {
    let revision = w.revision::<T>();
    // Compact once, then append arrivals. Never shift the tail for each removal.
    items.retain(|i| {
        w.get::<T>(i.entity).is_some()
            && !w.get::<Visible>(i.entity).is_some_and(|v| !v.0)
            && crate::world::scene::pose(w, i.entity).is_some()
    });
    for i in items.iter_mut() {
        let pose = crate::world::scene::pose(w, i.entity).unwrap();
        if next_tick {
            i.poses[0] = i.poses[1];
        }
        i.poses[1] = pose;
        if initial || crate::world::scene::snap(w, i.entity, parent_changed) {
            i.poses[0] = pose;
        }
        if initial || i.revision != revision {
            i.value.clone_from(&*w.get::<T>(i.entity).unwrap());
            i.revision = revision;
        }
    }
    let existing = items.len();
    for (entity, value) in w.query::<&T>().iter() {
        if items[..existing]
            .binary_search_by_key(&entity.index(), |i| i.entity.index())
            .is_ok()
            || w.get::<Visible>(entity).is_some_and(|v| !v.0)
        {
            continue;
        }
        if let Some(pose) = crate::world::scene::pose(w, entity) {
            items.push(Item {
                entity,
                poses: [pose; 2],
                value: value.clone(),
                revision,
            });
        }
    }
    if items.len() != existing {
        items.sort_unstable_by_key(|i| i.entity.index());
    }
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
    let blend = if mode == 4 {
        Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
    } else if mode == 3 {
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

#[cfg(test)]
mod retained_tests {
    use super::*;
    #[test]
    fn every_kind_and_owner_ordinal_has_the_same_total_order() {
        let mut kinds = vec![Kind::Particle(false), Kind::Model(0, 0), Kind::Sprite(0)];
        #[cfg(not(target_arch = "wasm32"))]
        kinds.push(Kind::Child(0));
        let mut rows = Vec::new();
        for kind in kinds {
            for index in [2, 0, 1] {
                rows.push(Order {
                    kind,
                    depth: 3.,
                    layer: 1,
                    slot: 7,
                    index,
                });
            }
        }
        rows.reverse();
        rows.sort_unstable_by(Order::compare);
        let actual: Vec<_> = rows.iter().map(|o| (o.kind.rank(), o.index)).collect();
        let mut expected = vec![
            (0, 0),
            (0, 1),
            (0, 2),
            (1, 0),
            (1, 1),
            (1, 2),
            (2, 0),
            (2, 1),
            (2, 2),
        ];
        #[cfg(not(target_arch = "wasm32"))]
        expected.extend([(3, 0), (3, 1), (3, 2)]);
        assert_eq!(actual, expected);
        // Rank is independently pinned; a mistaken enum rank must fail too.
        assert_eq!(Kind::Model(0, 0).rank(), 0);
        assert_eq!(Kind::Sprite(0).rank(), 1);
        assert_eq!(Kind::Particle(false).rank(), 2);
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(Kind::Child(0).rank(), 3);
    }
    #[test]
    fn revisions_reuse_owned_values_and_membership_compacts_in_order() {
        let mut w = World::new(60, 0);
        let first = w.spawn((Transform::default(), Emitter::default()));
        let entity = w.spawn((
            Transform::default(),
            Emitter {
                state: exact_game::emitter::EmitterState {
                    births: vec![Default::default()],
                    ..Default::default()
                },
                ..Default::default()
            },
            Sprite::new("shared.tex", [1., 1.]),
        ));
        w.propagate();
        let mut emitters = Vec::new();
        let mut sprites = Vec::new();
        feed::<Emitter>(&w, &mut emitters, true, false, false);
        feed::<Sprite>(&w, &mut sprites, true, false, false);
        let births = emitters[1].value.state.births.as_ptr();
        let texture = sprites[0].value.texture.as_ptr();
        w.get_mut::<Emitter>(entity).unwrap().rate = 90.;
        w.get_mut::<Sprite>(entity).unwrap().frame = [16, 0, 16, 16];
        feed::<Emitter>(&w, &mut emitters, false, false, false);
        feed::<Sprite>(&w, &mut sprites, false, false, false);
        assert_eq!(emitters[1].value.state.births.as_ptr(), births);
        assert_eq!(sprites[0].value.texture.as_ptr(), texture);
        assert_eq!(emitters[1].value.rate, 90.);
        assert_eq!(sprites[0].value.frame, [16, 0, 16, 16]);
        // Sentinel proves the unchanged-revision branch skipped the copy.
        emitters[1].value.rate = 91.;
        sprites[0].value.frame = [32, 0, 16, 16];
        feed::<Emitter>(&w, &mut emitters, false, false, false);
        feed::<Sprite>(&w, &mut sprites, false, false, false);
        assert_eq!(emitters[1].value.state.births.as_ptr(), births);
        assert_eq!(emitters[1].value.rate, 91.);
        assert_eq!(sprites[0].value.frame, [32, 0, 16, 16]);
        w.despawn(first);
        feed::<Emitter>(&w, &mut emitters, false, false, false);
        assert_eq!(emitters.len(), 1);
        assert_eq!(emitters[0].entity, entity);
        assert_eq!(emitters[0].value.state.births.as_ptr(), births);
    }
}
