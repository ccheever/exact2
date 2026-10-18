//! A look frame's instruments (LLP 1127.000 D1, D4): a record of each draw as it
//! is encoded, an occlusion query around each draw of the forward and
//! translucent passes, and copies of what the cull kept. Armed only for a look
//! frame under the agent's clock; a live frame carries none of it and pays one
//! `Option` check per draw.
//!
//! An occlusion query answers whether any sample of its draws passed the depth
//! test, not how many: Metal's visibility results are Boolean in wgpu
//! (vendor/wgpu-hal/src/metal/command.rs), and WebGPU promises only zero or not
//! zero (https://www.w3.org/TR/webgpu/#occlusion). So a draw reads as covered or
//! not, which is what the tells need.
use super::*;
use std::cell::{Cell, RefCell};

/// WebGPU's largest query set.
const MAX_QUERIES: u32 = 4096;

/// The pass a draw belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Last frame's camera lists drawn as occluders, depth only (occlusion.rs).
    Occluders,
    /// Sun shadow cascade `n`, depth only.
    SunShadow(u8),
    /// The opaque depth prepass.
    Prepass,
    /// The forward pass: opaque geometry, the sky, hulls, then translucency
    /// unless a split pass draws it.
    Forward,
    /// Translucency in its own pass (soft particles, a scene copy).
    Translucent,
}
impl Stage {
    pub fn name(self) -> String {
        match self {
            Stage::Occluders => "occluders".into(),
            Stage::SunShadow(n) => format!("sun shadow {n}"),
            Stage::Prepass => "depth prepass".into(),
            Stage::Forward => "forward".into(),
            Stage::Translucent => "translucent".into(),
        }
    }
    /// Passes begun with the probe's query set.
    fn queried(self) -> bool {
        matches!(self, Stage::Forward | Stage::Translucent)
    }
}

/// What one draw drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drew {
    /// Culled draw group `group` in view `view` (0 the camera, 1–3 the
    /// cascades), with the pipeline variant the forward pass chose for it.
    Group {
        group: usize,
        view: u32,
        variant: Option<usize>,
    },
    /// A run of `count` quads: particles, sprites or a placed child.
    Quads { kind: &'static str, count: u32 },
    /// One instance of a blended model, drawn in translucent order.
    Blended { batch: usize },
    /// A draw that is not an instanced batch: the sky, outline hulls.
    Fixed(&'static str),
}

/// One recorded draw and its occlusion query, if its pass carried one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Record {
    pub stage: Stage,
    pub drew: Drew,
    pub query: Option<u32>,
}

pub(crate) struct Probe {
    pub records: RefCell<Vec<Record>>,
    stage: Cell<Stage>,
    variant: Cell<Option<usize>>,
    pub queries: wgpu::QuerySet,
    capacity: u32,
    next: Cell<u32>,
}

impl Probe {
    /// Room for `draws` queried draws (at most WebGPU's 4096; later draws are
    /// recorded without a query).
    pub fn new(device: &wgpu::Device, draws: usize) -> Self {
        let capacity = (draws as u32).clamp(1, MAX_QUERIES);
        Self {
            records: RefCell::new(Vec::new()),
            stage: Cell::new(Stage::Occluders),
            variant: Cell::new(None),
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("game look coverage"),
                ty: wgpu::QueryType::Occlusion,
                count: capacity,
            }),
            capacity,
            next: Cell::new(0),
        }
    }
    pub fn stage(&self, stage: Stage) {
        self.stage.set(stage);
    }
    /// The pipeline variant of the next group the forward pass draws.
    pub fn variant(&self, variant: usize) {
        self.variant.set(Some(variant));
    }
    /// The query set for a pass of `stage`: the forward and translucent passes'.
    pub fn set_for(&self, stage: Stage) -> Option<&wgpu::QuerySet> {
        stage.queried().then_some(&self.queries)
    }
    /// Record a draw about to be issued in `pass`, beginning its occlusion
    /// query when the pass carries the set. Pair with [`Probe::end`].
    pub fn begin(&self, pass: &mut wgpu::RenderPass<'_>, mut drew: Drew) -> bool {
        let stage = self.stage.get();
        if let Drew::Group { variant, .. } = &mut drew {
            *variant = self.variant.take();
        }
        let query = (stage.queried() && self.next.get() < self.capacity).then(|| {
            let index = self.next.get();
            self.next.set(index + 1);
            pass.begin_occlusion_query(index);
            index
        });
        self.records
            .borrow_mut()
            .push(Record { stage, drew, query });
        query.is_some()
    }
    pub fn end(pass: &mut wgpu::RenderPass<'_>, began: bool) {
        if began {
            pass.end_occlusion_query();
        }
    }
    /// Record a draw without a query.
    pub fn note(&self, drew: Drew) {
        let stage = self.stage.get();
        self.records.borrow_mut().push(Record {
            stage,
            drew,
            query: None,
        });
    }
    /// Queries written this frame.
    pub fn used(&self) -> u32 {
        self.next.get()
    }
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// Arm the next encode's instruments.
    pub(crate) fn arm_probe(&mut self) {
        let draws = self.cull.groups.len() + self.quads.draws.len() + self.batches.len() + 8;
        self.probe = Some(Probe::new(&self.device, draws));
    }
    pub(crate) fn probe_stage(&self, stage: Stage) {
        if let Some(probe) = &self.probe {
            probe.stage(stage);
        }
    }
    /// The query set a pass of `stage` is begun with, while armed.
    pub(crate) fn probe_queries(&self, stage: Stage) -> Option<&wgpu::QuerySet> {
        self.probe.as_ref().and_then(|p| p.set_for(stage))
    }
    pub(crate) fn probe_begin(&self, pass: &mut wgpu::RenderPass<'_>, drew: Drew) -> bool {
        self.probe.as_ref().is_some_and(|p| p.begin(pass, drew))
    }
    /// Record a draw without a query: one that is not an instanced batch.
    pub(crate) fn probe_note(&self, drew: Drew) {
        if let Some(probe) = &self.probe {
            probe.note(drew);
        }
    }
    pub(crate) fn probe_variant(&self, variant: usize) {
        if let Some(probe) = &self.probe {
            probe.variant(variant);
        }
    }
}

impl Drew {
    pub fn quads(draw: &crate::quads::Draw) -> Self {
        let kind = match draw.kind {
            crate::quads::Kind::Particle(..) => "particles",
            crate::quads::Kind::Sprite(_) => "sprites",
            crate::quads::Kind::Model(..) => "blended model",
            #[cfg(not(target_arch = "wasm32"))]
            crate::quads::Kind::Child(_) => "placed child",
        };
        Drew::Quads {
            kind,
            count: draw.range.end - draw.range.start,
        }
    }
}

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
}

/// What a look frame's readback needs to know of the frame it encoded.
#[derive(Clone, Debug, Default)]
pub(crate) struct Facts {
    pub groups: Vec<GroupFact>,
    /// Every group drew directly, unculled: no lists or indirect counts exist.
    pub direct: bool,
    /// Sun shadow cascades drawn (0 without a shadowed sun).
    pub cascades: u32,
    /// The camera's lists were occlusion-culled this frame.
    pub occluding: bool,
    pub slot_list: Vec<u32>,
    /// Each model record's transform slot, to find an entity's model instances.
    pub record_slots: Vec<u32>,
    /// Bytes of the compacted lists and of the indirect arguments the readback copies.
    pub lists: u64,
    pub indirect: u64,
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
    Indirect,
    Lists,
}

/// A look frame's readback, read.
#[derive(Default)]
pub(crate) struct Reading {
    /// Tightly packed RGBA rows, top down.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Per query: whether any sample passed.
    pub covered: Vec<bool>,
    /// The indirect arguments as words.
    pub indirect: Vec<u32>,
    /// The compacted lists as words.
    pub lists: Vec<u32>,
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// The frame just encoded, as the look frame's answers read it.
    pub(crate) fn probe_facts(&self) -> Facts {
        let views = crate::cull::VIEWS;
        let groups: Vec<_> = (self.cull.groups.iter().enumerate())
            .map(|(index, g)| GroupFact {
                batch: g.batch,
                range: g.range.clone(),
                views: g.flags & 0xf,
                keep_all: g.flags & crate::cull::KEEP_ALL != 0,
                mirrored: g.mirrored,
                offsets: std::array::from_fn(|v| self.cull.offsets(v as u32, index)),
            })
            .collect();
        let lists = (groups.iter())
            .flat_map(|g| {
                g.offsets
                    .iter()
                    .map(move |o| u64::from(o.0) + u64::from(g.range.len() as u32) * 4)
            })
            .max()
            .unwrap_or(0)
            .min(self.cull.compacted.raw.size());
        Facts {
            direct: self.cull.direct || groups.is_empty(),
            cascades: self.shadows.as_ref().map_or(0, |s| s.count),
            occluding: self.occluding,
            slot_list: self.slot_list.clone(),
            record_slots: if ASSETS {
                self.models.records.iter().map(|r| r.transform).collect()
            } else {
                Vec::new()
            },
            lists,
            indirect: (u64::from(views) * groups.len() as u64 * 20)
                .min(self.cull.indirect.raw.size()),
            groups,
        }
    }

    /// Copy what the look frame reads back: `target`'s pixels, the probe's
    /// query results, and, for a culled frame, the indirect arguments and the
    /// compacted lists. Takes the probe; the next frame carries none.
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
        if let Some(probe) = probe.filter(|p| p.used() > 0) {
            let n = probe.used();
            let resolve = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("game look coverage resolve"),
                size: u64::from(n) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            encoder.resolve_query_set(&probe.queries, 0..n, &resolve, 0);
            let read = buffer(u64::from(n) * 8, "game look coverage");
            encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, u64::from(n) * 8);
            parts.push((Part::Queries(n), read));
        }
        if !facts.direct && facts.indirect > 0 {
            let read = buffer(facts.indirect, "game look indirect");
            encoder.copy_buffer_to_buffer(&self.cull.indirect.raw, 0, &read, 0, facts.indirect);
            parts.push((Part::Indirect, read));
            if facts.lists > 0 {
                let read = buffer(facts.lists, "game look lists");
                encoder.copy_buffer_to_buffer(&self.cull.compacted.raw, 0, &read, 0, facts.lists);
                parts.push((Part::Lists, read));
            }
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
                Part::Indirect => out.indirect = words(),
                Part::Lists => out.lists = words(),
            }
            drop(data);
            buffer.unmap();
        }
        Ok(out)
    }
}
