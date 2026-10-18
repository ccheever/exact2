//! What a look frame reads back and what it knows of the frame it encoded
//! (LLP 1127.000 D1, D4): the colour target, the particle counts, the
//! occlusion queries, the cull's account of each item, and for culled frames
//! the camera's, cascades' and local shadow layers' indirect arguments and the
//! camera's compacted lists.
use super::*;

/// One cull group as the look frame drew it.
#[derive(Clone, Debug)]
pub(crate) struct GroupFact {
    /// Its batch, and the batch's slot-list range.
    pub batch: usize,
    pub range: std::ops::Range<u32>,
    /// Views it is tested in (bit 0 the camera, bits 1–3 the cascades).
    pub views: u32,
    /// Kept whole: a game's custom material, which the GPU cull never tests.
    pub keep_all: bool,
    pub mirrored: bool,
    /// Its byte offsets in the compacted lists and the indirect arguments, per view.
    pub offsets: [(u32, u64); crate::cull::VIEWS as usize],
    /// Its first 64-item chunk in the cull's dispatch (`Cull::begin_setup`).
    pub chunk: u32,
}

fn group_facts(cull: &crate::cull::Cull) -> Vec<GroupFact> {
    let mut chunk = 0;
    (cull.groups.iter().enumerate())
        .map(|(index, g)| {
            let fact = GroupFact {
                batch: g.batch,
                range: g.range.clone(),
                views: g.flags & 0xf,
                keep_all: g.flags & crate::cull::KEEP_ALL != 0,
                mirrored: g.mirrored,
                offsets: std::array::from_fn(|v| cull.offsets(v as u32, index)),
                chunk,
            };
            chunk += g.range.len().div_ceil(64) as u32;
            fact
        })
        .collect()
}
/// Bytes of a cull's indirect arguments that hold its groups' draws.
fn indirect_bytes(cull: &crate::cull::Cull) -> u64 {
    (u64::from(crate::cull::VIEWS) * cull.groups.len() as u64 * 20).min(cull.indirect.raw.size())
}

/// One local shadow cull as the look frame drew it.
#[derive(Clone, Debug, Default)]
pub(crate) struct LocalFact {
    pub groups: Vec<GroupFact>,
    /// Bytes of its indirect arguments the readback copies (0 in a direct frame).
    pub indirect: u64,
}

/// What a look frame's readback needs to know of the frame it encoded.
#[derive(Clone, Debug, Default)]
pub(crate) struct Facts {
    pub groups: Vec<GroupFact>,
    /// Every group drew directly, unculled: no lists or indirect counts exist.
    pub direct: bool,
    /// Sun shadow cascades drawn (0 without a shadowed sun).
    pub cascades: u32,
    /// The local shadow culls, by the index local records name.
    pub local: Vec<LocalFact>,
    pub slot_list: Vec<u32>,
    /// Each model record's transform slot, to find an entity's model instances.
    pub record_slots: Vec<u32>,
    /// Bytes of the compacted lists and of the indirect arguments the readback copies.
    pub lists: u64,
    pub indirect: u64,
    /// Each quad draw's batch, by its index in the frame's list.
    pub quad_labels: Vec<String>,
    /// The frame's particle draws (index in the quad list, particles drawn),
    /// and the ones counted, in the counts' order.
    pub particle_draws: Vec<(usize, u32)>,
    pub counted: Vec<usize>,
    /// Why the particle draws went uncounted, if they did.
    pub uncounted: Option<&'static str>,
    /// What the quads did with each sprite and emitter (`Quads::observed`).
    pub quads: Vec<(exact_game::Entity, crate::quads::QuadFact)>,
    /// Each model record's box in its entity's space (`model_boxes`).
    pub model_boxes: Vec<(u32, crate::look_bounds::Aabb)>,
    /// Each blended model instance's slot and whether the CPU cull kept it:
    /// they draw one by one in depth order, outside the draw groups.
    pub blended: Vec<(u32, bool)>,
}

/// The buffers one look frame reads back, mapped together.
pub(crate) struct Readback {
    parts: Vec<(Part, wgpu::Buffer)>,
    mapped: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    failed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Bytes copied for reading, for the frame's cost.
    pub bytes: u64,
}
#[derive(Clone, Copy)]
enum Part {
    Pixels {
        row: u32,
        width: u32,
        height: u32,
        bgra: bool,
    },
    Queries(u32),
    Counts(u32),
    Indirect,
    Lists,
    LocalIndirect(usize),
    Cull,
}

/// A look frame's readback, read.
#[derive(Default)]
pub(crate) struct Reading {
    /// Tightly packed RGBA rows, top down; empty for a frame read for its bounds.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Per query: whether any sample passed.
    pub covered: Vec<bool>,
    /// Per counted particle draw: its fragments in front of the scene that
    /// move an 8-bit pixel.
    pub counts: Vec<u32>,
    /// The indirect arguments as words.
    pub indirect: Vec<u32>,
    /// The compacted lists as words.
    pub lists: Vec<u32>,
    /// Each local shadow cull's indirect arguments as words.
    pub local_indirect: Vec<Vec<u32>>,
    /// The cull's account of each item, `CULL_WORDS` words an item.
    pub cull: Vec<u32>,
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// The frame just encoded, as the look frame's answers read it.
    pub(crate) fn probe_facts(&mut self) -> Facts {
        let groups = group_facts(&self.cull);
        let lists = (groups.iter())
            .flat_map(|g| {
                g.offsets
                    .iter()
                    .map(move |o| u64::from(o.0) + u64::from(g.range.len() as u32) * 4)
            })
            .max()
            .unwrap_or(0)
            .min(self.cull.compacted.raw.size());
        let direct = self.cull.direct || groups.is_empty();
        let layers = self.local_plan.views.len().div_ceil(4);
        Facts {
            direct,
            cascades: self.shadows.as_ref().map_or(0, |s| s.count),
            local: (self.local_culls.iter().take(layers))
                .map(|l| LocalFact {
                    groups: group_facts(&l.cull),
                    indirect: if self.cull.direct {
                        0
                    } else {
                        indirect_bytes(&l.cull)
                    },
                })
                .collect(),
            slot_list: self.slot_list.clone(),
            record_slots: if ASSETS {
                self.models.records.iter().map(|r| r.transform).collect()
            } else {
                Vec::new()
            },
            lists,
            indirect: indirect_bytes(&self.cull),
            groups,
            quad_labels: (0..self.quads.draws.len())
                .map(|i| self.quads.batch_label(i))
                .collect(),
            particle_draws: (self.quads.draws.iter().enumerate())
                .filter(|(_, d)| matches!(d.kind, crate::quads::Kind::Particle(..)))
                .map(|(i, d)| (i, d.range.end - d.range.start))
                .collect(),
            counted: (self.probe.as_ref())
                .and_then(|p| p.counted.as_ref())
                .map_or_else(Vec::new, |(_, draws)| draws.clone()),
            uncounted: self.probe.as_ref().and_then(|p| p.uncounted),
            quads: self.quads.observed.take().unwrap_or_default(),
            model_boxes: self.model_boxes(),
            blended: (self.models.transparent.iter())
                .filter_map(|&(_, slot, depth)| {
                    Some((*self.slot_list.get(slot as usize)?, !depth.is_nan()))
                })
                .collect(),
        }
    }

    /// Each model record's mesh box in its entity's space, by entity index.
    pub(crate) fn model_boxes(&self) -> Vec<(u32, crate::look_bounds::Aabb)> {
        if !ASSETS {
            return Vec::new();
        }
        (self.models.records.iter())
            .filter_map(|r| {
                let mesh = self.meshes.get(r.geometry.0)?;
                let local = (mesh.center - mesh.half, mesh.center + mesh.half);
                Some((r.transform, crate::look_bounds::transformed(r.local, local)))
            })
            .collect()
    }

    /// Copy what the look frame reads back: `target`'s pixels unless the
    /// frame is read only for its bounds, the particle counts, the query
    /// results, the cull's account and, for a culled frame, the indirect
    /// arguments and the camera's compacted lists. Takes the probe; the next
    /// frame carries none.
    pub(crate) fn probe_copies(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        facts: &Facts,
    ) -> Readback {
        let probe = self.probe.take();
        let device = &self.device;
        let buffer = |size: u64, label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(4).next_multiple_of(4),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };
        let mut parts = Vec::new();
        if probe.as_ref().is_none_or(|p| p.pixels) {
            let (width, height) = (target.width(), target.height());
            let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
            let pixels = buffer(u64::from(row) * u64::from(height), "game look pixels");
            encoder.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &pixels,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
                        rows_per_image: None,
                    },
                },
                target.size(),
            );
            let bgra = matches!(
                target.format(),
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            );
            parts.push((
                Part::Pixels {
                    row,
                    width,
                    height,
                    bgra,
                },
                pixels,
            ));
        }
        let mut copy = |part, source: &wgpu::Buffer, size: u64, label| {
            let read = buffer(size, label);
            encoder.copy_buffer_to_buffer(source, 0, &read, 0, size);
            parts.push((part, read));
        };
        if let Some((counts, draws)) = probe.as_ref().and_then(|p| p.counted.as_ref()) {
            let n = draws.len() as u32;
            copy(
                Part::Counts(n),
                counts,
                u64::from(n) * 4,
                "game look particle coverage",
            );
        }
        if let Some(cull) = probe.as_ref().and_then(|p| p.cull.as_ref()) {
            copy(Part::Cull, cull, cull.size(), "game look cull account");
        }
        if !facts.direct && facts.indirect > 0 {
            copy(
                Part::Indirect,
                &self.cull.indirect.raw,
                facts.indirect,
                "game look indirect",
            );
            if facts.lists > 0 {
                copy(
                    Part::Lists,
                    &self.cull.compacted.raw,
                    facts.lists,
                    "game look lists",
                );
            }
        }
        for (i, local) in facts
            .local
            .iter()
            .enumerate()
            .filter(|(_, l)| l.indirect > 0)
        {
            let source = &self.local_culls[i].cull.indirect.raw;
            copy(
                Part::LocalIndirect(i),
                source,
                local.indirect,
                "game look local indirect",
            );
        }
        if let Some(probe) = probe.filter(|p| p.used() > 0) {
            let n = probe.used();
            let resolve = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("game look coverage resolve"),
                size: u64::from(n) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            encoder.resolve_query_set(probe.queries.as_ref().unwrap(), 0..n, &resolve, 0);
            let read = buffer(u64::from(n) * 8, "game look coverage");
            encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, u64::from(n) * 8);
            parts.push((Part::Queries(n), read));
        }
        let bytes = parts.iter().map(|(_, b)| b.size()).sum();
        Readback {
            parts,
            mapped: Default::default(),
            failed: Default::default(),
            bytes,
        }
    }
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// Each batch's mesh or model and material, for the draw listing; `shapes`
    /// names the primitive meshes (`Feed::shape_names`).
    pub(crate) fn probe_labels(&self, shapes: &[(MeshId, String)]) -> Vec<String> {
        (self.batches.iter().enumerate())
            .map(|(b, batch)| {
                let level = if batch.level > 0 {
                    format!(" lod {}", batch.level)
                } else {
                    String::new()
                };
                if let Some(material) = self.model_material(b) {
                    let model = (self.models.loaded.iter())
                        .find(|(_, m)| m.meshes.contains(&batch.mesh))
                        .map_or("?", |(name, _)| name.as_str());
                    return format!("model {model} · material {}{level}", material.0);
                }
                let shape = shapes
                    .iter()
                    .find(|(m, _)| *m == batch.mesh)
                    .map_or("mesh", |(_, n)| n);
                format!("{shape} · material per instance{level}")
            })
            .collect()
    }
}

impl Readback {
    /// Map every part after the copies are submitted.
    pub fn map(&self) {
        for (_, buffer) in &self.parts {
            let (mapped, failed) = (self.mapped.clone(), self.failed.clone());
            buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                failed.fetch_or(r.is_err(), std::sync::atomic::Ordering::SeqCst);
                mapped.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
    }
    /// Whether every part is mapped; natively this waits for the GPU. On the
    /// web the maps land on the page's event loop, between agent requests.
    pub fn ready(&self, device: &wgpu::Device) -> Result<bool, String> {
        #[cfg(not(target_arch = "wasm32"))]
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("look readback: {e:?}"))?;
        #[cfg(target_arch = "wasm32")]
        let _ = device;
        if self.failed.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("look readback: a map failed".into());
        }
        Ok(self.mapped.load(std::sync::atomic::Ordering::SeqCst) == self.parts.len())
    }
    pub fn read(self) -> Result<Reading, String> {
        let mut out = Reading::default();
        for (part, buffer) in &self.parts {
            let data = (buffer.slice(..).get_mapped_range())
                .map_err(|e| format!("look readback: {e:?}"))?;
            let words = || {
                data.chunks_exact(4)
                    .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
                    .collect::<Vec<_>>()
            };
            match *part {
                Part::Pixels {
                    row,
                    width,
                    height,
                    bgra,
                } => {
                    out.width = width;
                    out.height = height;
                    out.rgba.reserve((width * height * 4) as usize);
                    for y in 0..height as usize {
                        let line = &data[y * row as usize..][..width as usize * 4];
                        if bgra {
                            out.rgba.extend(
                                line.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]),
                            );
                        } else {
                            out.rgba.extend_from_slice(line);
                        }
                    }
                }
                Part::Queries(n) => {
                    out.covered = (data.chunks_exact(8).take(n as usize))
                        .map(|q| u64::from_le_bytes(q.try_into().unwrap()) != 0)
                        .collect();
                }
                Part::Counts(n) => out.counts = words().into_iter().take(n as usize).collect(),
                Part::Indirect => out.indirect = words(),
                Part::Lists => out.lists = words(),
                Part::LocalIndirect(i) => {
                    out.local_indirect.resize(i + 1, Vec::new());
                    out.local_indirect[i] = words();
                }
                Part::Cull => out.cull = words(),
            }
            drop(data);
            buffer.unmap();
        }
        Ok(out)
    }
}
