#![cfg(not(target_arch = "wasm32"))]
use exact_game::*;
#[path = "fixture/model.rs"]
mod skin_fixture;
#[path = "fixture/device.rs"]
mod test_device;
fn assets() -> std::collections::BTreeMap<String, Vec<u8>> {
    use exact_game::asset::{Clip, Track, TrackPath};
    let mut model = skin_fixture::skinned_model();
    model.clips = vec![Clip {
        name: "turn".into(),
        tracks: vec![Track {
            node: model.skins[0].joints[0],
            path: TrackPath::Rotation,
            times: vec![0., 1.],
            values: [
                Quat::IDENTITY.to_array(),
                Quat::from_rotation_y(2.).to_array(),
            ]
            .concat(),
            ..Default::default()
        }],
        ..Default::default()
    }];
    let mut assets = std::collections::BTreeMap::new();
    assets.insert("rig.model".into(), bin::to_vec(&model));
    assets.insert(
        model.textures[0].clone(),
        include_bytes!("../../bake/tests/fixtures/crate/0-srgb-straight.tex").to_vec(),
    );
    assets
}
#[test]
fn first_presented_skin_matches_current_pose_in_its_rectangle() {
    use exact_game_render::{
        exact_gpu::{fixture, wgpu, Frame, Surface},
        WorldSurface,
    };
    struct Birth<const HISTORY: u8>;
    impl<const HISTORY: u8> Game for Birth<HISTORY> {
        const ID: &'static str = "skin-birth";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let mut clip = Animation::play("turn").speed(0.);
            clip.time = 0.3;
            w.spawn_named(
                "rig",
                (Transform::default(), Mesh::asset("rig.model"), clip),
            );
            w.spawn((
                Transform::at(6., 3.4, 7.).looking_at(Vec3::new(0., 0.9, 0.), Vec3::Y),
                Camera::default(),
            ));
            w.insert_resource(Environment {
                fog: None,
                ..Default::default()
            });
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            animation::step(w);
            if HISTORY != 0 {
                let bind = animation::bind_pose(w.model("rig.model").unwrap());
                let mut p = w.require_mut::<Pose>("rig");
                p.previous = if HISTORY == 1 { p.local.clone() } else { bind };
            }
        }
    }
    let Some(gpu) =
        crate::test_device::device_or_skip(exact_game_render::exact_gpu::fixture::device())
    else {
        return;
    };
    fn first<const H: u8>(gpu: &exact_game_render::exact_gpu::Gpu, event: &str) -> fixture::Pixels {
        let data = assets();
        let mut s = WorldSurface::<Birth<H>, exact_game_render::ModelExecutor, true>::default();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.bind(&[], None).unwrap();
        for _ in 0..16 {
            for n in s.assets().requests {
                s.asset(&n, Ok(&data[&n]));
            }
            s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        }
        let mut f = Frame {
            width: 1280.,
            height: 720.,
            scale: 1.,
            now_ms: 0.,
            seekable: false,
            period_ms: 1000. / 60.,
            children_generation: 0,
            shader_generation: 0,
        };
        fixture::render(gpu, &mut s, &f).unwrap();
        f.now_ms = 1000. / 240.;
        let (mut image, _) = fixture::render(gpu, &mut s, &f).unwrap();
        if event != "birth" {
            match event {
                "restore" | "carry" => {
                    let saved = s.carry().unwrap().unwrap();
                    s.restore(
                        &saved,
                        if event == "restore" {
                            exact_game_render::exact_gpu::Restore::Open
                        } else {
                            exact_game_render::exact_gpu::Restore::Carry
                        },
                    )
                    .unwrap();
                }
                "model arrival" => {
                    s.device_lost();
                    s.device_ready(exact_gpu::wgpu::Features::empty());
                    for _ in 0..16 {
                        for n in s.assets().requests {
                            s.asset(&n, Ok(&data[&n]));
                        }
                        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
                    }
                }
                _ => unreachable!(),
            }
            // Restore rebases at the saved tick. A quarter-tick horizon keeps that
            // tick intact while exercising a nonzero interpolation alpha.
            f.period_ms = 1000. / 240.;
            image = fixture::render(gpu, &mut s, &f).unwrap().0;
        }
        assert_eq!(s.sim().unwrap().world().tick(), 1);
        assert!(s.take_error().is_none());
        image
    }
    for event in ["birth", "restore", "carry", "model arrival"] {
        let actual = if event == "birth" {
            first::<0>(&gpu, event)
        } else {
            first::<2>(&gpu, event)
        };
        let reference = first::<1>(&gpu, event);
        let bind_flash = first::<2>(&gpu, "birth");
        let differs = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).any(|(a, b)| a.abs_diff(b) > 2);
        // The deliberately corrupted history locates the skin's affected rectangle;
        // background pixels cannot dilute the tolerance.
        let (mut x0, mut y0, mut x1, mut y1) = (reference.width, reference.height, 0, 0);
        let mut bind_changes = 0;
        for y in 0..reference.height {
            for x in 0..reference.width {
                if differs(reference.at(x, y), bind_flash.at(x, y)) {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                    bind_changes += 1;
                }
            }
        }
        assert!(
            bind_changes > 50,
            "the oracle must detect a small skin bind flash"
        );
        let area = (x1 - x0 + 1) * (y1 - y0 + 1);
        assert!(
            area < reference.width * reference.height / 10,
            "skin rectangle is local: {area}"
        );
        let mut changed = 0;
        for y in y0..=y1 {
            for x in x0..=x1 {
                changed += u32::from(differs(actual.at(x, y), reference.at(x, y)));
            }
        }
        println!("{event}: skin rectangle {x0},{y0}..{x1},{y1}: changes {changed}/{area}, bind flash {bind_changes}");
        assert!(
            changed == 0,
            "{event} must match current/current: zero changed skin-rectangle pixels"
        );
    }
}

#[test]
fn rigid_node_animation_moves_pixels_and_glow_dims_baked_emission() {
    use exact_game::asset::{Clip, MaterialData, Track, TrackPath};
    use exact_game_render::{
        exact_gpu::{fixture, wgpu, Frame, Surface},
        WorldSurface,
    };
    struct Rigid;
    impl Game for Rigid {
        const ID: &'static str = "rigid-glow";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            let b = w.model("rig.model").unwrap().bounds;
            let center = (Vec3::from_slice(&b[..3]) + Vec3::from_slice(&b[3..])) * 0.5;
            w.spawn_named(
                "rig",
                (
                    Transform {
                        position: -center,
                        ..Default::default()
                    },
                    Mesh::asset("rig.model"),
                    Animation::play("shift").once(),
                    Glow(Tween::new(1.)),
                ),
            );
            w.spawn((
                Transform::at(0., 0., 8.).looking_at(Vec3::ZERO, Vec3::Y),
                Camera::default(),
            ));
            w.insert_resource(Environment {
                fog: None,
                background: Some([0.; 3]),
                ..Default::default()
            });
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            animation::step(w);
            if w.tick() == 29 {
                w.require_mut::<Glow>("rig").0 = Tween::new(0.);
            }
            if w.tick() == 59 {
                w.remove::<Glow>(w.named("rig").unwrap());
            }
        }
    }
    let Some(gpu) = test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut model = skin_fixture::skinned_model();
    model.skins.clear();
    model.textures.clear();
    model.materials = vec![MaterialData {
        base_color: [0., 0., 0., 1.],
        emissive: [1., 0., 0.],
        ..Default::default()
    }];
    for mesh in &mut model.meshes {
        mesh.joints.clear();
        mesh.weights.clear();
        mesh.material = 0;
    }
    for node in &mut model.nodes {
        node.skin = None;
    }
    let node = model.nodes.iter().position(|n| n.mesh.is_some()).unwrap() as u32;
    let bind = animation::bind_pose(&model);
    let start = Vec3::from_slice(&bind[node as usize * 10..]);
    model.clips = vec![Clip {
        name: "shift".into(),
        tracks: vec![Track {
            node,
            path: TrackPath::Translation,
            times: vec![0., 1.],
            values: [start.to_array(), (start + Vec3::X * 2.).to_array()].concat(),
            ..Default::default()
        }],
        ..Default::default()
    }];
    let mut surface = WorldSurface::<Rigid, exact_game_render::ModelExecutor, true>::default();
    surface.device_ready(wgpu::Features::empty());
    surface.bind(&[], None).unwrap();
    assert_eq!(surface.assets().requests, ["rig.model"]);
    surface.asset("rig.model", Ok(&bin::to_vec(&model)));
    surface.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let mut frame = Frame {
        width: 320.,
        height: 240.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let render = |surface: &mut WorldSurface<Rigid, exact_game_render::ModelExecutor, true>,
                  frame: &Frame| {
        let (image, _) = fixture::render(&gpu, surface, frame).unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        image
    };
    let red = |p: [u8; 4]| p[0] > 80 && p[0] > p[1] * 2 && p[0] > p[2] * 2;
    let center = |image: &fixture::Pixels| {
        let mut sum = 0.;
        let mut count = 0;
        for y in 0..image.height {
            for x in 0..image.width {
                if red(image.at(x, y)) {
                    sum += x as f32;
                    count += 1;
                }
            }
        }
        assert!(count > 100, "the emissive mesh must be visible");
        sum / count as f32
    };
    let initial = render(&mut surface, &frame);
    frame.now_ms = 250.;
    let moving = render(&mut surface, &frame);
    assert!(
        center(&moving) > center(&initial) + 8.,
        "an unskinned node must move its actual pixels"
    );
    frame.now_ms = 500.;
    let dimmed = render(&mut surface, &frame);
    assert_eq!(
        dimmed.count(red),
        0,
        "Glow(0) must extinguish baked model emission"
    );
    frame.now_ms = 1000.;
    let restored = render(&mut surface, &frame);
    assert!(
        center(&restored) > center(&moving) + 8.,
        "removing Glow restores baked emission at the animated pose"
    );
}
