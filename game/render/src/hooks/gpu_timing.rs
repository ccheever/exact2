//! Nonblocking timestamp readback, only while a canvas's perf is armed.
//!
//! Each sampled frame is read back and published on its own once the GPU has
//! run it; the gap between frames needs two consecutive rendered frames, and
//! comes from a frame and the one before it when both were sampled.
use crate::{perf::Ring, GPU_PASS_COUNT, GPU_PASS_NAMES};
use exact_gpu::wgpu;
use std::sync::{Arc, Mutex};
const PAIRS: usize = GPU_PASS_COUNT as usize;
const BYTES: u64 = GPU_PASS_COUNT as u64 * 16;
const STRIDE: u64 =
    BYTES.div_ceil(wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT) * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
/// Query sets: a frame is timed while the one before it is read back, so
/// consecutive frames can both be sampled.
const SLOTS: usize = 2;
type Completion = Arc<Mutex<Option<Result<(), String>>>>;

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Free,
    /// Submitted; waiting for the GPU to run the frame.
    Rendered,
    /// Resolved; waiting for its readback to map.
    Mapping,
}

struct Slot {
    set: wgpu::QuerySet,
    read: wgpu::Buffer,
    done: Completion,
    stage: Stage,
    // The pairs the frame wrote; every pass is reported.
    mask: u64,
    /// The frame's number among the rendered frames ([`GpuTiming::frames`]).
    frame: u64,
    /// The [`GpuTiming::reset`] it was sampled after.
    generation: u64,
}

pub(crate) struct GpuTiming {
    slots: [Slot; SLOTS],
    resolve: wgpu::Buffer,
    /// The free slot the next frame writes.
    next: Option<usize>,
    /// Frames submitted while armed, sampled or not.
    frames: u64,
    generation: u64,
    /// The last published frame's number and its last pass's end.
    previous: Option<(u64, u64)>,
    idle: Ring,
    envelope: Ring,
    rings: [Ring; PAIRS],
    // Bloom from its first level's start to its last level's end (pairs 4-14):
    // passes overlap on a tiler, so their own intervals do not sum.
    bloom: Ring,
    period: f64,
    error: Option<String>,
}

fn ring() -> Ring {
    let mut r = Ring::default();
    r.arm(false);
    r
}

impl GpuTiming {
    /// GPU buffers an armed canvas allocates for its timings.
    pub const BUFFERS: u64 = SLOTS as u64 + 1;

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let features = device.features();
        if !features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let buffer = |size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render hook timing readback"),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let slots = std::array::from_fn(|_| Slot {
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("render hook timings"),
                ty: wgpu::QueryType::Timestamp,
                count: GPU_PASS_COUNT * 2,
            }),
            read: buffer(
                STRIDE,
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            done: Default::default(),
            stage: Stage::Free,
            mask: 0,
            frame: 0,
            generation: 0,
        });
        Some(Self {
            slots,
            resolve: buffer(
                STRIDE * SLOTS as u64,
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            next: Some(0),
            frames: 0,
            generation: 0,
            previous: None,
            idle: ring(),
            envelope: ring(),
            rings: std::array::from_fn(|_| ring()),
            bloom: ring(),
            period: f64::from(queue.get_timestamp_period()) / 1_000_000.,
            error: None,
        })
    }
    /// Forget every sample; frames still in flight are read and discarded.
    pub fn reset(&mut self) {
        for r in &mut self.rings {
            r.arm(true);
        }
        self.bloom.arm(true);
        self.idle.arm(true);
        self.envelope.arm(true);
        self.generation += 1;
        self.previous = None;
    }
    pub fn poll(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = device.poll(wgpu::PollType::Poll);
        // Oldest first: a frame's gap is measured from the frame before it.
        let mut order: [usize; SLOTS] = std::array::from_fn(|i| i);
        order.sort_by_key(|&i| self.slots[i].frame);
        for i in order {
            let Some(result) = self.slots[i].done.lock().unwrap().take() else {
                continue;
            };
            let slot = &mut self.slots[i];
            if let Err(error) = result {
                self.error = Some(error);
                slot.stage = Stage::Free;
                continue;
            }
            match slot.stage {
                Stage::Rendered => {
                    // Metal counter samples are not resource hazards. Resolve only
                    // after the frame's completion callback, without blocking the
                    // display thread.
                    let at = i as u64 * STRIDE;
                    let mut encoder = device.create_command_encoder(&Default::default());
                    encoder.resolve_query_set(&slot.set, 0..GPU_PASS_COUNT * 2, &self.resolve, at);
                    encoder.copy_buffer_to_buffer(&self.resolve, at, &slot.read, 0, STRIDE);
                    queue.submit([encoder.finish()]);
                    let done = slot.done.clone();
                    slot.read
                        .slice(..)
                        .map_async(wgpu::MapMode::Read, move |r| {
                            *done.lock().unwrap() = Some(r.map_err(|e| e.to_string()))
                        });
                    slot.stage = Stage::Mapping;
                }
                Stage::Mapping => {
                    self.publish(i);
                    let slot = &mut self.slots[i];
                    slot.read.unmap();
                    slot.stage = Stage::Free;
                }
                Stage::Free => {}
            }
        }
        if self.next.is_none_or(|i| self.slots[i].stage != Stage::Free) {
            self.next = (0..SLOTS).find(|&i| self.slots[i].stage == Stage::Free);
        }
    }
    /// Push slot `i`'s mapped samples into the rings.
    fn publish(&mut self, i: usize) {
        let slot = &self.slots[i];
        if slot.generation != self.generation {
            return;
        }
        let data = slot
            .read
            .slice(..)
            .get_mapped_range()
            .expect("completed timing map");
        let mut bloom: Option<(u64, u64)> = None;
        let mut span: Option<(u64, u64)> = None;
        for pair in 0..PAIRS {
            if slot.mask & 1 << pair == 0 {
                continue;
            }
            let at = pair * 16;
            let a = u64::from_ne_bytes(data[at..at + 8].try_into().unwrap());
            let b = u64::from_ne_bytes(data[at + 8..at + 16].try_into().unwrap());
            if a > 0 && b >= a {
                self.rings[pair].push((b - a) as f64 * self.period);
                if (4..=14).contains(&pair) {
                    let s = bloom.get_or_insert((a, b));
                    *s = (s.0.min(a), s.1.max(b));
                }
                let s = span.get_or_insert((a, b));
                *s = (s.0.min(a), s.1.max(b));
            }
        }
        if let Some((a, b)) = bloom {
            self.bloom.push((b - a) as f64 * self.period);
        }
        if let Some((first, last)) = span {
            self.envelope.push((last - first) as f64 * self.period);
            // Their pass boundaries share the GPU clock; overlapping frames have zero idle.
            if let Some((frame, end)) = self.previous {
                if frame + 1 == slot.frame {
                    self.idle
                        .push(first.saturating_sub(end) as f64 * self.period);
                }
            }
            self.previous = Some((slot.frame, last));
        }
    }
    pub fn query(&self) -> Option<&wgpu::QuerySet> {
        if self.error.is_some() {
            return None;
        }
        self.next.map(|i| &self.slots[i].set)
    }
    /// A rendered frame was submitted; `timed`: the timestamp pairs it wrote
    /// into [`Self::query`]'s set, none when it was not sampled.
    pub fn submitted(&mut self, queue: &wgpu::Queue, timed: u64) {
        self.frames += 1;
        if timed == 0 || self.error.is_some() {
            return;
        }
        let Some(i) = self.next.take() else {
            return;
        };
        let slot = &mut self.slots[i];
        slot.mask = timed;
        slot.frame = self.frames;
        slot.generation = self.generation;
        slot.stage = Stage::Rendered;
        let done = slot.done.clone();
        queue.on_submitted_work_done(move || *done.lock().unwrap() = Some(Ok(())));
    }
    /// Every pass, by name; a pass that never ran (or that this device cannot
    /// time inside a pass or encoder) reports an empty ring. `bloom` spans its
    /// levels. Intervals overlap on tiled GPUs: do not sum them.
    pub fn append(&self, out: &mut String) {
        out.push_str(",\"gpuMs\":{");
        for (pair, ring) in self.rings.iter().enumerate() {
            out.push_str(&format!("\"{}\":", GPU_PASS_NAMES[pair]));
            ring.json(out);
            out.push(',');
        }
        out.push_str("\"bloom\":");
        self.bloom.json(out);
        out.push_str(",\"frame envelope\":");
        self.envelope.json(out);
        out.push_str(",\"between frames\":");
        self.idle.json(out);
        out.push_str("},\"gpuTimingError\":");
        out.push_str(&self.error.as_ref().map_or_else(
            || "null".into(),
            |e| exact_game::json::to_string(e).unwrap(),
        ));
    }
}
