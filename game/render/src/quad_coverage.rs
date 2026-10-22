//! A look frame's quads (LLP 1127.000 D1, D4): what `Quads::frame` did with each
//! sprite and emitter, each quad draw's batch, and the particle counts.
//!
//! The counts come from the particle draws themselves. While a look frame is
//! armed, each particle draw uses the probe variant of its own pipeline
//! (`shaders/count.wgsl`): the same vertex path, the same dot shape, texture
//! alpha and soft fade, blended into the same target as ever (the HDR image,
//! or a direct frame's target), and each fragment that contributes to the
//! frame added to its draw's counter in a storage buffer. A fragment
//! contributes when its contribution is above zero (alpha, times the brightest
//! channel for an additive draw) and it is in front of the scene's depth at one
//! of the samples it covers; the depth compared is the pixel centre's, and a
//! contribution too small to change an 8-bit pixel still counts. A particle
//! draw that is not submitted, or whose fragments are all transparent, counts
//! nothing. The shader reads the scene's depth, so the frame draws its
//! translucency in a pass of its own over the same attachments, as soft
//! particles do. A live frame makes no such pipeline and binds none. GL has no
//! fragment storage writes: there the counts are unmeasured.
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

/// The pipeline a quad draw bound (`Quads::bind`): `8 × direct +` its kind's
/// index (a particle's `4 × soft + 2 × textured + additive`, a sprite's alpha
/// mode, 0 for a blended primitive, 4 for a placed child), and whether it writes depth.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QuadPipeline {
    pub variant: usize,
    pub depth: bool,
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
/// `8 * direct + 4 * soft + 2 * textured + additive`.
#[derive(Default)]
pub(super) struct CountPipelines {
    layout: Option<wgpu::BindGroupLayout>,
    pipelines: [Option<wgpu::RenderPipeline>; 16],
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
            Kind::Particle(additive, look, soft, _) => {
                let what = match self.looks.get(look as usize).filter(|_| look != NO_LOOK) {
                    Some(l) => format!("particles {}", l.value.texture),
                    None => "particle dots".into(),
                };
                let blend = if additive { "additive" } else { "alpha" };
                format!("{what} · {blend}{}", if soft { " · soft" } else { "" })
            }
            Kind::Primitive(_) => "blended primitives".into(),
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

    /// Draw `index`'s probe record: its run of quads and the pipeline `bind`
    /// bound for it.
    pub(crate) fn record(
        &self,
        index: usize,
        pipeline: QuadPipeline,
    ) -> crate::renderer::probe::Drew {
        crate::renderer::probe::Drew::Quads {
            draw: index,
            count: self.draws[index].range.len() as u32,
            variant: pipeline.variant,
            depth: pipeline.depth,
            triangles: matches!(self.draws[index].kind, Kind::Primitive(_)),
        }
    }

    /// Arm this frame's particle counting over `depth`, the scene's depth the
    /// split translucent pass reads, after `prepare` and `order`; a `direct`
    /// frame's count pipelines draw into its target as its own do. `Err` names
    /// why the device cannot count.
    pub(crate) fn begin_count(
        &mut self,
        d: &wgpu::Device,
        depth: &wgpu::TextureView,
        direct: bool,
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
            let Kind::Particle(additive, look, soft, _) = self.draws[i].kind else {
                unreachable!()
            };
            let soft = soft && self.soft_active;
            self.count_pipeline(d, direct, soft, look != NO_LOOK, additive);
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
            // Each draw's slot: its counter, and whether it adds (count.wgsl).
            let mut bytes = vec![0u8; draws.len() * STRIDE as usize];
            for (k, &i) in draws.iter().enumerate() {
                let at = k * STRIDE as usize;
                let additive = matches!(self.draws[i].kind, Kind::Particle(true, ..));
                bytes[at..at + 4].copy_from_slice(&(k as u32).to_le_bytes());
                bytes[at + 4..at + 8].copy_from_slice(&u32::from(additive).to_le_bytes());
            }
            (which.slice(..).get_mapped_range_mut())
                .map_err(|_| "the counters' draw slots did not map")?
                .copy_from_slice(&bytes);
        }
        which.unmap();
        use count_interface::particle_count_4 as count;
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game look particle counts"),
            layout: self.counted.layout.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: count::COVERED.binding,
                    resource: counts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: count::COUNTED.binding,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &which,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: count::COUNT_DEPTH.binding,
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

    /// Forget a look frame's counting and observations.
    pub(crate) fn disarm(&mut self) {
        self.counting = None;
        self.observed = None;
    }

    /// Whether a look frame's counting is armed.
    #[cfg(test)]
    pub(crate) fn counting_armed(&self) -> bool {
        self.counting.is_some()
    }

    /// The counters and the quad draws they count; the next frame counts none.
    pub(crate) fn take_counts(&mut self) -> Option<(wgpu::Buffer, Vec<usize>)> {
        self.counting.take().map(|c| (c.counts, c.draws))
    }

    /// While counting: quad draw `index`'s probe variant, and its counter slot
    /// bound as group 3 (`Quads::bind`, after its own groups).
    pub(super) fn bind_count<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        index: usize,
        direct: bool,
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
        let variant = count_variant(direct, soft, textured, additive);
        pass.set_pipeline(self.counted.pipelines[variant].as_ref().unwrap());
        pass.set_bind_group(3, &counting.bind, &[(u64::from(slot) * STRIDE) as u32]);
    }

    fn count_pipeline(
        &mut self,
        d: &wgpu::Device,
        direct: bool,
        soft: bool,
        textured: bool,
        additive: bool,
    ) {
        let variant = count_variant(direct, soft, textured, additive);
        if self.counted.pipelines[variant].is_some() {
            return;
        }
        let samples = self.samples;
        let layout = self
            .counted
            .layout
            .get_or_insert_with(|| count_layout(d, samples));
        let base = if textured { "sprite" } else { "particle" };
        let (kind, entry) = if soft {
            let entry = count_interface::particle_soft_count_4::entry::QUAD_SOFT_COUNT_FS;
            ("soft_count", entry)
        } else {
            (
                "count",
                count_interface::particle_count_4::entry::QUAD_COUNT_FS,
            )
        };
        let shader = module(d, crate::pipeline::quad_module(base, kind, samples));
        let texture = textured.then(|| self.texture_layout.as_ref().unwrap());
        let soft_layout = soft.then(|| self.soft_layout.as_ref().unwrap());
        self.counted.pipelines[variant] = Some(pipeline_entry(
            d,
            self.cache.as_ref(),
            &shader,
            &[Some(&self.layout), texture, soft_layout, Some(&*layout)],
            usize::from(additive) + 2,
            entry,
            (samples, direct.then_some(self.format)),
        ));
    }
}

fn count_variant(direct: bool, soft: bool, textured: bool, additive: bool) -> usize {
    8 * usize::from(direct)
        + 4 * usize::from(soft)
        + 2 * usize::from(textured)
        + usize::from(additive)
}

/// The probe variants' counter group, as build.rs reflected it from the
/// assembled modules (all of which declare it alike): the draw's slot is
/// bound at a dynamic offset, and the depth is multisampled as the scene is.
fn count_layout(d: &wgpu::Device, samples: u32) -> wgpu::BindGroupLayout {
    use count_interface::particle_count_4 as count;
    let mut entries = [count::COVERED, count::COUNTED, count::COUNT_DEPTH];
    if let wgpu::BindingType::Buffer {
        has_dynamic_offset, ..
    } = &mut entries[1].ty
    {
        *has_dynamic_offset = true;
    }
    if let wgpu::BindingType::Texture { multisampled, .. } = &mut entries[2].ty {
        *multisampled = samples > 1;
    }
    d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("game look particle counts"),
        entries: &entries,
    })
}

#[allow(dead_code)]
mod count_interface {
    include!(concat!(env!("OUT_DIR"), "/count_interface.rs"));
}
