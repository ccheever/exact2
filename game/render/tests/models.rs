#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::{asset::Model, *};
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Surface};
#[path = "../../bake/tests/samples.rs"]
mod samples;
struct ModelGame;
impl Game for ModelGame {
    const ID: &'static str = "model-fixture";
    const ASSETS: &'static [&'static str] = &["sample.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let b = w.model("sample.model").unwrap().bounds;
        let lo = Vec3::from_slice(&b[..3]);
        let hi = Vec3::from_slice(&b[3..]);
        let scale = 2.0 / (hi - lo).max_element();
        let center = (hi + lo) * 0.5;
        w.spawn_named(
            "model",
            (
                Transform {
                    position: -center * scale,
                    scale: Vec3::splat(scale),
                    ..Default::default()
                },
                Mesh::asset("sample.model"),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::at(2.5, 1.3, 3.5).looking_at(Vec3::ZERO, Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn((
            Transform::at(3., 4., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                illuminance: 18000.,
                shadows: true,
                ..Default::default()
            },
        ));
        w.insert_resource(Environment {
            fog: None,
            background: Some([0.055, 0.065, 0.085]),
            ..Default::default()
        });
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
fn draw(gpu: &exact_gpu::Gpu, model: &Model, name: &str) -> fixture::Pixels {
    let mut surface =
        WorldSurface::<ModelGame, exact_game_render::ModelPresentation, true>::default();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    surface.bind(&[], None).unwrap();
    assert_eq!(surface.assets().requests, ["sample.model"]);
    surface.asset("sample.model", Ok(&bin::to_vec(model)));
    surface.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    for texture in surface.assets().requests {
        let path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Caches/exact2-game/gltf-samples")
            .join(&texture);
        surface.asset(&texture, Ok(&std::fs::read(path).unwrap()));
    }
    surface.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    assert!(surface.error().is_none(), "{:?}", surface.error());
    let frame = Frame {
        width: 800.,
        height: 800.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.0,
        children_generation: 0,
        shader_generation: 0,
    };
    let (pixels, _) = fixture::render(gpu, &mut surface, &frame).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    pixels.save(name);
    let reply = surface
        .agent(r#"{"op":"layout","entity":"model","width":800,"height":800}"#)
        .unwrap();
    assert!(reply.contains("\"bounds\""), "{reply}");
    pixels
}
#[test]
fn textured_samples_and_normal_emissive_differences() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        eprintln!("SKIP models: no GPU adapter");
        return;
    };
    for name in ["BoxTextured", "DamagedHelmet", "Fox"] {
        let model = samples::sample(name);
        let pixels = draw(&gpu, &model, name);
        let (mut lo, mut hi) = (255u8, 0u8);
        for y in 150..650 {
            for x in 150..650 {
                let p = pixels.at(x, y);
                lo = lo.min(p[0]);
                hi = hi.max(p[0]);
            }
        }
        assert!(hi - lo > 50, "{name} lacks visible texture contrast");
        if name == "DamagedHelmet" {
            let mut flat = model.clone();
            let removed = flat.materials[0].normal_texture.unwrap();
            flat.textures.remove(removed as usize);
            for m in &mut flat.materials {
                m.normal_texture = None;
                for index in [
                    &mut m.base_color_texture,
                    &mut m.metallic_roughness_texture,
                    &mut m.emissive_texture,
                    &mut m.occlusion_texture,
                ]
                .into_iter()
                .flatten()
                {
                    if *index > removed {
                        *index -= 1;
                    }
                }
            }
            let flat = draw(&gpu, &flat, "DamagedHelmet-no-normal");
            let mut dark = model.clone();
            for m in &mut dark.materials {
                m.emissive = [0.; 3];
            }
            let dark = draw(&gpu, &dark, "DamagedHelmet-no-emission");
            let difference = |other: &fixture::Pixels| {
                (0..800)
                    .flat_map(|y| (0..800).map(move |x| (x, y)))
                    .filter(|&(x, y)| {
                        let a = pixels.at(x, y);
                        let b = other.at(x, y);
                        (0..3).any(|i| a[i].abs_diff(b[i]) > 8)
                    })
                    .count()
            };
            eprintln!(
                "helmet pixel changes: normal={}, emissive={}",
                difference(&flat),
                difference(&dark)
            );
            assert!(
                difference(&flat) > 1000,
                "normal map must change visible lighting"
            );
            assert!(
                difference(&dark) > 100,
                "emission must light visible pixels"
            );
        }
    }
}
