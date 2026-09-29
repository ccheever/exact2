//! Readback for fixtures (LLP 1009 D1/D4): a surface rendered into a
//! module-owned texture that can be copied, and read back as pixels —
//! natively, synchronously, the parity instrument's native half. (The web's
//! readback is asynchronous and the module ABI's; it is not here.) With
//! `EXACT_GPU_OUT=<dir>` set, [`Pixels::save`] writes a PPM there, so a run
//! can be looked at without a host.

use crate::{block_on, load_gpu, Frame, Gpu, Surface};
use std::path::PathBuf;
use std::time::Duration;

/// Skip only a classified absent adapter; device and configuration failures fail tests.
pub fn device_or_skip<T>(result: Result<T, String>) -> Option<T> {
    match result {
        Ok(gpu) => Some(gpu),
        Err(reason) if reason.starts_with("no adapter:") => {
            eprintln!("SKIP GPU test: {reason}");
            None
        }
        Err(reason) => panic!("GPU device request failed: {reason}"),
    }
}

/// Pixels read back: `width`×`height` RGBA, four bytes each, rows top-down,
/// tightly packed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub data: Vec<u8>,
}

impl Pixels {
    /// The pixel at `(x, y)`; panics off the picture.
    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        assert!(
            x < self.width && y < self.height,
            "({x}, {y}) is off a {}x{} picture",
            self.width,
            self.height
        );
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    /// How many pixels satisfy `pred`.
    pub fn count(&self, pred: impl Fn([u8; 4]) -> bool) -> usize {
        self.data
            .chunks_exact(4)
            .filter(|p| pred([p[0], p[1], p[2], p[3]]))
            .count()
    }

    /// The pixels as a binary PPM (P6), alpha dropped.
    pub fn ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        out.extend(self.data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
        out
    }

    /// Write `<name>.ppm` under `$EXACT_GPU_OUT` when it is set (the
    /// directory is created): the path written, or `None`.
    pub fn save(&self, name: &str) -> Option<PathBuf> {
        let dir = PathBuf::from(std::env::var_os("EXACT_GPU_OUT")?);
        let path = dir.join(format!("{name}.ppm"));
        let written =
            std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, self.ppm()));
        match written {
            Ok(()) => Some(path),
            Err(e) => {
                eprintln!("exact gpu: could not write {}: {e}", path.display());
                None
            }
        }
    }
}

/// This machine's device, or why there is none — a fixture skips, saying so.
pub fn device() -> Result<Gpu, String> {
    block_on(load_gpu(wgpu::Instance::default(), None))
}

/// One frame of `surface` into a fresh `Rgba8Unorm` texture of the frame's
/// pixel size, read back; and whether the surface wants another frame. The
/// surface has no children texture here. `Rgba8Unorm` rather than the
/// presented surface's `Bgra8Unorm` because the browser's readback is RGBA
/// and the surface builds its pipeline for whatever format it is given: the
/// fixture measures the shader, not a drawable's channel order.
pub fn render(
    gpu: &Gpu,
    surface: &mut dyn Surface,
    frame: &Frame,
) -> Result<(Pixels, bool), String> {
    let (width, height) = frame.pixels();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fixture"),
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
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let wants = surface.render(frame, &gpu.device, &gpu.queue, &mut encoder, &view, format);
    gpu.queue.submit([encoder.finish()]);
    surface.submitted();
    Ok((read(gpu, &texture)?, wants))
}

/// Read a texture back as RGBA: one 2D layer (mip 0) of an `Rgba8Unorm`,
/// `Rgba8UnormSrgb`, `Bgra8Unorm`, or `Bgra8UnormSrgb` texture created with
/// `COPY_SRC` — BGRA is swizzled — waiting for the GPU. Anything else is
/// refused by name.
pub fn read(gpu: &Gpu, texture: &wgpu::Texture) -> Result<Pixels, String> {
    use wgpu::TextureFormat as F;
    let bgra = match texture.format() {
        F::Rgba8Unorm | F::Rgba8UnormSrgb => false,
        F::Bgra8Unorm | F::Bgra8UnormSrgb => true,
        other => return Err(format!("{other:?} is not an RGBA8 or BGRA8 format")),
    };
    let layers = texture.depth_or_array_layers();
    if texture.dimension() != wgpu::TextureDimension::D2 || layers != 1 {
        return Err(format!(
            "a {:?} texture with {layers} layers is not one 2D layer",
            texture.dimension()
        ));
    }
    let (width, height) = (texture.width(), texture.height());
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let row = (width * 4).div_ceil(align) * align;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    // The map's own result, not inferred from the poll: a lost device or a
    // refused map names itself.
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| format!("poll: {e:?}"))?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| "the map callback did not run".to_string())?
        .map_err(|e| format!("map: {e:?}"))?;
    let mapped = slice
        .get_mapped_range()
        .map_err(|e| format!("map: {e:?}"))?;
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let start = (y * row) as usize;
        let line = &mapped[start..start + (width * 4) as usize];
        if bgra {
            data.extend(line.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]));
        } else {
            data.extend_from_slice(line);
        }
    }
    Ok(Pixels {
        width,
        height,
        data,
    })
}
