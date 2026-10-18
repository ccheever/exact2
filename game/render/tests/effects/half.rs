//! Half-precision shading (pipeline.rs `half_precision`): where the device has
//! SHADER_F16 the forward colour arithmetic runs in f16, elsewhere in f32.
use super::super::support::{gpu, target};
use super::direct::{both, differing, sphere, town, SIZE};
use exact_game::asset::MaterialData;
use exact_game::{
    Camera, DirectionalLight, Environment, SceneSamples, ToneMapping, Transform, Vec3, World,
};
use exact_game_render::{Feed, Renderer};
use exact_gpu::{fixture, wgpu, Gpu};

/// The device with half precision, and the same device without.
fn devices() -> Option<(Gpu, Gpu)> {
    let half = gpu()?;
    if !half.device.features().contains(wgpu::Features::SHADER_F16) {
        eprintln!("this device has no SHADER_F16: every shader is f32");
        return None;
    }
    let full = fixture::device_without(wgpu::Features::SHADER_F16).unwrap();
    assert!(!full.device.features().contains(wgpu::Features::SHADER_F16));
    Some((half, full))
}

#[test]
fn half_precision_draws_the_town_within_two_levels() {
    let Some((half, full)) = devices() else {
        return;
    };
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    for (tone, bloom) in [(ToneMapping::None, false), (ToneMapping::Aces, true)] {
        let w = town(1., tone, true, bloom);
        for (samples, prepass) in [
            (SceneSamples::One, false),
            (SceneSamples::One, true),
            (SceneSamples::Four, false),
        ] {
            let halves = both(&half, &w, srgb, samples, prepass);
            let fulls = both(&full, &w, srgb, samples, prepass);
            for (path, (h, f)) in ["hdr", "direct"].iter().zip(halves.iter().zip(&fulls)) {
                for (frame, float) in [(&h.0, "f16"), (&f.0, "f32")] {
                    frame.save(&format!("{float}-{tone:?}-{samples:?}-{prepass}-{path}"));
                }
                let (count, most) = differing(&h.0, &f.0, 2);
                eprintln!(
                    "{tone:?}, {samples:?}, prepass {prepass}, {path}: {count} pixels \
                     differ by more than 2 levels, at most {most}"
                );
                assert_eq!(count, 0, "largest difference {most}");
            }
        }
    }
}

/// A glossy metal and a glossy dielectric under a bright sun: specular peaks
/// beyond the half range are capped, not infinite, and still draw white.
#[test]
fn half_precision_keeps_glossy_highlights_finite() {
    let Some((half, full)) = devices() else {
        return;
    };
    let draw = |gpu: &Gpu| {
        let mut w = World::new(60, 1);
        w.insert_resource(Environment {
            background: Some([0.02; 3]),
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.spawn((
            Transform::at(2., 4., 6.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                illuminance: 6. / 0.0003,
                shadows: false,
                ..Default::default()
            },
        ));
        w.spawn((Transform::at(0., 0., 4.), Camera::default()));
        for (i, (metallic, roughness)) in [(1., 0.05), (0., 0.045)].into_iter().enumerate() {
            let name = format!("gloss{i}.model");
            let model = sphere(
                MaterialData {
                    base_color: [0.9, 0.7, 0.5, 1.],
                    metallic,
                    roughness,
                    ..Default::default()
                },
                false,
            );
            let mesh = w.generated_model(&name, model).unwrap();
            w.spawn((Transform::at(-0.7 + 1.4 * i as f32, 0., 0.), mesh));
        }
        w.propagate();
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut r = Renderer::new(&gpu.device, &gpu.queue, format);
        for i in 0..2 {
            let name = format!("gloss{i}.model");
            r.prepare_model(&name, w.model(&name).unwrap()).unwrap();
        }
        let mut feed = Feed::default();
        feed.feed(&w, &mut r).unwrap();
        let texture = target(gpu, SIZE, format);
        let input = feed.frame(&w, 1., SIZE.0 as f32 / SIZE.1 as f32);
        r.draw(&texture.create_view(&Default::default()), SIZE, &input);
        fixture::read(gpu, &texture).unwrap()
    };
    let (h, f) = (draw(&half), draw(&full));
    let white = |p: &fixture::Pixels| p.data.chunks(4).filter(|c| c[..3] == [255; 3]).count();
    assert!(white(&f) > 0, "the fixture has highlights");
    let (count, most) = differing(&h, &f, 2);
    eprintln!("glossy: {count} pixels differ by more than 2 levels, at most {most}");
    assert_eq!(count, 0, "largest difference {most}");
}
