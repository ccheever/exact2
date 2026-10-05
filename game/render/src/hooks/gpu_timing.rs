//! Nonblocking timestamp readback, only while a canvas's perf is armed.
use crate::{perf::Ring, GPU_PASS_COUNT, GPU_PASS_NAMES};
use exact_gpu::wgpu;
use std::sync::{Arc, Mutex};
const PAIRS: usize = GPU_PASS_COUNT as usize;
const BYTES: u64 = GPU_PASS_COUNT as u64 * 16;
type Completion = Arc<Mutex<Option<Result<(), String>>>>;
pub(crate) struct GpuTiming {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    done: Completion,
    phase: u8,
    // The pairs the frame awaiting readback wrote; every pass is reported.
    mask: u64,
    rings: [Ring; PAIRS],
    // Bloom from its first level's start to its last level's end (pairs 4-14):
    // passes overlap on a tiler, so their own intervals do not sum.
    bloom: Ring,
    period: f64,
    error: Option<String>,
}
impl GpuTiming {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let features = device.features();
        if !features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("render hook timings"),
            ty: wgpu::QueryType::Timestamp,
            count: GPU_PASS_COUNT * 2,
        });
        let buffer = |usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render hook timing readback"),
                size: BYTES,
                usage,
                mapped_at_creation: false,
            })
        };
        let mut rings: [Ring; PAIRS] = std::array::from_fn(|_| Ring::default());
        for r in &mut rings {
            r.arm(false);
        }
        let mut bloom = Ring::default();
        bloom.arm(false);
        Some(Self {
            set,
            resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
            read: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
            done: Default::default(),
            phase: 0,
            mask: 0,
            rings,
            bloom,
            period: f64::from(queue.get_timestamp_period()) / 1_000_000.,
            error: None,
        })
    }
    pub fn reset(&mut self) {
        for r in &mut self.rings {
            r.arm(true);
        }
        self.bloom.arm(true);
        self.mask = 0;
    }
    pub fn poll(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = device.poll(wgpu::PollType::Poll);
        let Some(result) = self.done.lock().unwrap().take() else {
            return;
        };
        if let Err(error) = result {
            self.error = Some(error);
            self.phase = 0;
            return;
        }
        if self.phase == 1 {
            // Metal counter samples are not resource hazards. Resolve only after
            // the frame's completion callback, without blocking the display thread.
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.resolve_query_set(&self.set, 0..GPU_PASS_COUNT * 2, &self.resolve, 0);
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, BYTES);
            queue.submit([encoder.finish()]);
            let done = self.done.clone();
            self.read
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    *done.lock().unwrap() = Some(r.map_err(|e| e.to_string()))
                });
            self.phase = 2;
        } else if self.phase == 2 {
            let data = self
                .read
                .slice(..)
                .get_mapped_range()
                .expect("completed timing map");
            let mut bloom: Option<(u64, u64)> = None;
            for pair in 0..PAIRS {
                if self.mask & 1 << pair == 0 {
                    continue;
                }
                let at = pair * 16;
                let a = u64::from_ne_bytes(data[at..at + 8].try_into().unwrap());
                let b = u64::from_ne_bytes(data[at + 8..at + 16].try_into().unwrap());
                if a > 0 && b >= a {
                    let ms = (b - a) as f64 * self.period;
                    self.rings[pair].push(ms);
                    if (4..=14).contains(&pair) {
                        let span = bloom.get_or_insert((a, b));
                        *span = (span.0.min(a), span.1.max(b));
                    }
                }
            }
            if let Some((a, b)) = bloom {
                self.bloom.push((b - a) as f64 * self.period);
            }
            drop(data);
            self.read.unmap();
            self.phase = 0;
        }
    }
    pub fn query(&self) -> Option<&wgpu::QuerySet> {
        (self.phase == 0 && self.error.is_none()).then_some(&self.set)
    }
    /// `timed`: the timestamp pairs the submitted frame wrote.
    pub fn submitted(&mut self, queue: &wgpu::Queue, timed: u64) {
        if self.phase != 0 || self.error.is_some() {
            return;
        }
        self.mask = timed;
        let done = self.done.clone();
        queue.on_submitted_work_done(move || *done.lock().unwrap() = Some(Ok(())));
        self.phase = 1;
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
        out.push_str("},\"gpuTimingError\":");
        out.push_str(&self.error.as_ref().map_or_else(
            || "null".into(),
            |e| exact_game::json::to_string(e).unwrap(),
        ));
    }
}
