//! A look frame's quads (LLP 1127.000 D1, D4): what `Quads::frame` did with each
//! sprite and emitter, each quad draw's batch, and the particle counts.
//!
//! The counts come from the particle draws themselves. While a look frame is
//! armed, each particle draw uses the probe variant of its own pipeline
//! (`shaders/count.wgsl`): the same vertex path, the same dot shape, texture
//! alpha and soft fade, blended into the frame as ever, and each fragment in
//! front of the scene that moves an 8-bit pixel (alpha at least 1/255) added
//! to its draw's counter in a storage buffer. A particle draw that is not
//! submitted, or whose fragments are all transparent, counts nothing. The
//! shader reads the scene's depth, so the frame draws its translucency in its
//! own pass, as soft particles do. A live frame makes no such pipeline and
//! binds none. GL has no fragment storage writes: there the counts are
//! unmeasured.
use super::*;

/// What a look frame's quads did with one sprite or emitter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum QuadFact {
    /// Outside the camera's view, culled on the CPU.
    Culled,
    /// Its texture is not resident: the sprite was not drawn.
    Pending(String),
    /// Drawn: the quads it drew this frame (a particle count, or 1).
    Drawn(u32),
}

/// A look frame's particle counting: its counters, the bind group of
/// counters, draw slots and scene depth, and each particle draw's slot.
pub(super) struct Counting {
    counts: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Each quad draw's counter, for the particle draws.
    slots: Vec<Option<u32>>,
    draws: Vec<usize>,
}

/// The probe variants of the particle pipelines, made on first use: index
/// `4 * soft + 2 * textured + additive`.
#[derive(Default)]
pub(super) struct CountPipelines {
    layout: Option<wgpu::BindGroupLayout>,
    pipelines: [Option<wgpu::RenderPipeline>; 8],
}

/// One counter word per draw, each draw's slot at a dynamic uniform offset.
const STRIDE: u64 = 256;

/// Whether `d` can count: fragment shaders that write storage, which GL lacks.
pub(crate) fn counts(d: &wgpu::Device) -> bool {
    d.adapter_info().backend != wgpu::Backend::Gl
}

impl Quads {
    /// Draw `index`'s batch: what it shares with every quad drawn with it.
    pub(crate) fn batch_label(&self, index: usize) -> String {
        let Some(draw) = self.draws.get(index) else {
            return "?".into();
        };
        match draw.kind {
            Kind::Particle(additive, look, soft) => {
                let what = match self.looks.get(look as usize).filter(|_| look != NO_LOOK) {
                    Some(l) => format!("particles {}", l.value.texture),
                    None => "particle dots".into(),
                };
                let blend = if additive { "additive" } else { "alpha" };
                format!("{what} · {blend}{}", if soft { " · soft" } else { "" })
            }
            Kind::Sprite(i) => format!(
                "sprite {}",
                self.sprites
                    .get(i)
                    .map_or("?", |s| s.value.texture.as_str())
            ),
            Kind::Model(batch, _) => format!("blended model · batch {batch}"),
            #[cfg(not(target_arch = "wasm32"))]
            Kind::Child(i) => format!("placed child {i}"),
        }
    }

    /// Arm this frame's particle counting over `depth`, the scene's depth the
    /// split translucent pass reads, after `prepare` and `order`. `Err` names
    /// why the device cannot count.
    pub(crate) fn begin_count(
        &mut self,
        d: &wgpu::Device,
        depth: &wgpu::TextureView,
    ) -> Result<(), &'static str> {
        if !counts(d) {
            return Err("this backend's fragment shaders cannot write storage");
        }
        let draws: Vec<usize> = (self.draws.iter().enumerate())
            .filter(|(_, draw)| matches!(draw.kind, Kind::Particle(..)))
            .map(|(i, _)| i)
            .collect();
        if draws.is_empty() {
            return Ok(());
        }
        for &i in &draws {
            let Kind::Particle(additive, look, soft) = self.draws[i].kind else {
                unreachable!()
            };
            let soft = soft && self.soft_active;
            self.count_pipeline(d, soft, look != NO_LOOK, additive);
        }
        let mut slots = vec![None; self.draws.len()];
        for (k, &i) in draws.iter().enumerate() {
            slots[i] = Some(k as u32);
        }
        let counts = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game look particle counts"),
            size: draws.len() as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let which = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game look particle draw"),
            size: draws.len() as u64 * STRIDE,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        {
            let mut bytes = vec![0u8; draws.len() * STRIDE as usize];
            for k in 0..draws.len() {
                let at = k * STRIDE as usize;
                bytes[at..at + 4].copy_from_slice(&(k as u32).to_le_bytes());
            }
            (which.slice(..).get_mapped_range_mut())
                .map_err(|_| "the counters' draw slots did not map")?
                .copy_from_slice(&bytes);
        }
        which.unmap();
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game look particle counts"),
            layout: self.counted.layout.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: counts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &which,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
            ],
        });
        self.counting = Some(Counting {
            counts,
            bind,
            slots,
            draws,
        });
        Ok(())
    }

    /// Whether this frame draws any particle (after `order`).
    pub(crate) fn particles_drawn(&self) -> bool {
        self.draws
            .iter()
            .any(|d| matches!(d.kind, Kind::Particle(..)))
    }

    /// The counters and the quad draws they count; the next frame counts none.
    pub(crate) fn take_counts(&mut self) -> Option<(wgpu::Buffer, Vec<usize>)> {
        self.counting.take().map(|c| (c.counts, c.draws))
    }

    /// While counting: quad draw `index`'s probe variant, and its counter slot
    /// bound as group 3 (`Quads::draw`, after its own groups).
    pub(super) fn bind_count<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        index: usize,
        soft: bool,
        textured: bool,
        additive: bool,
    ) {
        let Some(counting) = &self.counting else {
            return;
        };
        let Some(slot) = counting.slots.get(index).copied().flatten() else {
            return;
        };
        let variant = 4 * usize::from(soft) + 2 * usize::from(textured) + usize::from(additive);
        pass.set_pipeline(self.counted.pipelines[variant].as_ref().unwrap());
        pass.set_bind_group(3, &counting.bind, &[(u64::from(slot) * STRIDE) as u32]);
    }

    fn count_pipeline(&mut self, d: &wgpu::Device, soft: bool, textured: bool, additive: bool) {
        let variant = 4 * usize::from(soft) + 2 * usize::from(textured) + usize::from(additive);
        if self.counted.pipelines[variant].is_some() {
            return;
        }
        let samples = self.samples;
        let layout = self
            .counted
            .layout
            .get_or_insert_with(|| count_layout(d, samples));
        let piece = if textured {
            crate::pipeline::SPRITE
        } else {
            crate::pipeline::PARTICLE
        };
        let count = crate::pipeline::depth_source(crate::pipeline::COUNT, samples);
        let (source, entry) = if soft {
            let soft = crate::pipeline::depth_source(crate::pipeline::SOFT, samples);
            let parts = [piece, &soft, &count, crate::pipeline::SOFT_COUNT];
            (crate::pipeline::quad_source(&parts), "quad_soft_count_fs")
        } else {
            (
                crate::pipeline::quad_source(&[piece, &count]),
                "quad_count_fs",
            )
        };
        let shader = module(d, &source);
        let texture = textured.then(|| self.texture_layout.as_ref().unwrap());
        let soft_layout = soft.then(|| self.soft_layout.as_ref().unwrap());
        self.counted.pipelines[variant] = Some(pipeline_entry(
            d,
            self.cache.as_ref(),
            &shader,
            &[Some(&self.layout), texture, soft_layout, Some(&*layout)],
            usize::from(additive) + 2,
            entry,
            (samples, None),
        ));
    }
}

fn count_layout(d: &wgpu::Device, samples: u32) -> wgpu::BindGroupLayout {
    d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("game look particle counts"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: samples > 1,
                },
                count: None,
            },
        ],
    })
}
