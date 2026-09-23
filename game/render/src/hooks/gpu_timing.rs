//! Nonblocking timestamp readback, only while a hooked canvas's perf is armed.
use super::Needs;
use crate::{perf::Ring, GPU_PASS_COUNT, GPU_PASS_NAMES};
use exact_gpu::wgpu;
use std::sync::{Arc, Mutex};
const PAIRS: [u32; 8] = [3, 17, 18, 19, 20, 21, 22, 23];
const BYTES: u64 = GPU_PASS_COUNT as u64 * 16;
type Completion = Arc<Mutex<Option<Result<(), String>>>>;
pub(crate) struct GpuTiming {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    done: Completion,
    phase: u8,
    mask: [bool; 8],
    rings: [Ring; 8],
    period: f64,
    features: wgpu::Features,
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
        let mut rings: [Ring; 8] = Default::default();
        for r in &mut rings {
            r.arm(false);
        }
        Some(Self {
            set,
            resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
            read: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
            done: Default::default(),
            phase: 0,
            mask: [false; 8],
            rings,
            period: f64::from(queue.get_timestamp_period()) / 1_000_000.,
            features,
            error: None,
        })
    }
    pub fn reset(&mut self) {
        for r in &mut self.rings {
            r.arm(true);
        }
        self.mask = [false; 8];
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
            for (i, pair) in PAIRS.iter().enumerate() {
                if !self.mask[i] {
                    continue;
                }
                let at = *pair as usize * 16;
                let a = u64::from_ne_bytes(data[at..at + 8].try_into().unwrap());
                let b = u64::from_ne_bytes(data[at + 8..at + 16].try_into().unwrap());
                if a > 0 && b >= a {
                    self.rings[i].push((b - a) as f64 * self.period);
                }
            }
            drop(data);
            self.read.unmap();
            self.phase = 0;
        }
    }
    pub fn query(&self) -> Option<&wgpu::QuerySet> {
        (self.phase == 0 && self.error.is_none()).then_some(&self.set)
    }
    pub fn submitted(&mut self, queue: &wgpu::Queue, needs: Needs, drawable: bool) {
        if self.phase != 0 || self.error.is_some() {
            return;
        }
        let enc = self
            .features
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
        let pass = self
            .features
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES);
        self.mask = [
            true,
            drawable && enc,
            drawable && pass,
            drawable && pass,
            drawable && needs.contains(Needs::SCENE_COPY),
            drawable && enc && needs.contains(Needs::HDR_POST),
            drawable && needs.contains(Needs::SCENE_COPY),
            drawable && needs.contains(Needs::FINAL_DEPTH),
        ];
        let done = self.done.clone();
        queue.on_submitted_work_done(move || *done.lock().unwrap() = Some(Ok(())));
        self.phase = 1;
    }
    pub fn append(&self, out: &mut String) {
        out.push_str(",\"gpuMs\":{");
        for (i, pair) in PAIRS.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("\"{}\":", GPU_PASS_NAMES[*pair as usize]));
            let supported = match i {
                1 | 5 => self
                    .features
                    .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
                2 | 3 => self
                    .features
                    .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES),
                _ => true,
            };
            if supported {
                self.rings[i].json(out);
            } else {
                out.push_str("null");
            }
        }
        out.push_str("},\"gpuTimingError\":");
        out.push_str(&self.error.as_ref().map_or_else(
            || "null".into(),
            |e| exact_game::json::to_string(e).unwrap(),
        ));
    }
}
