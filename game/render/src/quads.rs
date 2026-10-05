//! Separate sprite/particle arenas and one total order shared with blended models.
use crate::{
    buffers::{bytes, Buffer},
    FrameInput, RenderError,
};
use exact_game::{asset::AlphaMode, Emitter, Entity, ParticleLook, Sprite, Transform, World};
use exact_gpu::wgpu;
use glam::{Vec2, Vec3};
use std::{collections::BTreeMap, ops::Range};

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Model(usize, u32),
    /// Additive, the `looks` index of a textured look (`NO_LOOK` for dots), and
    /// soft (fading into the opaque scene).
    Particle(bool, u32, bool),
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
            Self::Particle(..) => 2,
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
    // The entity's effective `Opacity` (1 when unfiltered by visibility).
    pub opacity: f32,
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
const NO_LOOK: u32 = u32::MAX;
pub(crate) struct Quads {
    particles: Vec<Item<Emitter>>,
    looks: Vec<Item<ParticleLook>>,
    textured_pipelines: Option<[wgpu::RenderPipeline; 2]>,
    sprites: Vec<Item<Sprite>>,
    particle_data: Vec<Quad>,
    sprite_data: Vec<Quad>,
    particle_arena: Option<Arena>,
    sprite_arena: Option<Arena>,
    pub order: Vec<Order>,
    pub draws: Vec<Draw>,
    layout: wgpu::BindGroupLayout,
    bind: wgpu::BindGroup,
    particle_pipelines: Option<[wgpu::RenderPipeline; 2]>,
    // Soft dots (alpha, additive), then soft textured particles.
    soft_pipelines: Option<[wgpu::RenderPipeline; 4]>,
    soft_layout: Option<wgpu::BindGroupLayout>,
    soft_uniform: Option<wgpu::Buffer>,
    // This frame's scene depth binding, only while the translucent pass is split.
    soft_bind: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    soft_active: bool,
    sprite_pipelines: Option<[wgpu::RenderPipeline; 3]>,
    texture_layout: Option<wgpu::BindGroupLayout>,
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
    pub fn reserved_bytes(&self) -> (usize, u64) {
        (
            self.particle_data.capacity() * size_of::<Quad>()
                + self
                    .particle_arena
                    .as_ref()
                    .map_or(0, |a| a.words.capacity() * size_of::<f32>())
                + self.order.capacity() * size_of::<Order>()
                + self.draws.capacity() * size_of::<Draw>(),
            self.particle_arena
                .as_ref()
                .map_or(0, |a| a.buffer.raw.size()),
        )
    }
    #[cfg(test)]
    pub fn particle_capacity(&self) -> usize {
        self.particle_data.capacity()
    }
    pub fn reallocations(&self) -> u64 {
        self.particle_arena.as_ref().map_or(0, |a| a.reallocations)
            + self.sprite_arena.as_ref().map_or(0, |a| a.reallocations)
    }
    pub fn work_pipelines(&self) -> u64 {
        let count = u64::from(self.particle_pipelines.is_some()) * 2
            + u64::from(self.soft_pipelines.is_some()) * 4
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
            looks: Vec::new(),
            textured_pipelines: None,
            sprites: Vec::new(),
            sprite_data: Vec::with_capacity(if ASSETS { 4096 } else { 0 }),
            particle_data: Vec::new(),
            particle_arena: None,
            sprite_arena: (ASSETS || cfg!(not(target_arch = "wasm32")))
                .then(|| Arena::new(d, 4096)),
            order: Vec::with_capacity(4096),
            draws: Vec::with_capacity(4096),
            layout,
            bind,
            particle_pipelines: None,
            soft_pipelines: None,
            soft_layout: None,
            soft_uniform: None,
            soft_bind: None,
            soft_active: false,
            sprite_pipelines: None,
            texture_layout: None,
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
        feed(w, &mut self.looks, initial, next_tick, parent_changed);
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
    pub fn sprite_bind(&mut self, d: &wgpu::Device, t: &crate::models::Texture) -> wgpu::BindGroup {
        self.prepare_sprites(d);
        d.create_bind_group(&wgpu::BindGroupDescriptor {
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
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn children(
        &mut self,
        d: &wgpu::Device,
        q: &wgpu::Queue,
        placed: &crate::placed::Placements,
    ) {
        let previous_count = self.children.len();
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
        if self.children.len() != previous_count {
            self.prepare(d, q);
        }
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
        #[cfg(not(target_arch = "wasm32"))]
        let sprites = sprites + self.children.len();
        if let Some(arena) = &mut self.sprite_arena {
            arena.reallocations += u64::from(arena.buffer.grow(d, q, (sprites * 80) as u64));
            arena
                .words
                .reserve((sprites * 20).saturating_sub(arena.words.len()));
        }
        self.sprite_data
            .reserve(sprites.saturating_sub(self.sprite_data.len()));
        let particles = self
            .particles
            .iter()
            .map(|p| {
                let emitter = &p.value;
                let births = emitter
                    .state
                    .births
                    .iter()
                    .map(|b| u64::from(b.count))
                    .sum::<u64>();
                // Prepare a steady stream before ready, including the retained interpolation tick.
                let stream = (emitter.rate as f64 * (emitter.lifetime as f64 + 1. / self.hz as f64))
                    .ceil() as u64;
                births
                    .max(stream)
                    .saturating_add(u64::from(emitter.state.burst))
            })
            .sum::<u64>()
            .min(u64::from(exact_game::emitter::PARTICLE_BUDGET)) as usize;
        if !self.particles.is_empty() {
            let arena = self
                .particle_arena
                .get_or_insert_with(|| Arena::new(d, particles));
            arena.reallocations += u64::from(arena.buffer.grow(d, q, (particles * 80) as u64));
            arena
                .words
                .reserve((particles * 20).saturating_sub(arena.words.len()));
            self.particle_data
                .reserve(particles.saturating_sub(self.particle_data.len()));
        }
        let entries = particles + sprites + 4096;
        self.order.reserve(entries.saturating_sub(self.order.len()));
        self.draws.reserve(entries.saturating_sub(self.draws.len()));
        if !self.looks.is_empty() && self.textured_pipelines.is_none() {
            self.texture_layout(d);
            let shader = source(d, include_str!("shaders/sprite.wgsl"));
            let texture = self.texture_layout.as_ref().unwrap();
            // Textured particles: straight alpha or additive, like dots.
            self.textured_pipelines = Some(std::array::from_fn(|i| {
                pipeline(d, &shader, &[&self.layout, texture], i + 2)
            }));
        }
        if !self.particles.is_empty() && self.particle_pipelines.is_none() {
            let shader = source(d, include_str!("shaders/particle.wgsl"));
            self.particle_pipelines = Some(std::array::from_fn(|i| {
                pipeline(d, &shader, &[&self.layout], i + 2)
            }));
        }
        if self.wants_soft() && self.soft_pipelines.is_none() {
            self.texture_layout(d);
            let depth = wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: true,
                },
                count: None,
            };
            let uniform = wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            };
            let soft = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("game soft particles"),
                entries: &[depth, uniform],
            });
            let soft_source = include_str!("shaders/soft.wgsl");
            let dots = source(
                d,
                &[include_str!("shaders/particle.wgsl"), soft_source].concat(),
            );
            let textured = source(
                d,
                &[include_str!("shaders/sprite.wgsl"), soft_source].concat(),
            );
            let texture = self.texture_layout.as_ref().unwrap();
            self.soft_pipelines = Some(std::array::from_fn(|i| {
                let (shader, group) = if i < 2 {
                    (&dots, None)
                } else {
                    (&textured, Some(texture))
                };
                pipeline_entry(
                    d,
                    shader,
                    &[Some(&self.layout), group, Some(&soft)],
                    i % 2 + 2,
                    "quad_soft_fs",
                )
            }));
            self.soft_uniform = Some(d.create_buffer(&wgpu::BufferDescriptor {
                label: Some("game soft particles"),
                size: 80,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.soft_layout = Some(soft);
        }
    }
    /// Whether any emitter's look fades softly into the scene (soft pipelines).
    pub fn wants_soft(&self) -> bool {
        self.looks.iter().any(|l| l.value.soft > 0.)
    }
    /// Whether this frame draws any soft particle (after `order`).
    pub fn soft_drawn(&self) -> bool {
        self.draws
            .iter()
            .any(|d| matches!(d.kind, Kind::Particle(_, _, true)))
    }
    /// Bind this frame's multisampled scene depth for soft particles (`None`
    /// draws them hard, as without a split translucent pass).
    pub fn soft_pass(
        &mut self,
        d: &wgpu::Device,
        q: &wgpu::Queue,
        depth: Option<&wgpu::TextureView>,
        inverse_view_proj: glam::Mat4,
        split: f32,
    ) {
        self.soft_active = false;
        let (Some(depth), Some(layout), Some(uniform)) =
            (depth, &self.soft_layout, &self.soft_uniform)
        else {
            return;
        };
        let mut words = inverse_view_proj.to_cols_array().to_vec();
        words.extend([split, 0., 0., 0.]);
        q.write_buffer(uniform, 0, bytes(&words));
        if self
            .soft_bind
            .as_ref()
            .is_none_or(|(view, _)| view != depth)
        {
            let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game soft particles"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            self.soft_bind = Some((depth.clone(), bind));
        }
        self.soft_active = true;
    }
    /// The translucent pass is split from the opaque one this frame.
    pub fn soft_active(&self) -> bool {
        self.soft_active
    }
    /// Derive this frame's quads; `cull` skips what the camera cannot see.
    pub fn frame<const ASSETS: bool>(
        &mut self,
        f: &FrameInput<'_>,
        textures: &BTreeMap<String, crate::models::Texture>,
        cull: bool,
    ) {
        self.order.clear();
        self.draws.clear();
        self.particle_data.clear();
        self.sprite_data.clear();
        if let Some(arena) = &mut self.particle_arena {
            arena.words.clear();
        }
        if let Some(arena) = &mut self.sprite_arena {
            arena.words.clear();
        }
        let inv = f.view.inverse();
        let right = inv.x_axis.truncate().normalize();
        let up = inv.y_axis.truncate().normalize();
        // Quads the camera cannot see are neither derived nor uploaded.
        // @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
        let camera = crate::cull::planes(f.proj * f.view);
        let mut left = exact_game::emitter::PARTICLE_BUDGET;
        for item in &self.particles {
            let t = f.displayed_matrix(
                item.entity,
                crate::world::scene::interpolate(item.poses, f.alpha),
            );
            let e = &item.value;
            // World-space particles are not near their emitter: never cull those.
            if cull
                && e.origins.is_empty()
                && !crate::cull::sphere_visible(&camera, t.w_axis.truncate(), emitter_reach(e, &t))
            {
                // Skipped particles still charge the world budget in entity order.
                left -= e.live(self.hz, f.alpha).min(left);
                continue;
            }
            let found = self
                .looks
                .binary_search_by_key(&item.entity.index(), |l| l.entity.index())
                .ok()
                .filter(|&i| self.looks[i].entity == item.entity);
            let look = found.map(|i| &self.looks[i].value);
            // A texture draws only once resident (and only with assets).
            let textured = found.filter(|&i| {
                ASSETS && {
                    let name = &self.looks[i].value.texture;
                    !name.is_empty()
                        && textures
                            .get(name)
                            .is_some_and(|t| t.active && t.sprite_bind.is_some())
                }
            });
            let stretch = look.map_or(0., |l| l.stretch.max(0.));
            let soft = look.map_or(0., |l| l.soft.max(0.));
            let eye = f.camera_position;
            e.particles(self.hz, f.alpha, |p| {
                if left == 0 {
                    return;
                }
                left -= 1;
                let (position, scale) = if p.world {
                    (p.position, Vec2::ONE)
                } else {
                    let scale =
                        Vec2::new(t.x_axis.truncate().length(), t.y_axis.truncate().length());
                    (t.transform_point3(p.position), scale)
                };
                let index = self.particle_data.len();
                let (mut x, mut y) = (right * p.size * scale.x, up * p.size * scale.y);
                let velocity = if p.world {
                    p.velocity
                } else {
                    t.transform_vector3(p.velocity)
                };
                if stretch > 0. && velocity.length_squared() > 1e-12 {
                    // Lengthen along the motion, facing the camera across it.
                    let along = velocity.normalize();
                    let across = along.cross(eye - position).normalize_or(right);
                    y = along * (y.length() + velocity.length() * stretch);
                    x = across * x.length();
                }
                let (uv, mode) = match (textured, look) {
                    (Some(_), Some(l)) => (l.uv(l.frame(p.age, p.lifetime)), 2.),
                    _ => ([0., 0., 1., 1.], 3.),
                };
                let mut color = p.color;
                color[3] *= item.opacity;
                self.particle_data
                    .push(quad(position, x, y, color, uv, mode, soft));
                self.order.push(Order {
                    kind: Kind::Particle(
                        e.additive,
                        textured.map_or(NO_LOOK, |i| i as u32),
                        soft > 0.,
                    ),
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
                let Some(texture) = textures
                    .get(&s.texture)
                    .filter(|texture| texture.active && texture.sprite_bind.is_some())
                else {
                    continue;
                };
                let size = texture.size;
                let t = f.displayed_matrix(
                    item.entity,
                    crate::world::scene::interpolate(item.poses, f.alpha),
                );
                let x = right * (s.size.x * t.x_axis.truncate().length());
                let y = up * (s.size.y * t.y_axis.truncate().length());
                let center = t.w_axis.truncate() + x * (0.5 - s.anchor.x) + y * (0.5 - s.anchor.y);
                let radius = 0.5 * (x.length_squared() + y.length_squared()).sqrt();
                if cull && !crate::cull::sphere_visible(&camera, center, radius) {
                    continue;
                }
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
                let mut color = s.color;
                color[3] *= item.opacity;
                self.sprite_data
                    .push(quad(center, x, y, color, uv, mode, s.cutoff));
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
                    push_draw(
                        &mut self.draws,
                        &self.sprites,
                        &self.looks,
                        Kind::Sprite(index),
                        start,
                    );
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
                Kind::Particle(..) => {
                    let arena = self
                        .particle_arena
                        .as_mut()
                        .expect("particles must prepare before drawing");
                    let at = arena.words.len() / 20;
                    arena.words.extend(self.particle_data[o.index].words);
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
            push_draw(&mut self.draws, &self.sprites, &self.looks, o.kind, at);
        }
        if let Some(arena) = &mut self.sprite_arena {
            arena.upload(d, q);
        }
        if let Some(arena) = &mut self.particle_arena {
            assert!(
                arena.words.len() as u64 * 4 <= arena.buffer.raw.size(),
                "particles must prepare before drawing"
            );
            arena.buffer.write(q, 0, bytes(&arena.words));
        }
    }
    pub fn draw<'a, const ASSETS: bool>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        draw: &Draw,
        textures: &'a BTreeMap<String, crate::models::Texture>,
    ) {
        pass.set_bind_group(0, &self.bind, &[]);
        match draw.kind {
            Kind::Particle(additive, look, soft) => {
                let soft = match (&self.soft_bind, &self.soft_pipelines) {
                    (Some((_, bind)), Some(pipelines)) if soft && self.soft_active => {
                        Some((bind, pipelines))
                    }
                    _ => None,
                };
                if let Some((bind, pipelines)) = soft {
                    let textured = usize::from(look != NO_LOOK);
                    pass.set_pipeline(&pipelines[2 * textured + usize::from(additive)]);
                    if look != NO_LOOK {
                        let texture = &textures[&self.looks[look as usize].value.texture];
                        pass.set_bind_group(1, texture.sprite_bind.as_ref().unwrap(), &[]);
                    }
                    pass.set_bind_group(2, bind, &[]);
                } else if look == NO_LOOK {
                    pass.set_pipeline(
                        &self.particle_pipelines.as_ref().unwrap()[usize::from(additive)],
                    );
                } else {
                    let texture = &textures[&self.looks[look as usize].value.texture];
                    pass.set_pipeline(
                        &self.textured_pipelines.as_ref().unwrap()[usize::from(additive)],
                    );
                    pass.set_bind_group(1, texture.sprite_bind.as_ref().unwrap(), &[]);
                }
                pass.set_vertex_buffer(
                    0,
                    self.particle_arena
                        .as_ref()
                        .expect("particles must prepare before drawing")
                        .buffer
                        .raw
                        .slice(..),
                );
            }
            Kind::Sprite(i) if ASSETS => {
                let s = &self.sprites[i].value;
                let variant = match s.alpha {
                    AlphaMode::Opaque => 0,
                    AlphaMode::Mask => 1,
                    AlphaMode::Blend => 2,
                };
                pass.set_pipeline(&self.sprite_pipelines.as_ref().unwrap()[variant]);
                pass.set_bind_group(1, textures[&s.texture].sprite_bind.as_ref().unwrap(), &[]);
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
        ((self.particle_arena.as_ref().map_or(0, |a| a.words.len())
            + self.sprite_arena.as_ref().map_or(0, |a| a.words.len()))
            / 20) as u64
    }
}
fn push_draw(
    draws: &mut Vec<Draw>,
    sprites: &[Item<Sprite>],
    looks: &[Item<ParticleLook>],
    kind: Kind,
    at: u32,
) {
    if let Some(previous) = draws.last_mut() {
        let compatible = match (previous.kind, kind) {
            (Kind::Particle(a, i, s), Kind::Particle(b, j, t)) => {
                a == b
                    && s == t
                    && (i == j
                        || (i != NO_LOOK
                            && j != NO_LOOK
                            && looks[i as usize].value.texture == looks[j as usize].value.texture))
            }
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
    feed_poses(w, items, initial, next_tick, parent_changed, true);
}

pub(crate) fn feed_poses<T: exact_game::Component + Clone>(
    w: &World,
    items: &mut Vec<Item<T>>,
    initial: bool,
    next_tick: bool,
    parent_changed: bool,
    visible_only: bool,
) {
    let revision = w.revision::<T>();
    // Compact once, then append arrivals. Never shift the tail for each removal.
    items.retain_mut(|i| {
        let Some(value) = w.get::<T>(i.entity) else {
            return false;
        };
        if visible_only && !w.is_visible(i.entity) {
            return false;
        }
        if visible_only {
            // Opacity fades quads; at zero they are not drawn at all.
            i.opacity = opacity(w, i.entity);
            if i.opacity <= 0. {
                return false;
            }
        }
        let Some(pose) = crate::world::scene::pose(w, i.entity) else {
            return false;
        };
        if next_tick {
            i.poses[0] = i.poses[1];
        }
        i.poses[1] = pose;
        if initial || crate::world::scene::snap(w, i.entity, parent_changed) {
            i.poses[0] = pose;
        }
        if initial || i.revision != revision {
            i.value.clone_from(&*value);
            i.revision = revision;
        }
        true
    });
    let existing = items.len();
    let mut retained = 0;
    for (entity, value) in w.query::<&T>().iter() {
        // Both walks use entity order; retained rows need no lookup.
        if retained < existing && items[retained].entity == entity {
            retained += 1;
            continue;
        }
        if visible_only && !w.is_visible(entity) {
            continue;
        }
        let opacity = if visible_only { opacity(w, entity) } else { 1. };
        if opacity <= 0. {
            continue;
        }
        if let Some(pose) = crate::world::scene::pose(w, entity) {
            items.push(Item {
                entity,
                poses: [pose; 2],
                value: value.clone(),
                opacity,
                revision,
            });
        }
    }
    if items.len() != existing {
        items.sort_unstable_by_key(|i| i.entity.index());
    }
}

/// The effective `Opacity` down the Parent chain; 1 without any.
fn opacity(w: &World, mut e: Entity) -> f32 {
    let mut product = 1.;
    // Bounded even if tools edit a cycle before propagation.
    for _ in 0..=w.len() {
        if let Some(o) = w.get::<exact_game::Opacity>(e) {
            product *= exact_game::opacity(o.0);
        }
        match w.get::<exact_game::Parent>(e) {
            Some(p) => e = p.0,
            None => break,
        }
    }
    product
}

// Every particle centre lies within shape + speed·T + |g|·T²/2 of the emitter's
// origin for the longest living birth's T (drag only shortens travel and fall);
// quads extend half a diagonal beyond it.
fn emitter_reach(e: &Emitter, t: &glam::Mat4) -> f32 {
    let lifetime = e.state.births.iter().map(|b| b.lifetime).fold(0., f32::max);
    let shape = match e.shape {
        exact_game::emitter::Shape::Point => 0.,
        exact_game::emitter::Shape::Sphere(r) => r.abs(),
        exact_game::emitter::Shape::Cone(r, h) => (r * r + h * h).sqrt(),
        exact_game::emitter::Shape::Box(size) => size.length() * 0.5,
    };
    let local = shape + e.speed.abs() * lifetime + e.gravity.length() * lifetime * lifetime * 0.5;
    let [x, y, z] = [t.x_axis, t.y_axis, t.z_axis].map(|a| a.truncate().length_squared());
    let size = e.size[0].max(e.size[1]);
    local * (x + y + z).sqrt() + 0.5 * size * (x + y).sqrt()
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
    let layouts: Vec<_> = layouts.iter().map(|l| Some(*l)).collect();
    pipeline_entry(d, shader, &layouts, mode, "quad_fs")
}
fn pipeline_entry(
    d: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    mode: usize,
    fragment: &str,
) -> wgpu::RenderPipeline {
    let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("game quad"),
        bind_group_layouts: layouts,
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
            entry_point: Some(fragment),
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
    #[cfg(not(target_arch = "wasm32"))]
    fn r14_native_children_reserve_order_before_frame() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let q = &mut renderer.quads;
        let plane = crate::placed::Plane {
            placement: exact_gpu::Placement {
                hidden: false,
                homography: [0.; 9],
                depth: 0.,
                clip_depth: [[0.; 3]; 2],
            },
            center: Vec3::ZERO,
            x: Vec3::X,
            y: Vec3::Y,
        };
        q.children = (0..5000).map(|i| (i, plane)).collect();
        q.prepare(&gpu.device, &gpu.queue);
        let capacity = q.order.capacity();
        q.frame::<false>(&FrameInput::default(), &BTreeMap::new(), true);
        assert_eq!(q.order.len(), 5000);
        assert_eq!(q.order.capacity(), capacity);
    }
    #[test]
    fn emitter_reach_contains_every_derived_particle_quad() {
        let mut w = World::new(60, 3);
        let shapes = [
            exact_game::emitter::Shape::Point,
            exact_game::emitter::Shape::Sphere(1.5),
            exact_game::emitter::Shape::Cone(0.7, 2.),
        ];
        let mut ids = Vec::new();
        for (i, shape) in shapes.into_iter().enumerate() {
            for drag in [0., 0.8] {
                ids.push(w.spawn(Emitter {
                    shape,
                    drag,
                    speed: 4. + i as f32,
                    spread: 2.5,
                    gravity: Vec3::new(1., -9.81, 0.5),
                    size: [0.3, 0.9],
                    ..Emitter::sparks().rate(200.).lifetime(1.5).seed(i as u64)
                }));
            }
        }
        for _ in 0..120 {
            exact_game::emitter::step(&w);
        }
        let t = glam::Mat4::from_scale_rotation_translation(
            Vec3::new(2., 0.5, 1.),
            glam::Quat::from_rotation_z(0.7),
            Vec3::new(3., 1., -2.),
        );
        let (right, up) = (Vec3::X, Vec3::Y);
        for id in ids {
            let e = w.get::<Emitter>(id).unwrap();
            let reach = emitter_reach(&e, &t);
            let mut seen = 0;
            for alpha in [0., 0.5, 1.] {
                e.particles(60, alpha, |p| {
                    seen += 1;
                    let centre = t.transform_point3(p.position);
                    let x = right * p.size * t.x_axis.truncate().length();
                    let y = up * p.size * t.y_axis.truncate().length();
                    for corner in [x + y, x - y, -x + y, -x - y] {
                        let d = (centre + corner * 0.5).distance(t.w_axis.truncate());
                        assert!(d <= reach * 1.0001, "{d} beyond reach {reach}");
                    }
                });
            }
            assert!(seen > 100);
        }
    }
    #[test]
    fn every_kind_and_owner_ordinal_has_the_same_total_order() {
        let mut kinds = vec![
            Kind::Particle(false, NO_LOOK, false),
            Kind::Model(0, 0),
            Kind::Sprite(0),
        ];
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
        assert_eq!(Kind::Particle(false, NO_LOOK, false).rank(), 2);
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
