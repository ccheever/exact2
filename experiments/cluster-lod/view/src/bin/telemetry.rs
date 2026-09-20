//! Native diagnostics: bounded staging, no per-frame waits in interval-only playback.
use clod_view::{Renderer, wgpu};
use std::sync::mpsc::{Receiver, channel};
pub struct Counters {
    buffer: wgpu::Buffer,
    pending: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    pub latest: [u64; 2],
    pub totals: [u64; 2],
    pub samples: u32,
}
impl Counters {
    pub fn new(device: &wgpu::Device, frames: u32) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bounded frame completeness"),
                size: 32 * u64::from(frames.max(1)),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            pending: None,
            latest: [0; 2],
            totals: [0; 2],
            samples: 0,
        }
    }
    pub fn encode(&self, renderer: &Renderer, encoder: &mut wgpu::CommandEncoder, frame: u32) {
        renderer.copy_selection_totals(encoder, &self.buffer, u64::from(frame) * 32);
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn map(&mut self) {
        let (send, receive) = channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = send.send(r);
            });
        self.pending = Some(receive);
    }
    pub fn collect(
        &mut self,
        device: &wgpu::Device,
        wait: bool,
        frames: u32,
    ) -> Result<(), String> {
        device
            .poll(if wait {
                wgpu::PollType::wait_indefinitely()
            } else {
                wgpu::PollType::Poll
            })
            .map_err(|e| e.to_string())?;
        let Some(receive) = &self.pending else {
            return Ok(());
        };
        match receive.try_recv() {
            Ok(result) => result.map_err(|e| e.to_string())?,
            Err(std::sync::mpsc::TryRecvError::Empty) if !wait => return Ok(()),
            Err(e) => return Err(e.to_string()),
        }
        let mapped = self
            .buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        for words in bytemuck::cast_slice::<u8, u32>(&mapped)
            .chunks_exact(8)
            .take(frames as usize)
        {
            self.latest = [u64::from(words[3]), u64::from(words[7])];
            for i in 0..2 {
                self.totals[i] += self.latest[i];
            }
            self.samples += 1;
        }
        drop(mapped);
        self.buffer.unmap();
        self.pending = None;
        Ok(())
    }
}
pub fn retain<T>(samples: &mut Vec<T>, value: T) {
    if samples.len() == 1000 {
        samples.remove(0);
    }
    samples.push(value);
}
