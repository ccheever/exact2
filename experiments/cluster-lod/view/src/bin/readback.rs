use clod_view::{Frame, Renderer, wgpu};
pub fn read(renderer: &Renderer, frame: &Frame) -> Result<(Vec<u8>, Option<[f64; 4]>), String> {
    frame
        .pixels
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| {
            if let Err(e) = result {
                eprintln!("pixel map failed: {e}");
            }
        });
    if let Some(buffer) = &frame.timestamps {
        buffer.slice(..).map_async(wgpu::MapMode::Read, |result| {
            if let Err(e) = result {
                eprintln!("timestamp map failed: {e}");
            }
        });
    }
    renderer
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    let mapped = frame
        .pixels
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let mut pixels = Vec::with_capacity((renderer.width * renderer.height * 4) as usize);
    for row in mapped.chunks_exact(frame.row_bytes as usize) {
        pixels.extend_from_slice(&row[..renderer.width as usize * 4]);
    }
    drop(mapped);
    frame.pixels.unmap();
    let times = frame
        .timestamps
        .as_ref()
        .map(|buffer| {
            let mapped = buffer
                .slice(..)
                .get_mapped_range()
                .map_err(|e| e.to_string())?;
            let values: &[u64] = bytemuck::cast_slice(&mapped);

            let scale = renderer.queue.get_timestamp_period() as f64 / 1e6;
            let offset = if frame.gpu_selected { 4 } else { 0 };
            let elapsed = |i: usize| -> f64 {
                match values[i + 1].checked_sub(values[i]) {
                    Some(v) => v as f64 * scale,
                    None => {
                        eprintln!("invalid timestamp pair {i}: {values:?}");
                        f64::NAN
                    }
                }
            };
            let times = [
                elapsed(offset),
                elapsed(offset + 2),
                if frame.gpu_selected { elapsed(0) } else { 0.0 },
                if frame.gpu_selected { elapsed(2) } else { 0.0 },
            ];
            drop(mapped);
            buffer.unmap();
            Ok::<_, String>(times)
        })
        .transpose()?;
    Ok((pixels, times))
}
pub fn png_bytes(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(pixels).map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Difference {
    pub mean: f64,
    pub max: u8,
    pub fraction: f64,
}
pub fn difference(a: &[u8], b: &[u8]) -> Difference {
    let mut sum = 0u64;
    let mut max = 0u8;
    let mut changed = 0usize;
    for (a, b) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut pixel = 0;
        for channel in 0..3 {
            let d = a[channel].abs_diff(b[channel]);
            sum += d as u64;
            max = max.max(d);
            pixel = pixel.max(d);
        }
        changed += usize::from(pixel > 2);
    }
    Difference {
        mean: sum as f64 / (a.len() / 4 * 3) as f64 / 255.0,
        max,
        fraction: changed as f64 / (a.len() / 4) as f64,
    }
}
