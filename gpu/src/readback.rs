//! Reusable native capture storage. A canvas keeps one target and staging
//! buffer; map completion is still synchronous, so no frame can overwrite it.
use crate::*;

pub(crate) struct Readback {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    buffer: wgpu::Buffer,
    row: u32,
}

impl Readback {
    fn new(gpu: &Gpu, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas readback"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("canvas readback staging"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            texture,
            view,
            buffer,
            row,
        }
    }

    fn copy(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row),
                    rows_per_image: None,
                },
            },
            self.texture.size(),
        );
    }

    fn read(&self, gpu: &Gpu, out: &mut [u8]) -> Result<(), String> {
        let slice = self.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let result = (|| {
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| format!("poll: {e:?}"))?;
            rx.recv_timeout(std::time::Duration::from_secs(10))
                .map_err(|_| "the map callback did not run".to_string())?
                .map_err(|e| format!("map: {e:?}"))?;
            let mapped = slice
                .get_mapped_range()
                .map_err(|e| format!("map: {e:?}"))?;
            let width = self.texture.width() as usize * 4;
            let bgra = matches!(
                self.texture.format(),
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            );
            for (dst, src) in out
                .chunks_exact_mut(width)
                .zip(mapped.chunks_exact(self.row as usize))
            {
                if bgra {
                    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                        d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
                    }
                } else {
                    dst.copy_from_slice(&src[..width]);
                }
            }
            Ok(())
        })();
        self.buffer.unmap();
        result
    }
}

impl Module {
    /// Render and capture directly into caller-owned RGBA bytes, reusing GPU
    /// storage until the size, format, or device changes.
    pub fn readback_into(&mut self, id: u32, frame: &Frame, out: &mut [u8]) -> Option<bool> {
        self.check_device();
        if let Some(why) = shaders::missing(self.registry.shaders) {
            self.error = why;
            return None;
        }
        self.flush();
        let gpu = self.gpu.as_ref()?;
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return None;
        };
        if !inst.bound {
            return None;
        }
        let format = inst
            .config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);
        if !matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            self.error = format!("{format:?} is not an RGBA8 or BGRA8 format");
            return None;
        }
        let frame = Frame {
            seekable: self.seekable,
            period_ms: self.period_ms,
            children_generation: inst.children_generation,
            shader_generation: shaders::shader_generation(),
            headroom: 1.,
            ..*frame
        };
        let (width, height) = frame.pixels();
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        if out.len() < len {
            self.error = format!("readback: {} bytes for {} pixels", out.len(), len / 4);
            return None;
        }
        inst.surface.prepare_assets(&gpu.device, &gpu.queue, format);
        if inst.readback.as_ref().is_none_or(|r| {
            r.texture.width() != width
                || r.texture.height() != height
                || r.texture.format() != format
        }) {
            inst.readback = Some(Readback::new(gpu, width, height, format));
        }
        let target = inst.readback.as_ref().unwrap();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let wants = inst.surface.render(
            &frame,
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &target.view,
            format,
        );
        target.copy(&mut encoder);
        gpu.queue.submit([encoder.finish()]);
        inst.surface.submitted();
        let result = target.read(gpu, &mut out[..len]);
        inst.drain();
        if let Some(SurfaceError(e)) = inst.surface.take_error() {
            self.error = e;
            return None;
        }
        match result {
            Ok(()) => {
                inst.dirty = false;
                Some(wants)
            }
            Err(e) => {
                self.error = e;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Solid;
    impl Surface for Solid {
        fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn render(
            &mut self,
            frame: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            encoder: &mut wgpu::CommandEncoder,
            view: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: frame.now_ms / 255.,
                            g: 0.,
                            b: 1.,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            true
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("solid", 0, || Box::new(Solid))],
        shaders: &[],
    };

    #[test]
    fn reusable_capture_handles_unaligned_rows_resize_and_short_buffers() {
        let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
            return;
        };
        let mut module = Module::new(&REGISTRY);
        module.set_gpu(gpu);
        let id = module.create_headless("solid").unwrap();
        module.bind(id, &[], None);
        let mut frame = Frame {
            width: 3.,
            height: 2.,
            scale: 1.,
            now_ms: 31.,
            seekable: false,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
            headroom: 1.,
        };
        for (width, red) in [(3., 31.), (3., 95.), (65., 191.), (1., 255.)] {
            frame.width = width;
            frame.now_ms = red;
            let len = width as usize * 2 * 4;
            let mut output = vec![77; len + 4];
            assert_eq!(module.readback_into(id, &frame, &mut output), Some(true));
            assert!(output[..len]
                .chunks_exact(4)
                .all(|pixel| pixel == [red as u8, 0, 255, 255]));
            assert_eq!(&output[len..], &[77; 4], "caller suffix remains untouched");
            let mut short = vec![77; len - 1];
            assert!(module.readback_into(id, &frame, &mut short).is_none());
            assert!(short.iter().all(|&byte| byte == 77), "refusal is atomic");
        }
        module.lose_device();
        assert!(module.instances[&id].readback.is_none());
    }
}
