//! The aurora composes its children (LLP 1014 D3, the readback fixture's
//! native half): a known bitmap — an opaque white block on a transparent
//! ground — handed to the surface as the host hands it a capture, rendered
//! into a module-owned texture on this machine's GPU and read back. Where
//! the bitmap is transparent the sky is untouched, pixel for pixel; where it
//! is opaque the ink is white; the refraction moves the block's edge by at
//! most the shader's warp. Skips, saying so, when no adapter exists.
//! `EXACT_GPU_OUT=<dir>` keeps the picture as a PPM.

use caltrain_gpu::AuroraSurface;
use exact_gpu::wgpu;
use exact_gpu::{fixture, Frame, Gpu, Surface, Value};

const W: u32 = 160;
const H: u32 = 120;

/// A children texture as `Module::texture` makes one — `Rgba8Unorm`,
/// premultiplied, rows top-down — transparent but for an opaque white block.
fn children(gpu: &Gpu, block: Option<(u32, u32, u32, u32)>) -> wgpu::TextureView {
    let mut bytes = vec![0u8; (W * H * 4) as usize];
    if let Some((x0, y0, w, h)) = block {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let i = ((y * W + x) * 4) as usize;
                bytes[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    let size = wgpu::Extent3d {
        width: W,
        height: H,
        depth_or_array_layers: 1,
    };
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("children"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(W * 4),
            rows_per_image: None,
        },
        size,
    );
    texture.create_view(&Default::default())
}

#[test]
fn the_aurora_composes_its_children_over_the_sky() {
    let gpu = match fixture::device() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("{e}; the readback fixture is skipped");
            return;
        }
    };
    // The shaders travel as files (LLP 1030 D8): registered as a host would.
    exact_gpu::shaders::load_dir(&caltrain_gpu::shader_dir(), &caltrain_gpu::REGISTRY).unwrap();
    let frame = Frame {
        width: W as f32,
        height: H as f32,
        scale: 1.0,
        now_ms: 1234.0,
        children_generation: 0,
        seekable: false,
        period_ms: 0.0,
        shader_generation: exact_gpu::shaders::shader_generation(),
    };
    let mut sky = AuroraSurface::new();
    sky.bind(&[Value::str("mv")], None).unwrap();
    assert_eq!(
        sky.children_mode(),
        exact_gpu::ChildrenMode::Composite { previous: false }
    );

    // The sky alone, then with a transparent children texture: the same
    // picture, pixel for pixel.
    let (alone, _) = fixture::render(&gpu, &mut sky, &frame).unwrap();
    sky.children(Some(&children(&gpu, None)));
    let (clear, _) = fixture::render(&gpu, &mut sky, &frame).unwrap();
    assert!(
        alone.data == clear.data,
        "a transparent children texture changes nothing"
    );

    // A white block in the middle: white ink where it is (the glow only
    // brightens), the sky untouched away from it.
    let block = (60, 40, 40, 40);
    sky.children(Some(&children(&gpu, Some(block))));
    let (composed, wants) = fixture::render(&gpu, &mut sky, &frame).unwrap();
    assert!(wants, "still lit from the clock");
    composed.save("aurora-children");
    let inside = composed.at(80, 60);
    assert!(
        inside[..3].iter().all(|c| *c >= 250),
        "white ink over the sky: {inside:?}"
    );
    // The refraction samples at most 0.012 × 1.25 of the width away — two
    // pixels here; ten pixels from the block the sky is exactly the sky.
    for (x, y) in [
        (10, 10),
        (150, 110),
        (10, 110),
        (150, 10),
        (80, 10),
        (80, 110),
    ] {
        assert_eq!(
            composed.at(x, y),
            alone.at(x, y),
            "the sky is untouched at ({x}, {y})"
        );
    }
    // The white area is the block's, give or take the refracted edge.
    let white = composed.count(|p| p[..3].iter().all(|c| *c >= 250));
    let area = (block.2 * block.3) as usize;
    assert!(
        white >= area * 9 / 10 && white <= area * 11 / 10 + 64,
        "{white} white pixels for a {area}-pixel block"
    );

    // The children gone: the sky alone again.
    sky.children(None);
    let (again, _) = fixture::render(&gpu, &mut sky, &frame).unwrap();
    assert!(again.data == alone.data, "no children, no ink");
}
