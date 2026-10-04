//! What `ModelLod` saves on a forest: 20,000 trees, triangles the camera draws
//! and whole-frame time at 1080p, with and without a coarse far level.
//! `cargo test -p exact-game-render --release --test lod_bench -- --ignored --nocapture`
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::{ModelPresentation, WorldSurface};
use exact_gpu::{fixture, wgpu, Frame, Surface, Value};

// A closed-ish trunk: `segments` around, `rings` up, 1 m radius, 8 m tall.
fn trunk(segments: u32, rings: u32) -> asset::MeshData {
    let mut m = asset::MeshData {
        bounds: [-1., 0., -1., 1., 8., 1.],
        ..Default::default()
    };
    for r in 0..=rings {
        for s in 0..=segments {
            let a = s as f32 / segments as f32 * std::f32::consts::TAU;
            let (x, z) = (a.cos(), a.sin());
            m.positions.extend([x, 8. * r as f32 / rings as f32, z]);
            m.normals.extend([x, 0., z]);
            m.uvs
                .extend([s as f32 / segments as f32, r as f32 / rings as f32]);
        }
    }
    for r in 0..rings {
        for s in 0..segments {
            let i = r * (segments + 1) + s;
            let j = i + segments + 1;
            m.indices.extend([i, j, i + 1, i + 1, j, j + 1]);
        }
    }
    m
}

#[derive(Default, Args)]
struct ForestArgs {
    lod: bool,
}
struct Forest;
impl Game for Forest {
    const ID: &'static str = "lod-bench";
    type Args = ForestArgs;
    fn setup(w: &mut World, args: &ForestArgs) {
        w.spawn((
            Transform::at(0., 12., -10.).looking_at(Vec3::new(0., 0., 100.), Vec3::Y),
            Camera::default(),
        ));
        w.spawn((
            Transform::at(5., 10., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        let tree = w.generated("tree.model", trunk(32, 32)).unwrap();
        w.generated("tree_low.model", trunk(6, 1)).unwrap();
        // 20,000 trees, 4 m apart, ahead of the camera.
        for i in 0..20_000 {
            let (x, z) = ((i % 100) as f32 * 4. - 200., (i / 100) as f32 * 4.);
            let e = w.spawn((Transform::at(x, 0., z), tree.clone()));
            if args.lod {
                w.insert(
                    e,
                    ModelLod {
                        levels: vec![LodLevel {
                            distance: 40.,
                            model: "tree_low.model".into(),
                        }],
                        hide: Some(400.),
                    },
                );
            }
        }
    }
    fn tick(_: &mut World, _: &Input, _: &ForestArgs) {}
}

#[test]
#[ignore = "GPU timing measurement; GPU host"]
fn model_lod_cost_on_twenty_thousand_trees() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let run = |lod: bool| {
        let mut s = WorldSurface::<Forest, ModelPresentation, true>::default();
        s.device_ready(gpu.device.features());
        s.bind(&[Value::Bool(lod)], None).unwrap();
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        s.agent(r#"{"op":"state","perf":true}"#);
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let target = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 1920,
                    height: 1080,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let mut f = Frame {
            width: 960.,
            height: 540.,
            scale: 2.,
            now_ms: 0.,
            seekable: false,
            period_ms: 16.,
            children_generation: 0,
            shader_generation: 0,
        };
        let mut ms = Vec::new();
        for i in 0..100 {
            f.now_ms += 16.;
            let start = std::time::Instant::now();
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            s.render(&f, &gpu.device, &gpu.queue, &mut encoder, &target, format);
            gpu.queue.submit([encoder.finish()]);
            s.submitted();
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            if i >= 30 {
                ms.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
        assert!(s.error().is_none(), "{:?}", s.error());
        ms.sort_by(f64::total_cmp);
        let state = s.agent(r#"{"op":"state","perf":true}"#).unwrap_or_default();
        let field = |key: &str| {
            state.find(key).map_or(0, |i| {
                state[i + key.len()..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
        };
        (
            ms[ms.len() / 2],
            field("\"cameraTriangles\":"),
            field("\"camera\":"),
        )
    };
    for round in 0..3 {
        let (off, on) = (run(false), run(true));
        eprintln!(
            "LOD round {round}: without {:.2} ms, {} camera triangles, {} instances; \
             with {:.2} ms, {} camera triangles, {} instances",
            off.0, off.1, off.2, on.0, on.1, on.2
        );
    }
}
