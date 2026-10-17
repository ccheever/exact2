use super::*;
use exact_game::{asset::Model, *};
use glam::{Mat4, Quat, Vec3};

#[derive(Default, Component)]
struct Gait {
    angle: f32,
    shown: bool,
    overflow: bool,
}
struct Scene;
impl Game for Scene {
    type Args = ();
    const ID: &'static str = "pose-span-proof";
    fn setup(w: &mut World, _: &()) {
        let mesh = w
            .generated_model("posed.model", crate::test_model::posable_model())
            .unwrap();
        let e = w.spawn_named(
            "actor",
            (
                Transform::default(),
                mesh,
                Gait {
                    angle: 0.4,
                    shown: true,
                    overflow: false,
                },
                Animation::play("bend"),
            ),
        );
        w.spawn_named(
            "charm",
            (Transform::default(), SocketFollow::new(e, "hand")),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        animation::step(w);
    }
    fn present(p: &mut Present<'_>, _: &()) {
        p.insert_each::<Gait, NodeOffsets>(|_, _, g| {
            g.shown.then(|| {
                if g.overflow {
                    NodeOffsets::default()
                        .with(NodeName::new("arm"), Transform::at(f32::MAX * 0.75, 0., 0.))
                } else {
                    rows(g.angle)
                }
            })
        });
    }
}
fn rows(angle: f32) -> NodeOffsets {
    NodeOffsets::default().with(
        NodeName::new("arm"),
        Transform {
            position: Vec3::new(0.1, 0.2, 0.3),
            rotation: Quat::from_rotation_x(angle),
            scale: Vec3::new(1.2, 0.8, 1.),
        },
    )
}
fn setup_skin(gpu: &exact_gpu::Gpu, model: &Model, e: Entity) -> (Skinning, wgpu::Buffer) {
    let mut skin = Skinning::new(&gpu.device, None);
    let templates = skin.add(&gpu.device, &gpu.queue, model);
    let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 80,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let records: Vec<_> = templates
        .iter()
        .map(|&t| DrawInstance {
            data: 0,
            transform: e.index(),
            geometry: crate::MeshId(0),
            material: crate::MaterialId(0),
            local: Mat4::IDENTITY,
            skin: Some(t),
            tint: [1.; 4],
            glow: [0.; 3],
        })
        .collect();
    skin.set(&gpu.device, &gpu.queue, &uniform, &records)
        .unwrap();
    (skin, uniform)
}
fn close(a: Mat4, b: Mat4) {
    for (a, b) in a.to_cols_array().into_iter().zip(b.to_cols_array()) {
        assert!((a - b).abs() < 2e-5, "{a} != {b}");
    }
}
#[test]
fn clip_then_ik_then_offset_palettes_and_socket_match_at_all_alphas() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let model = crate::test_model::posable_model();
    model.validate().unwrap();
    let mut sim = Sim::<Scene>::new(()).unwrap();
    sim.run(50.);
    let e = sim.world().named("actor").unwrap();
    let (mut skin, uniform) = setup_skin(&gpu, &model, e);
    skin.feed(&gpu.queue, sim.world(), &[e], true);
    let mut attachments = crate::world::scene::Attachments::default();
    attachments.feed(sim.world(), true, true, false, false);
    sim.run(17.);
    // The base endpoints are sampled clips followed by the real two-bone solver.
    let rest = animation::bind_pose(&model);
    let mut endpoints = [Vec::new(), Vec::new()];
    for (i, time) in [0.2, 0.7].into_iter().enumerate() {
        animation::sample(&model.clips[0], time, &rest, &mut endpoints[i]);
        animation::solve_ik(
            &model,
            &mut endpoints[i],
            &Ik {
                chain: ["root".into(), "arm".into(), "hand".into()],
                target: Vec3::new(1.2, 1.5, 0.),
                pole: Vec3::Z,
                weight: 0.7,
            },
        )
        .unwrap();
    }
    sim.world_mut().remove::<Animation>(e);
    let mut pose = Pose::default();
    pose.previous = endpoints[0].clone();
    pose.local = endpoints[1].clone();
    sim.world_mut().insert(e, pose);
    skin.feed(&gpu.queue, sim.world(), &[e], false); // first Pose was already present from Animation
    assert_eq!(skin.records.len(), 1, "three jobs share one instance span");
    assert_eq!(skin.pose_words.len(), model.nodes.len() * 20);
    assert!(skin.jobs_words.chunks_exact(4).all(|j| j[1] == 0));
    attachments.feed(sim.world(), false, true, false, false);
    for alpha in [0., 0.5, 1.] {
        let mut frame = [0.; 20];
        frame[19] = alpha;
        gpu.queue.write_buffer(&uniform, 0, bytes(&frame));
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        skin.encode(&mut encoder, None);
        gpu.queue.submit([encoder.finish()]);
        let data = tests::read(&gpu, &skin.palette.raw, 3 * 64);
        let packed = skin
            .pose_words
            .chunks_exact(model.nodes.len() * 10)
            .collect::<Vec<_>>();
        let mut interpolated = vec![0.; rest.len()];
        for n in 0..model.nodes.len() {
            let at = n * 10;
            let a = crate::node_poses::read(&packed[0][at..]);
            let b = crate::node_poses::read(&packed[1][at..]);
            crate::node_poses::put(
                &mut interpolated[at..at + 10],
                Transform {
                    position: a.position.lerp(b.position, alpha),
                    rotation: a.rotation.slerp(b.rotation, alpha).normalize(),
                    scale: a.scale.lerp(b.scale, alpha),
                },
            );
        }
        for (j, node) in [2, 4, 5].into_iter().enumerate() {
            let actual: Vec<_> = data[j * 64..j * 64 + 64]
                .chunks_exact(4)
                .map(|b| f32::from_ne_bytes(b.try_into().unwrap()))
                .collect();
            close(
                Mat4::from_cols_slice(&actual),
                animation::joint_matrix(&model, &interpolated, node),
            );
        }
        attachments.frame(alpha);
        let charm = sim.world().named("charm").unwrap();
        let drawn = attachments
            .output
            .iter()
            .find(|v| v.entity == charm)
            .unwrap();
        close(
            drawn.matrix,
            animation::joint_matrix(&model, &interpolated, 2),
        );
    }
    // Noncommuting rotated rest makes the node-frame rule observable.
    let at = 10;
    let expected = crate::node_poses::compose(
        crate::node_poses::read(&endpoints[1][at..]),
        rows(0.4).rows()[0].transform,
    )
    .unwrap();
    assert_eq!(
        crate::node_poses::read(&skin.pose_words[rest.len() + at..]).position,
        expected.position
    );
}

#[test]
fn endpoint_history_same_tick_removal_reset_and_invalid_rows() {
    let mut sim = Sim::<Scene>::new(()).unwrap();
    let e = sim.world().named("actor").unwrap();
    let mut history = crate::node_poses::Histories::default();
    history.update(sim.world(), true);
    sim.run(50.);
    history.update(sim.world(), false);
    let first = history.pair(e, false);
    sim.world_mut().get_mut::<Gait>(e).unwrap().angle = 0.8;
    sim.run(0.);
    history.update(sim.world(), false);
    let pair = history.pair(e, false);
    assert_eq!(
        pair[0].rows()[0].transform.rotation,
        first[0].rows()[0].transform.rotation
    );
    assert_ne!(
        pair[0].rows()[0].transform.rotation,
        pair[1].rows()[0].transform.rotation
    );
    sim.world_mut().get_mut::<Gait>(e).unwrap().shown = false;
    sim.run(17.);
    history.update(sim.world(), false);
    let removed = history.pair(e, false);
    assert_eq!(removed[0].rows().len(), 1);
    assert!(removed[1].rows().is_empty());
    sim.run(17.);
    history.update(sim.world(), false);
    assert!(history.pair(e, false).iter().all(|p| p.rows().is_empty()));
    sim.world_mut().get_mut::<Gait>(e).unwrap().shown = true;
    sim.run(17.);
    history.update(sim.world(), false);
    assert!(history.pair(e, false)[0].rows().is_empty());
    assert_eq!(
        history.pair(e, true)[0].rows()[0].transform.rotation,
        history.pair(e, true)[1].rows()[0].transform.rotation
    );
    let base = Transform {
        rotation: Quat::from_rotation_z(0.3),
        scale: Vec3::new(2., 3., 1.),
        ..Transform::default()
    };
    let zero = Transform {
        rotation: Quat::from_xyzw(0., 0., 0., 0.),
        ..Transform::default()
    };
    let result = crate::node_poses::compose(base, zero).unwrap();
    assert!(result.rotation.abs_diff_eq(base.rotation, 1e-6));
    for invalid in [
        Transform::default().with_scale(Vec3::ZERO),
        Transform::at(f32::NAN, 0., 0.),
        Transform::at(f32::INFINITY, 0., 0.),
        Transform::default().with_scale(-Vec3::ONE),
        Transform::at(f32::MAX, f32::MAX, 0.),
    ] {
        assert!(crate::node_poses::compose(base, invalid).is_none());
    }
    let mut model = crate::test_model::posable_model();
    model.nodes[1].posable = false;
    let untouched = crate::node_poses::node_pair([base; 2], [rows(0.4); 2], &model.nodes[1]);
    assert_eq!(
        untouched, [base; 2],
        "a LOD without marked names ignores rows"
    );
}

#[test]
fn pack_primes_first_pose_birth_teleport_generation_replacement_and_lod() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let model = crate::test_model::posable_model();
    let mut sim = Sim::<Scene>::new(()).unwrap();
    sim.run(50.);
    let e = sim.world().named("actor").unwrap();
    sim.world_mut().remove::<Animation>(e);
    let (mut skin, _) = setup_skin(&gpu, &model, e);
    let len = model.nodes.len() * 10;
    let primed = |s: &Skinning| assert_eq!(s.pose_words[..len], s.pose_words[len..]);
    skin.pack(sim.world(), &[e], false);
    primed(&skin);
    let mut attachments = crate::world::scene::Attachments::default();
    attachments.feed(sim.world(), true, true, false, false);
    attachments.frame(0.5);
    let charm = sim.world().named("charm").unwrap();
    close(
        attachments
            .output
            .iter()
            .find(|a| a.entity == charm)
            .unwrap()
            .matrix,
        animation::joint_matrix(&model, &skin.pose_words[len..], 2),
    );
    let mut pose = Pose::default();
    pose.previous = animation::bind_pose(&model);
    pose.local = pose.previous.clone();
    pose.local[0] = 3.;
    sim.world_mut().insert(e, pose);
    skin.pack(sim.world(), &[e], false);
    primed(&skin);
    attachments.feed(sim.world(), false, false, false, false);
    attachments.frame(0.5);
    close(
        attachments
            .output
            .iter()
            .find(|a| a.entity == charm)
            .unwrap()
            .matrix,
        animation::joint_matrix(&model, &skin.pose_words[len..], 2),
    );
    sim.world_mut().get_mut::<Pose>(e).unwrap().local[0] = 4.;
    skin.pack(sim.world(), &[e], false);
    assert_eq!(skin.pose_words[0], 3., "retain the first-pose prime");
    assert_eq!(skin.pose_words[len], 4.);
    attachments.feed(sim.world(), false, false, false, false);
    attachments.frame(0.5);
    let mut midpoint = skin.pose_words[len..].to_vec();
    midpoint[0] = 3.5;
    close(
        attachments
            .output
            .iter()
            .find(|a| a.entity == charm)
            .unwrap()
            .matrix,
        animation::joint_matrix(&model, &midpoint, 2),
    );
    sim.run(17.);
    sim.world_mut().teleport(e, Transform::at(5., 0., 0.));
    skin.pack(sim.world(), &[e], false);
    primed(&skin);
    sim.run(17.);
    // Replacement marks all of the model's templates, including its first span.
    skin.mark_fresh(&[2]);
    skin.pack(sim.world(), &[e], false);
    primed(&skin);
    for t in &mut skin.templates {
        t.fresh = false;
    }
    // A newly prepared LOD has a separate model identity and span.
    let other = skin.add(&gpu.device, &gpu.queue, &model)[0];
    let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 80,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    let record = DrawInstance {
        data: 0,
        transform: e.index(),
        geometry: crate::MeshId(0),
        material: crate::MaterialId(0),
        local: Mat4::IDENTITY,
        skin: Some(other),
        tint: [1.; 4],
        glow: [0.; 3],
    };
    skin.set(&gpu.device, &gpu.queue, &uniform, &[record])
        .unwrap();
    skin.pack(sim.world(), &[e], false);
    primed(&skin);
    let saved = sim.world().save();
    sim.world_mut().load(&saved).unwrap();
    skin.pack(sim.world(), &[e], false);
    primed(&skin);
    let previous = skin.pose_words[..len].to_vec();
    sim.world_mut().get_mut::<Pose>(e).unwrap().local[0] += 1.;
    sim.world_mut().get_mut::<Gait>(e).unwrap().angle += 0.3;
    sim.run(0.);
    skin.pack(sim.world(), &[e], false);
    assert_eq!(
        skin.pose_words[..len],
        previous,
        "restore prime survives a re-feed"
    );
    assert_ne!(skin.pose_words[..len], skin.pose_words[len..]);
}

#[test]
fn hidden_instances_keep_tick_exact_rows_and_same_tick_feed_replaces_current() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut sim = Sim::<Scene>::new(()).unwrap();
    let mut renderer =
        crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer
        .prepare_model("posed.model", &crate::test_model::posable_model())
        .unwrap();
    let e = sim.world().named("actor").unwrap();
    let mut feed = crate::Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    sim.run(50.);
    feed.feed(sim.world(), &mut renderer).unwrap();
    sim.world_mut().insert(e, Visible(false));
    sim.run(0.);
    feed.feed(sim.world(), &mut renderer).unwrap();
    for angle in [0.7, 0.9] {
        sim.world_mut().get_mut::<Gait>(e).unwrap().angle = angle;
        sim.run(17.);
        feed.feed(sim.world(), &mut renderer).unwrap();
    }
    sim.world_mut().remove::<Visible>(e);
    sim.run(0.);
    feed.feed(sim.world(), &mut renderer).unwrap();
    let skin = renderer.models.skinning.as_ref().unwrap();
    let before = skin.history.pair(e, false);
    assert!(before[0].rows()[0]
        .transform
        .rotation
        .abs_diff_eq(Quat::from_rotation_x(0.7), 1e-6));
    assert!(before[1].rows()[0]
        .transform
        .rotation
        .abs_diff_eq(Quat::from_rotation_x(0.9), 1e-6));
    sim.world_mut().get_mut::<Gait>(e).unwrap().angle = 1.1;
    sim.run(0.);
    feed.feed(sim.world(), &mut renderer).unwrap();
    let after = renderer
        .models
        .skinning
        .as_ref()
        .unwrap()
        .history
        .pair(e, false);
    assert_eq!(
        before[0].rows()[0].transform.rotation,
        after[0].rows()[0].transform.rotation
    );
    assert!(after[1].rows()[0]
        .transform
        .rotation
        .abs_diff_eq(Quat::from_rotation_x(1.1), 1e-6));
    // Clearing the last row at the same tick must invalidate the feed too.
    sim.world_mut().get_mut::<Gait>(e).unwrap().shown = false;
    sim.run(0.);
    feed.feed(sim.world(), &mut renderer).unwrap();
    let removed = renderer
        .models
        .skinning
        .as_ref()
        .unwrap()
        .history
        .pair(e, false);
    assert!(removed[1].rows().is_empty());
    assert_eq!(removed[0].rows()[0].transform, after[0].rows()[0].transform);
}

#[test]
fn overflowing_either_endpoint_draws_both_base_endpoints_in_spans_and_sockets() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let model = crate::test_model::posable_model();
    let mut sim = Sim::<Scene>::new(()).unwrap();
    sim.run(50.);
    let e = sim.world().named("actor").unwrap();
    sim.world_mut().remove::<Animation>(e);
    let mut base = animation::bind_pose(&model);
    crate::node_poses::put(&mut base[10..20], Transform::default());
    let mut pose = Pose::default();
    pose.previous = base.clone();
    pose.local = base.clone();
    sim.world_mut().insert(e, pose);
    let (mut skin, _) = setup_skin(&gpu, &model, e);
    skin.feed(&gpu.queue, sim.world(), &[e], true);
    let mut attachments = crate::world::scene::Attachments::default();
    attachments.feed(sim.world(), true, true, false, false);
    sim.run(17.);
    let mut current = base.clone();
    current[17] = 2.;
    let mut pose = Pose::default();
    pose.previous = base.clone();
    pose.local = current.clone();
    sim.world_mut().insert(e, pose);
    sim.world_mut().get_mut::<Gait>(e).unwrap().overflow = true;
    sim.run(0.);
    skin.feed(&gpu.queue, sim.world(), &[e], false);
    attachments.feed(sim.world(), false, true, false, false);
    let len = base.len();
    assert_eq!(&skin.pose_words[10..20], &base[10..20]);
    assert_eq!(&skin.pose_words[len + 10..len + 20], &current[10..20]);
    let charm = sim.world().named("charm").unwrap();
    for alpha in [0., 0.5, 1.] {
        attachments.frame(alpha);
        let mut pose = base.clone();
        pose[17] = 1. + alpha;
        close(
            attachments
                .output
                .iter()
                .find(|a| a.entity == charm)
                .unwrap()
                .matrix,
            animation::joint_matrix(&model, &pose, 2),
        );
    }
}

fn socket_matches_span(
    sim: &Sim<Scene>,
    feed: &mut crate::Feed,
    renderer: &crate::Renderer,
    model: &Model,
) {
    let skin = renderer.models.skinning.as_ref().unwrap();
    let len = model.nodes.len() * 10;
    for alpha in [0., 0.5, 1.] {
        let mut pose = vec![0.; len];
        for (i, node) in pose.chunks_exact_mut(10).enumerate() {
            let a = crate::node_poses::read(&skin.pose_words[i * 10..]);
            let b = crate::node_poses::read(&skin.pose_words[len + i * 10..]);
            crate::node_poses::put(
                node,
                Transform {
                    position: a.position.lerp(b.position, alpha),
                    rotation: a.rotation.slerp(b.rotation, alpha).normalize(),
                    scale: a.scale.lerp(b.scale, alpha),
                },
            );
        }
        let frame = feed.frame(sim.world(), alpha, 1.);
        for name in ["charm", "late"] {
            if let Some(charm) = sim.world().named(name) {
                close(
                    frame
                        .attachments
                        .iter()
                        .find(|a| a.entity == charm)
                        .unwrap()
                        .matrix,
                    animation::joint_matrix(model, &pose, 2),
                );
            }
        }
    }
}

#[test]
fn model_switches_prime_spans_and_sockets_even_without_asset_delivery() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let a = crate::test_model::posable_model();
    let mut b = a.clone();
    b.nodes[0].transform = Mat4::from_translation(Vec3::X).to_cols_array();
    let mut sim = Sim::<Scene>::new(()).unwrap();
    let e = sim.world().named("actor").unwrap();
    sim.world_mut().remove::<Animation>(e);
    sim.world_mut().remove::<Pose>(e);
    sim.world_mut()
        .generated_model("other.model", b.clone())
        .unwrap();
    let mut renderer =
        crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer.prepare_model("posed.model", &a).unwrap();
    renderer.prepare_model("other.model", &b).unwrap();
    let mut feed = crate::Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    for (name, model, angle) in [("other.model", &b, 0.8), ("posed.model", &a, 1.2)] {
        sim.run(17.);
        sim.world_mut().insert(e, Mesh::asset(name));
        sim.world_mut().get_mut::<Gait>(e).unwrap().angle = angle;
        sim.run(0.);
        feed.feed(sim.world(), &mut renderer).unwrap();
        let skin = renderer.models.skinning.as_ref().unwrap();
        let len = model.nodes.len() * 10;
        assert_eq!(skin.pose_words[..len], skin.pose_words[len..]);
        socket_matches_span(&sim, &mut feed, &renderer, model);
        let previous = skin.pose_words[..len].to_vec();
        sim.world_mut().get_mut::<Gait>(e).unwrap().angle += 0.1;
        sim.run(0.);
        feed.feed(sim.world(), &mut renderer).unwrap();
        let skin = renderer.models.skinning.as_ref().unwrap();
        assert_eq!(skin.pose_words[..len], previous);
        assert_ne!(skin.pose_words[..len], skin.pose_words[len..]);
        if sim.world().named("late").is_none() {
            sim.world_mut()
                .spawn_named("late", (Transform::default(), SocketFollow::new(e, "hand")));
            sim.run(0.);
            feed.feed(sim.world(), &mut renderer).unwrap();
        }
        socket_matches_span(&sim, &mut feed, &renderer, model);
    }
}

#[test]
fn retiring_models_reclaims_names_diagnostics_and_priming_but_keeps_hidden_live_instances() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut model = crate::test_model::posable_model();
    let mut sim = Sim::<Scene>::new(()).unwrap();
    sim.world_mut()
        .generated_model("live.model", model.clone())
        .unwrap();
    let hidden = sim.world_mut().spawn((
        Transform::default(),
        Mesh::asset("live.model"),
        Gait {
            angle: 0.3,
            shown: true,
            overflow: false,
        },
    ));
    sim.run(50.);
    let mut renderer =
        crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer.prepare_model("posed.model", &model).unwrap();
    renderer.prepare_model("live.model", &model).unwrap();
    let mut feed = crate::Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    let live_model = renderer
        .models
        .skinning
        .as_ref()
        .unwrap()
        .primed
        .iter()
        .find(|p| p.0 == hidden)
        .unwrap()
        .1;
    sim.world_mut().insert(hidden, Visible(false));
    sim.run(17.);
    feed.feed(sim.world(), &mut renderer).unwrap();
    for i in 1..=12 {
        model.nodes[0].transform = Mat4::from_translation(Vec3::X * i as f32).to_cols_array();
        renderer.prepare_model("posed.model", &model).unwrap();
        let skin = renderer.models.skinning.as_ref().unwrap();
        assert_eq!(skin.models.iter().flatten().count(), 2);
        assert!(skin.models.len() <= 3, "vacant model slots are reused");
        assert_eq!(skin.primed.len(), 1, "retired model instances are removed");
        assert_eq!((skin.primed[0].0, skin.primed[0].1), (hidden, live_model));
        let live_model = skin.models[live_model].as_ref().unwrap();
        assert!(!live_model.nodes.is_empty(), "live names remain resident");
        sim.world_mut().get_mut::<Gait>(hidden).unwrap().angle = i as f32 * 0.1;
        sim.run(17.);
        feed.feed(sim.world(), &mut renderer).unwrap();
        let skin = renderer.models.skinning.as_ref().unwrap();
        assert_eq!(skin.primed.len(), 2);
        for (_, id, _) in &skin.primed {
            assert!(skin.models[*id].is_some());
        }
    }
    let before = renderer
        .models
        .skinning
        .as_ref()
        .unwrap()
        .history
        .pair(hidden, false);
    sim.world_mut().remove::<Visible>(hidden);
    sim.run(0.);
    feed.feed(sim.world(), &mut renderer).unwrap();
    let skin = renderer.models.skinning.as_ref().unwrap();
    let after = skin.history.pair(hidden, false);
    for endpoint in 0..2 {
        assert_eq!(
            before[endpoint].rows()[0].transform,
            after[endpoint].rows()[0].transform
        );
    }
    assert!(skin
        .primed
        .iter()
        .any(|p| (p.0, p.1) == (hidden, live_model)));
}
