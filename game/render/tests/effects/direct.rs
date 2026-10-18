//! Direct frames with models, hulls and translucency (render README, "Direct
//! frames"): the frame drawn straight into an sRGB target against the same frame
//! through the HDR image and its tone pass.
use super::super::support::{gpu, target};
use exact_game::asset::{AlphaMode, MaterialData, MeshData, MeshOutline, Model, Node, TextureData};
use exact_game::{
    Bloom, Camera, DirectionalLight, Emitter, Environment, Fog, Material, SceneSamples,
    ShadowFilter, Shadows, Sprite, ToneMapping, Transform, Vec3, World,
};
use exact_game_render::{shapes, Feed, Renderer};
use exact_gpu::{fixture, wgpu, Gpu};

pub(super) const SIZE: (u32, u32) = (192, 144);
const CLEAR: [f32; 3] = [0.45, 0.6, 0.8];

pub(super) fn sphere(material: MaterialData, outline: bool) -> Model {
    let (v, indices) = shapes::sphere(32);
    let normals: Vec<f32> = v.iter().flat_map(|v| v.normal).collect();
    Model {
        meshes: vec![MeshData {
            positions: v.iter().flat_map(|v| v.position).collect(),
            normals: normals.clone(),
            uvs: v.iter().flat_map(|v| v.uv).collect(),
            indices,
            outline: outline.then(|| MeshOutline {
                width: 0.04,
                min_pixels: 1.5,
                color: [0.9, 0.05, 0.05],
                normals,
                reach: vec![0.1; v.len()],
            }),
            bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
            ..Default::default()
        }],
        materials: vec![material],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
        ..Default::default()
    }
}

fn panel(material: MaterialData) -> Model {
    Model {
        meshes: vec![MeshData {
            positions: vec![-0.5, -0.5, 0., 0.5, -0.5, 0., 0.5, 0.5, 0., -0.5, 0.5, 0.],
            normals: [0., 0., 1.].repeat(4),
            uvs: vec![0., 1., 1., 1., 1., 0., 0., 0.],
            indices: vec![0, 1, 2, 0, 2, 3],
            bounds: [-0.5, -0.5, 0., 0.5, 0.5, 0.],
            ..Default::default()
        }],
        materials: vec![material],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        bounds: [-0.5, -0.5, 0., 0.5, 0.5, 0.],
        ..Default::default()
    }
}

/// Castle Town's settings: one shadowed sun (PCF-soft, one cascade), linear fog
/// in the clear colour, no tone mapping or bloom. Toon spheres with hulls and
/// rim, one far in the fog and one emitting above 1 with nothing translucent over
/// it; a blended model, blended and masked sprites, straight-alpha and additive
/// particles, all in front of lit, fogged ground. `translucent: false` leaves
/// out everything that blends.
pub(super) fn town(
    exposure: f32,
    tone_mapping: ToneMapping,
    translucent: bool,
    bloom: bool,
) -> World {
    let mut w = World::new(60, 1);
    w.insert_resource(Environment {
        background: Some(CLEAR),
        zenith: [0.55, 0.6, 0.66],
        horizon: [0.4, 0.42, 0.45],
        ground: [0.12; 3],
        ambient: 0.62,
        sun_disc: 0.,
        exposure,
        fog: Some(Fog {
            color: Some(CLEAR),
            ..Fog::linear(5., 16.)
        }),
        bloom: bloom.then(Bloom::default),
        tone_mapping,
    });
    w.insert_resource(Shadows {
        cascades: 1,
        filter: ShadowFilter::PcfSoft {
            depth_bias: 0.0004,
            normal_bias: 0.044,
        },
        strength: 0.65,
        ..Shadows::default()
    });
    w.spawn((
        Transform::at(6., 18., -6.).looking_at(Vec3::ZERO, Vec3::Y),
        DirectionalLight {
            color: [1.; 3],
            illuminance: 1.15 / 0.0003,
            shadows: true,
        },
    ));
    w.spawn((
        Transform::at(0., 2.4, 7.).looking_at(Vec3::new(0., 0.8, 0.), Vec3::Y),
        Camera::default(),
    ));
    w.spawn((
        Transform {
            position: Vec3::new(0., -0.05, -6.),
            scale: Vec3::new(14., 0.1, 30.),
            ..Default::default()
        },
        exact_game::Mesh::cube(1.),
        Material::rgb(0.42, 0.4, 0.36),
    ));
    let toon = MaterialData {
        base_color: [0.8, 0.55, 0.3, 1.],
        roughness: 0.8,
        rim: 0.35,
        ..Default::default()
    };
    let toon = w.generated_model("toon.model", sphere(toon, true)).unwrap();
    w.spawn((at(-1.3, 0.8, 0., 1.6), toon.clone()));
    w.spawn((at(2.5, 0.8, -9., 1.6), toon));
    let glow = MaterialData {
        base_color: [0.1, 0.1, 0.1, 1.],
        emissive: [0.3, 0.6, 2.5],
        ..Default::default()
    };
    let glow = w
        .generated_model("glow.model", sphere(glow, false))
        .unwrap();
    w.spawn((at(2.6, 0.5, 1., 0.8), glow));
    if translucent {
        let glass = MaterialData {
            base_color: [0.1, 0.5, 0.9, 0.45],
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..Default::default()
        };
        let glass = w.generated_model("glass.model", panel(glass)).unwrap();
        w.spawn((at(-0.5, 0.9, 1.4, 1.3), glass));
        w.spawn((
            Transform::at(0.4, 2., 0.6),
            Sprite {
                color: [1., 1., 1., 0.85],
                ..Sprite::new("tag.tex", [1.4, 0.6])
            },
        ));
        w.spawn((
            Transform::at(-2.6, 1.9, 0.4),
            Sprite {
                alpha: AlphaMode::Mask,
                ..Sprite::new("tag.tex", [1., 0.5])
            },
        ));
        w.spawn((
            Transform::at(-2.4, 0.3, 1.6),
            Emitter {
                additive: false,
                color: [[1., 0.6, 0.2, 0.8], [0.9, 0.2, 0.1, 0.1]],
                size: [0.18, 0.06],
                seed: 3,
                ..Emitter::sparks().rate(120.).lifetime(0.8)
            },
        ));
        w.spawn((
            Transform::at(0.9, 0.2, 2.2),
            Emitter {
                color: [[0.25, 0.5, 1., 0.6], [0.1, 0.2, 0.9, 0.]],
                size: [0.16, 0.05],
                seed: 7,
                ..Emitter::sparks().rate(120.).lifetime(0.8)
            },
        ));
    }
    for _ in 0..30 {
        exact_game::emitter::step(&w);
    }
    w.propagate();
    w
}

fn at(x: f32, y: f32, z: f32, scale: f32) -> Transform {
    Transform {
        scale: Vec3::splat(scale),
        ..Transform::at(x, y, z)
    }
}

/// A 4×2 atlas, with its mip chain: opaque white and red over half, fading
/// alpha over the rest.
fn tag() -> TextureData {
    let mut mips = vec![[
        [255, 255, 255, 255],
        [255, 255, 255, 255],
        [255, 40, 40, 255],
        [255, 40, 40, 190],
        [40, 40, 40, 140],
        [255, 255, 255, 96],
        [200, 230, 255, 48],
        [255, 255, 255, 0],
    ]
    .concat()];
    // 4×2 → 2×1: each texel the mean of its 2×2 block; → 1×1.
    let texel = |level: &[u8], i: usize, c: usize| u32::from(level[i * 4 + c]);
    let half: Vec<u8> = (0..2)
        .flat_map(|x| {
            let level = &mips[0];
            (0..4).map(move |c| {
                let sum: u32 = [2 * x, 2 * x + 1, 4 + 2 * x, 5 + 2 * x]
                    .iter()
                    .map(|&i| texel(level, i, c))
                    .sum();
                (sum / 4) as u8
            })
        })
        .collect();
    let one: Vec<u8> = (0..4)
        .map(|c| ((texel(&half, 0, c) + texel(&half, 1, c)) / 2) as u8)
        .collect();
    mips.extend([half, one]);
    TextureData {
        width: 4,
        height: 2,
        mips,
        srgb: true,
        ..Default::default()
    }
}

/// `w` with `samples` and `prepass` into `format`, through the HDR image and
/// then direct: each frame's pixels and its count of direct frames so far.
pub(super) fn both(
    gpu: &Gpu,
    w: &World,
    format: wgpu::TextureFormat,
    samples: SceneSamples,
    prepass: bool,
) -> [(fixture::Pixels, u64); 2] {
    let mut r = Renderer::configured(&gpu.device, &gpu.queue, format, samples, prepass);
    for name in ["toon.model", "glow.model", "glass.model"] {
        if let Some(model) = w.model(name) {
            r.prepare_model(name, model).unwrap();
        }
    }
    r.add_texture("tag.tex", &tag()).unwrap();
    let mut feed = Feed::default();
    // The second feed binds the texture, delivered after the first, to its sprites.
    feed.feed(w, &mut r).unwrap();
    feed.feed(w, &mut r).unwrap();
    let texture = target(gpu, SIZE, format);
    [false, true].map(|direct| {
        r.set_direct(direct);
        let input = feed.frame(w, 1., SIZE.0 as f32 / SIZE.1 as f32);
        let stats = r.draw(&texture.create_view(&Default::default()), SIZE, &input);
        (fixture::read(gpu, &texture).unwrap(), stats.direct_frames)
    })
}

/// Pixels whose channels differ by more than `levels`, and the largest difference.
pub(super) fn differing(a: &fixture::Pixels, b: &fixture::Pixels, levels: u8) -> (usize, u8) {
    let mut count = 0;
    let mut most = 0;
    for (p, q) in a.data.chunks(4).zip(b.data.chunks(4)) {
        let d = p.iter().zip(q).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
        most = most.max(d);
        count += usize::from(d > levels);
    }
    (count, most)
}

#[test]
fn a_direct_town_frame_draws_what_its_hdr_frame_draws() {
    let Some(gpu) = gpu() else {
        return;
    };
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    let pixels = (SIZE.0 * SIZE.1) as usize;
    for exposure in [1., 0.75] {
        let w = town(exposure, ToneMapping::None, true, false);
        for (samples, prepass) in [
            (SceneSamples::One, false),
            (SceneSamples::One, true),
            (SceneSamples::Four, false),
        ] {
            let [(hdr, before), (direct, after)] = both(&gpu, &w, srgb, samples, prepass);
            for (pixels, path) in [(&hdr, "hdr"), (&direct, "direct")] {
                pixels.save(&format!("town-{exposure}-{samples:?}-{prepass}-{path}"));
            }
            assert_eq!(
                (before, after),
                (0, 1),
                "{samples:?}: one HDR frame, then direct"
            );
            let (count, most) = differing(&hdr, &direct, 2);
            eprintln!(
                "exposure {exposure}, {samples:?}, prepass {prepass}: {count} pixels differ \
                 by more than 2 levels, at most {most}"
            );
            match samples {
                // Every value it blends lies in [0, 1]: blending in linear light
                // commutes with exposure and with the target's clamp.
                SceneSamples::One => assert_eq!(count, 0, "largest difference {most}"),
                // Each sample is clamped before the resolve: the edges of the
                // sphere above 1 against the ground move (40 pixels here).
                SceneSamples::Four => assert!(count < pixels / 100, "{count}"),
            }
        }
    }
    // The scene shows what it tests: the hulls, and everything that blends.
    let [(_, _), (direct, _)] = both(
        &gpu,
        &town(1., ToneMapping::None, true, false),
        srgb,
        SceneSamples::One,
        true,
    );
    let red = direct
        .data
        .chunks(4)
        .filter(|p| p[0] > 150 && p[0].saturating_sub(p[1].max(p[2])) > 80)
        .count();
    assert!(red > 100, "outline pixels: {red}");
    let [(_, _), (opaque, _)] = both(
        &gpu,
        &town(1., ToneMapping::None, false, false),
        srgb,
        SceneSamples::One,
        true,
    );
    let (covered, _) = differing(&direct, &opaque, 8);
    assert!(covered > pixels / 50, "translucent pixels: {covered}");
}

#[test]
fn only_frames_that_need_no_hdr_image_draw_direct() {
    let Some(gpu) = gpu() else {
        return;
    };
    let direct = |w: &World, format| both(&gpu, w, format, SceneSamples::One, false)[1].1 == 1;
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    let unorm = wgpu::TextureFormat::Rgba8Unorm;
    let blended = town(1., ToneMapping::None, true, false);
    assert!(direct(&blended, srgb));
    // The shader would encode sRGB before the blend, which would mix encoded values.
    assert!(!direct(&blended, unorm));
    // ACES is not linear: blending tonemapped values is not tonemapping the blend.
    assert!(!direct(&town(1., ToneMapping::Aces, true, false), srgb));
    // With nothing to blend, each pixel's tone curve is the tone pass's.
    for tone in [ToneMapping::None, ToneMapping::Aces] {
        assert!(direct(&town(1., tone, false, false), unorm), "{tone:?}");
    }
    let bloom = town(1., ToneMapping::None, false, true);
    assert!(!direct(&bloom, srgb), "bloom reads the HDR image");
}

#[test]
fn a_translucent_fragment_over_light_above_one_blends_with_one() {
    let Some(gpu) = gpu() else {
        return;
    };
    // The difference README "Direct frames" names: a direct frame clamps each
    // fragment to the target before it blends, the HDR path only after.
    let mut w = World::new(60, 1);
    w.insert_resource(Environment {
        background: Some([0.; 3]),
        ambient: 0.,
        fog: None,
        bloom: None,
        tone_mapping: ToneMapping::None,
        ..Default::default()
    });
    w.spawn((Transform::at(0., 0., 4.), Camera::default()));
    let bright = MaterialData {
        base_color: [0., 0., 0., 1.],
        emissive: [3.; 3],
        ..Default::default()
    };
    let bright = w.generated_model("toon.model", panel(bright)).unwrap();
    w.spawn((at(0., 0., 0., 4.), bright));
    let shade = MaterialData {
        base_color: [0., 0., 0., 0.5],
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    };
    let shade = w.generated_model("glass.model", panel(shade)).unwrap();
    w.spawn((at(0., 0., 1., 4.), shade));
    w.propagate();
    let [(hdr, _), (direct, _)] = both(
        &gpu,
        &w,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        SceneSamples::One,
        false,
    );
    let (x, y) = (SIZE.0 / 2, SIZE.1 / 2);
    // 3 × 0.5 = 1.5, clamped to 1; 1 × 0.5 = 0.5, which sRGB encodes as 188.
    assert_eq!(hdr.at(x, y)[..3], [255; 3]);
    assert!(direct.at(x, y)[..3].iter().all(|c| c.abs_diff(188) <= 1));
}

/// Every pixel's stored alpha, from an RGBA8 or `Rgba16Float` texture.
fn stored_alpha(gpu: &Gpu, texture: &wgpu::Texture) -> Vec<f32> {
    let texel = texture.format().block_copy_size(None).unwrap();
    let row = (texture.width() * texel).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * texture.height()),
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
        texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap();
    let half = |bits: u16| {
        let (e, m) = (i32::from(bits >> 10 & 31), f32::from(bits & 1023));
        assert!(bits >> 15 == 0 && e != 31, "alpha {bits:#06x}");
        if e == 0 {
            m * 2f32.powi(-24)
        } else {
            (1. + m / 1024.) * 2f32.powi(e - 15)
        }
    };
    data.chunks(row as usize)
        .flat_map(|r| r[..(texture.width() * texel) as usize].chunks(texel as usize))
        .map(|p| match texel {
            4 => f32::from(p[3]) / 255.,
            _ => half(u16::from_le_bytes([p[6], p[7]])),
        })
        .collect()
}

/// The opacity promise (`Surface::opaque`) without render hooks: every
/// clear of the output stores alpha one, opaque draws write one, and every
/// blend composes alpha OVER, which keeps a destination alpha of one at one.
/// Each path that blends into the output, direct and through the HDR image,
/// into 8-bit and half-float targets.
#[test]
fn every_output_pixel_stores_alpha_one() {
    let Some(gpu) = gpu() else {
        return;
    };
    use wgpu::TextureFormat::{Rgba16Float, Rgba8Unorm, Rgba8UnormSrgb};
    for (tone, bloom) in [
        (ToneMapping::None, false),
        (ToneMapping::Aces, false),
        (ToneMapping::None, true),
    ] {
        let w = town(1., tone, true, bloom);
        for format in [Rgba8UnormSrgb, Rgba8Unorm, Rgba16Float] {
            let mut r =
                Renderer::configured(&gpu.device, &gpu.queue, format, SceneSamples::One, true);
            for name in ["toon.model", "glow.model", "glass.model"] {
                r.prepare_model(name, w.model(name).unwrap()).unwrap();
            }
            r.add_texture("tag.tex", &tag()).unwrap();
            let mut feed = Feed::default();
            feed.feed(&w, &mut r).unwrap();
            feed.feed(&w, &mut r).unwrap();
            let texture = target(&gpu, SIZE, format);
            for direct in [false, true] {
                r.set_direct(direct);
                let input = feed.frame(&w, 1., SIZE.0 as f32 / SIZE.1 as f32);
                r.draw(&texture.create_view(&Default::default()), SIZE, &input);
                let lowest = stored_alpha(&gpu, &texture)
                    .into_iter()
                    .fold(1f32, f32::min);
                assert!(
                    lowest > 0.999,
                    "{tone:?}, bloom {bloom}, {format:?}, direct {direct}: alpha {lowest}"
                );
            }
        }
    }
}
