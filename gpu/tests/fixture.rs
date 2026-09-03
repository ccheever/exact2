//! The readback helper (`exact_gpu::fixture`): a surface's frame comes back
//! as tightly packed RGBA whatever the copy's row padding or the texture's
//! channel order, refusals name what was refused, and the PPM is a PPM.
//! The GPU-backed tests skip, saying so, without an adapter.

use exact_gpu::fixture::{self, Pixels};
use exact_gpu::wgpu;
use exact_gpu::{Frame, Gpu, Surface, SurfaceError, Value};

/// Clears to one color.
struct Fill([f64; 4]);

impl Surface for Fill {
    fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
        Ok(())
    }

    fn render(
        &mut self,
        _: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        _: wgpu::TextureFormat,
    ) -> bool {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: self.0[0],
                        g: self.0[1],
                        b: self.0[2],
                        a: self.0[3],
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        queue.submit([encoder.finish()]);
        false
    }
}

fn device() -> Option<Gpu> {
    match fixture::device() {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("{e}; the readback test is skipped");
            None
        }
    }
}

fn texture(gpu: &Gpu, format: wgpu::TextureFormat, layers: u32) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 3,
            height: 2,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[test]
fn a_frame_reads_back_unpadded_and_as_a_ppm() {
    let Some(gpu) = device() else { return };
    let mut fill = Fill([0.2, 0.4, 0.6, 1.0]);
    // Three wide: a 12-byte row the copy pads to 256.
    let frame = Frame {
        width: 3.0,
        height: 2.0,
        scale: 1.0,
        now_ms: 0.0,
        children_generation: 0,
        shader_generation: exact_gpu::shaders::shader_generation(),
    };
    let (px, wants) = fixture::render(&gpu, &mut fill, &frame).unwrap();
    assert!(!wants);
    assert_eq!((px.width, px.height, px.data.len()), (3, 2, 24));
    assert_eq!(px.at(2, 1), [51, 102, 153, 255]);
    assert_eq!(px.count(|p| p == [51, 102, 153, 255]), 6);
    let ppm = px.ppm();
    assert_eq!(&ppm[..11], b"P6\n3 2\n255\n");
    assert_eq!(ppm.len(), 11 + 18);
}

#[test]
fn a_bgra_texture_comes_back_rgba() {
    let Some(gpu) = device() else { return };
    let mut fill = Fill([0.2, 0.4, 0.6, 1.0]);
    let bgra = texture(&gpu, wgpu::TextureFormat::Bgra8Unorm, 1);
    let view = bgra.create_view(&Default::default());
    let frame = Frame {
        width: 3.0,
        height: 2.0,
        scale: 1.0,
        now_ms: 0.0,
        children_generation: 0,
        shader_generation: exact_gpu::shaders::shader_generation(),
    };
    fill.render(
        &frame,
        &gpu.device,
        &gpu.queue,
        &view,
        wgpu::TextureFormat::Bgra8Unorm,
    );
    let px = fixture::read(&gpu, &bgra).unwrap();
    assert_eq!(px.at(0, 0), [51, 102, 153, 255], "swizzled to RGBA");
}

#[test]
fn other_formats_and_array_textures_are_refused_by_name() {
    let Some(gpu) = device() else { return };
    let e = fixture::read(&gpu, &texture(&gpu, wgpu::TextureFormat::R32Float, 1)).unwrap_err();
    assert!(e.contains("R32Float") && e.contains("not an RGBA8"), "{e}");
    let e = fixture::read(&gpu, &texture(&gpu, wgpu::TextureFormat::Rgba8Unorm, 2)).unwrap_err();
    assert!(e.contains("2 layers"), "{e}");
}

#[test]
#[should_panic(expected = "(3, 0) is off a 3x2 picture")]
fn a_pixel_off_the_picture_panics() {
    let px = Pixels {
        width: 3,
        height: 2,
        data: vec![0; 24],
    };
    px.at(3, 0);
}

#[test]
fn save_creates_the_directory_it_is_pointed_at() {
    let dir = std::env::temp_dir().join(format!("exact-gpu-fixture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let px = Pixels {
        width: 1,
        height: 1,
        data: vec![1, 2, 3, 4],
    };
    // Process-wide; the only reader is `save`, and this is the file's one setter.
    std::env::set_var("EXACT_GPU_OUT", &dir);
    let path = px.save("one");
    std::env::remove_var("EXACT_GPU_OUT");
    assert_eq!(path, Some(dir.join("one.ppm")));
    assert_eq!(
        std::fs::read(dir.join("one.ppm")).unwrap(),
        b"P6\n1 1\n255\n\x01\x02\x03"
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(px.save("none"), None, "unset: nothing written");
}
