use super::*;
use exact_game::Facing;

#[test]
fn placed_status_depth_preserves_float_bits_and_other_fields() {
    #[derive(Default, exact_game::Data)]
    struct Reply {
        world: Info,
    }
    #[derive(Default, exact_game::Data)]
    struct Info {
        placed: Status,
    }
    #[derive(Default, exact_game::Data)]
    struct Status {
        child: u16,
        hidden: bool,
        depth: f32,
    }
    let mut world = World::new(60, 0);
    world.spawn_named("label", Placed::child(0));
    let mut placements = Placements::default();
    placements.child(0, "", None, [0., 0., 100., 50.]);
    let mut plane = project(
        Placed::child(0),
        Transform::at(0., 0., -2.),
        [0., 0., 100., 50.],
        Mat4::IDENTITY,
        Camera::default().matrix(Vec2::splat(200.)),
        Vec2::splat(200.),
    );
    for depth in [0., -0., 12.345, -1e-5, f32::from_bits(1), f32::MAX] {
        plane.placement.depth = depth;
        placements.children[0].plane = Some(plane);
        let mut reply = r#"{"world":{"name":"label"}}"#.to_owned();
        placements.status(&world, r#"{"op":"state","entity":"label"}"#, &mut reply);
        let parsed: Reply = exact_game::json::from_str(&reply).unwrap();
        assert_eq!(parsed.world.placed.child, 0);
        assert_eq!(parsed.world.placed.hidden, plane.placement.hidden);
        assert_eq!(parsed.world.placed.depth.to_bits(), depth.to_bits());
    }
}

#[test]
fn hand_computed_square_and_projective_fourth_corner() {
    let camera = Camera {
        fov_y_degrees: 90.,
        ..Default::default()
    };
    let size = Vec2::splat(200.);
    let plane = project(
        Placed {
            anchor: [0.5, 0.5],
            ..Placed::child(1).width(2.)
        },
        Transform::at(0., 0., -2.),
        [10., 20., 100., 100.],
        Mat4::IDENTITY,
        camera.matrix(size),
        size,
    );
    let h = plane.placement.homography;
    let map = |x, y| {
        Vec2::new(h[0] * x + h[1] * y + h[2], h[3] * x + h[4] * y + h[5])
            / (h[6] * x + h[7] * y + h[8])
    };
    for (x, y, want) in [
        (0., 0., Vec2::splat(50.)),
        (100., 0., Vec2::new(150., 50.)),
        (100., 100., Vec2::splat(150.)),
        (0., 100., Vec2::new(50., 150.)),
    ] {
        assert!((map(x, y) - want).length() < 0.0001);
    }
    assert!(!plane.placement.hidden);
    assert_eq!(plane.placement.depth, -2.);
}
#[test]
fn hidden_is_explicit_for_near_offscreen_and_fixed_back() {
    let c = Camera::default();
    let size = Vec2::splat(200.);
    let project_at = |value, t| {
        project(
            value,
            t,
            [0., 0., 100., 100.],
            Mat4::IDENTITY,
            c.matrix(size),
            size,
        )
        .placement
    };
    assert!(project_at(Placed::child(1), Transform::at(0., 0., 0.)).hidden);
    assert!(project_at(Placed::child(1), Transform::at(100., 0., -2.)).hidden);
    // A plane surrounding the viewport remains visible.
    assert!(!project_at(Placed::child(1).width(200.), Transform::at(0., -50., -2.)).hidden);
    let mut t = Transform::at(0., 0., -2.);
    t.rotation = exact_game::Quat::from_rotation_y(std::f32::consts::PI);
    assert!(project_at(Placed::child(1).facing(Facing::Fixed), t).hidden);
    assert!(!project_at(Placed::child(1), t).hidden);
}
#[test]
fn invisible_owner_stays_hidden_and_duplicate_child_is_refused() {
    let mut w = World::new(60, 7);
    w.spawn((
        Transform::at(0., 0., -2.),
        Placed::child(0),
        exact_game::Visible(false),
    ));
    let mut p = Placements::default();
    p.child(0, "", None, [0., 0., 100., 50.]);
    p.feed(&w).unwrap();
    p.frame(&FrameInput::default(), Vec2::splat(200.));
    assert!(p.placement(0).unwrap().hidden);
    w.spawn((Transform::default(), Placed::child(0)));
    assert!(p
        .feed(&w)
        .unwrap_err()
        .to_string()
        .contains("multiple owners"));
}
#[test]
fn saved_component_does_not_include_the_derived_outcome() {
    let mut w = World::new(60, 7);
    w.spawn_named(
        "sign",
        (Transform::at(0., 0., -2.), Placed::child(1).width(1.2)),
    );
    let hash = w.hash();
    let saved = w.save();
    let mut p = Placements::default();
    p.child(0, "", None, [0., 0., 100., 20.]);
    p.child(1, "", None, [0., 20., 100., 20.]);
    p.feed(&w).unwrap();
    p.frame(&FrameInput::default(), Vec2::splat(200.));
    assert_eq!(w.hash(), hash);
    assert_eq!(w.save(), saved);
    assert!(p.placement(0).is_none());
    assert!(p.placement(1).is_some());
    w.load(&saved).unwrap();
    assert_eq!(
        *w.get::<Placed>("sign").unwrap(),
        Placed::child(1).width(1.2)
    );
}

#[test]
fn displayed_plane_uses_interpolated_entity_and_camera() {
    let mut w = World::new(60, 7);
    let e = w.spawn((Transform::at(0., 0., -4.), Placed::child(0)));
    let mut p = Placements::default();
    p.child(0, "", None, [0., 0., 100., 50.]);
    p.feed(&w).unwrap();
    // Ordinary writes update the retained current pose without priming history.
    w.get_mut::<Transform>(e).unwrap().position.x = 2.;
    p.feed(&w).unwrap();
    p.items[0].poses[0] = Transform::at(0., 0., -4.);
    let input = FrameInput {
        alpha: 0.5,
        view: Mat4::from_translation(Vec3::new(-0.5, 0., 0.)),
        ..Default::default()
    };
    p.frame(&input, Vec2::new(200., 100.));
    let plane = p.children[0].plane.unwrap();
    assert!((plane.center.x - 1.).abs() < 1e-5);
    let h = plane.placement.homography;
    let anchor = Vec2::new(
        (h[0] * 50. + h[1] * 50. + h[2]) / (h[6] * 50. + h[7] * 50. + h[8]),
        (h[3] * 50. + h[4] * 50. + h[5]) / (h[6] * 50. + h[7] * 50. + h[8]),
    );
    let clip = input.proj * input.view * Vec3::new(1., 0., -4.).extend(1.);
    let expected = Vec2::new((clip.x / clip.w + 1.) * 100., (1. - clip.y / clip.w) * 50.);
    assert!((anchor - expected).length() < 1e-4);
}

#[test]
#[ignore = "projection diagnostic; run release with --nocapture"]
fn homography_cost() {
    let size = Vec2::new(1280., 720.);
    let proj = Camera::default().matrix(size);
    let start = std::time::Instant::now();
    for i in 0..400_000 {
        std::hint::black_box(project(
            Placed::child(1),
            Transform::at((i % 40) as f32 * 0.02, 1., -4.),
            [0., 0., 140., 40.],
            Mat4::IDENTITY,
            proj,
            size,
        ));
    }
    println!(
        "U1 projection: {:.3} ns/child/frame (400000 samples)",
        start.elapsed().as_secs_f64() * 1e9 / 400_000.
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn captured_children_share_draw_and_hit_depth_and_a_wall_occludes_them() {
    use exact_gpu::{fixture, Frame, Surface};
    struct Signs;
    impl exact_game::Game for Signs {
        const ID: &'static str = "placed-pixels";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.insert_resource(exact_game::Environment {
                background: Some([0.; 3]),
                fog: None,
                bloom: None,
                ..Default::default()
            });
            w.spawn((Transform::at(0., 0., 5.), Camera::orthographic(4.)));
            // Deliberately spawn the farther Contract child first.
            w.spawn_named(
                "far",
                (Transform::at(0., -1., -1.), Placed::child(1).width(2.)),
            );
            w.spawn_named(
                "near",
                (Transform::at(0., -1., 0.), Placed::child(0).width(2.)),
            );
            w.spawn_named(
                "wall",
                (
                    Transform::at(4., 0., 1.),
                    exact_game::Mesh::cube(2.),
                    exact_game::Material::rgb(0., 0., 1.).emissive(0., 0., 4.),
                ),
            );
        }
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let texture = |rgba: [u8; 4]| {
        let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            t.as_image_copy(),
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            t.size(),
        );
        t.create_view(&Default::default())
    };
    let mut s = crate::WorldSurface::<Signs>::default();
    s.bind(&[], None).unwrap();
    // Premultiplied half-alpha green over opaque red.
    s.child(
        0,
        "",
        Some(&texture([0, 128, 0, 128])),
        [0., 0., 100., 100.],
    );
    s.child(
        1,
        "",
        Some(&texture([255, 0, 0, 255])),
        [0., 100., 100., 100.],
    );
    let frame = Frame {
        width: 100.,
        height: 100.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    let pixels = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    let [r, g, b, _] = pixels.at(50, 50);
    assert!(
        r > 100 && g > 100 && b < 5 && (i16::from(r) - i16::from(g)).abs() <= 2,
        "premultiplied near green over far red: {r},{g},{b}"
    );
    assert!(
        s.placement(0).unwrap().depth > s.placement(1).unwrap().depth,
        "host hits the last child drawn"
    );
    // Equal-depth ties use Contract child order, independent of entity order.
    s.sim()
        .unwrap()
        .world()
        .get_mut::<Transform>("far")
        .unwrap()
        .position
        .z = 0.;
    s.child(
        1,
        "",
        Some(&texture([255, 0, 0, 255])),
        [0., 100., 100., 100.],
    );
    let pixels = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    let [r, g, _, _] = pixels.at(50, 50);
    assert!(r > 200 && g < 5, "later child is on top at equal depth");
    s.sim()
        .unwrap()
        .world()
        .get_mut::<Transform>("wall")
        .unwrap()
        .position
        .x = 0.;
    s.child(
        1,
        "",
        Some(&texture([255, 0, 0, 255])),
        [0., 100., 100., 100.],
    );
    let pixels = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    let [r, g, b, _] = pixels.at(50, 50);
    // The blue dielectric also reflects a few percent of the sky; red text would be > 200.
    assert!(
        b > 200 && r < 16 && g < 16,
        "opaque wall depth-tests captured text {r},{g},{b}"
    );
    // Place the captured planes behind the existing camera's near plane.
    s.sim()
        .unwrap()
        .world()
        .get_mut::<Transform>("near")
        .unwrap()
        .position
        .z = 6.;
    s.child(0, "", None, [0.; 4]);
    fixture::render(&gpu, &mut s, &frame).unwrap();
    assert!(s.placement(0).unwrap().hidden);
}

#[test]
fn css_hides_depth_crossings_while_native_clips_the_visible_nameplate() {
    let size = Vec2::splat(200.);
    let camera = Camera {
        near: 0.1,
        far: 10.,
        ..Default::default()
    };
    for (z, width) in [(-0.15, 0.2), (-0.15, 2.), (-11., 1.)] {
        let pose = Transform {
            rotation: exact_game::Quat::from_rotation_y(0.8),
            ..Transform::at(0., 0., z)
        };
        let value = Placed {
            anchor: [0.5, 0.5],
            ..Placed::child(0).width(width).facing(Facing::Fixed)
        };
        let css = value.project(
            pose,
            Vec2::splat(100.),
            Mat4::IDENTITY,
            camera.matrix(size),
            size,
        );
        assert!(
            css.hidden,
            "CSS cannot cross near or eye: z={z}, width={width}"
        );
        let native = project(
            value,
            pose,
            [0., 0., 100., 100.],
            Mat4::IDENTITY,
            camera.matrix(size),
            size,
        )
        .placement;
        assert_eq!(
            native.hidden,
            z < -10.,
            "walk-up nameplate still intersects native clip volume"
        );
        if !native.hidden {
            let near = native.clip_depth[0];
            let sides = [near[2], near[0] * 100. + near[2]];
            assert!(sides.iter().any(|d| *d < 0.) && sides.iter().any(|d| *d > 0.));
            assert!(native.homography.iter().all(|v| v.is_finite()));
        }
    }
}

#[test]
fn headless_placement_uses_the_displayed_camera_and_plane_sample() {
    let mut w = World::new(60, 0);
    let camera = w.spawn((Transform::at(0., 0., 8.), Camera::default()));
    let owner = w.spawn((Transform::default(), Placed::child(0)));
    w.propagate();
    let mut p = Placements::default();
    p.child(0, "", None, [0., 0., 100., 50.]);
    p.feed(&w).unwrap();
    w.get_mut::<Transform>(camera).unwrap().position.x = 1.;
    w.get_mut::<Transform>(owner).unwrap().position.x = 2.;
    w.propagate();
    p.feed(&w).unwrap();
    // A fractional displayed sample differs from the committed world pose.
    p.items[0].poses[0] = Transform::default();
    let size = Vec2::new(1280., 720.);
    p.cameras[0].poses[0] = Transform::at(0., 0., 8.);
    p.headless(size, 0.5);
    let expected = project(
        Placed::child(0),
        Transform::at(1., 0., 0.),
        [0., 0., 100., 50.],
        Mat4::from_translation(Vec3::new(-0.5, 0., -8.)),
        Camera::default().matrix(size),
        size,
    );
    assert_eq!(
        p.placement(0).unwrap().homography,
        expected.placement.homography
    );
}

#[test]
fn disjoint_corner_parallelogram_is_not_in_the_viewport() {
    let pose = Transform {
        position: Vec3::new(1.75, 1.75, -0.5),
        rotation: exact_game::Quat::from_rotation_z(3. * std::f32::consts::FRAC_PI_4),
        scale: Vec3::new(4.5f32.sqrt(), 2f32.sqrt(), 1.),
    };
    let p = Placed {
        anchor: [0.5, 0.5],
        ..Placed::child(0).facing(Facing::Fixed)
    }
    .project(
        pose,
        Vec2::splat(100.),
        Mat4::IDENTITY,
        Mat4::from_scale(Vec3::new(1., 1., -1.)),
        Vec2::splat(200.),
    );
    assert!(
        p.hidden,
        "quad outside the top-right corner has no intersection"
    );
}

#[test]
fn linux_placed_socket_redelivery_snaps_the_new_rig_history() {
    use exact_game::{
        asset::{Content, Model, Node},
        Mesh, Pose, SocketFollow,
    };
    struct Rig;
    impl exact_game::Game for Rig {
        const ID: &'static str = "placed-redelivery";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    let mut model = Model {
        nodes: vec![Node {
            name: "head".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
    sim.deliver_asset("rig.model", Ok(Content::Model(model.clone())))
        .unwrap();
    let mut pose = Pose::default();
    pose.previous = exact_game::animation::bind_pose(&model);
    pose.local = pose.previous.clone();
    pose.local[0] = 4.;
    sim.world_mut().spawn_named(
        "rig",
        (Transform::default(), Mesh::asset("rig.model"), pose),
    );
    sim.world_mut().spawn((
        Transform::default(),
        SocketFollow::new("rig", "head"),
        Placed::child(0),
    ));
    sim.world_mut()
        .spawn((Transform::at(0., 0., 10.), Camera::default()));
    let saved = sim.world().save();
    sim.world_mut().load(&saved).unwrap();
    let mut p = Placements::default();
    p.child(0, "", None, [0., 0., 100., 50.]);
    p.feed(sim.world()).unwrap();
    p.feed(sim.world()).unwrap();
    p.headless(Vec2::splat(200.), 0.5);
    assert!((p.children[0].plane.unwrap().center.x - 2.).abs() < 1e-5);
    model.nodes[0].transform[13] = 1.;
    sim.deliver_asset("rig.model", Ok(Content::Model(model)))
        .unwrap();
    p.feed(sim.world()).unwrap();
    p.headless(Vec2::splat(200.), 0.5);
    assert!(
        (p.children[0].plane.unwrap().center.x - 4.).abs() < 1e-5,
        "new rig must snap instead of interpolating old previous pose"
    );
}

#[test]
fn displayed_unresolved_follower_logs_once_and_falls_back() {
    let mut w = World::new(60, 0);
    w.spawn_named(
        "lost-charm",
        (
            Transform::at(1., 2., 3.),
            exact_game::SocketFollow::new("missing-head", "head"),
        ),
    );
    let mut attachments = scene::Attachments::default();
    let before = w.journal();
    for _ in 0..3 {
        attachments.feed(&w, false, false, false, false);
    }
    assert_eq!(
        w.journal().len(),
        before.len(),
        "presentation diagnostics must not enter saves"
    );
    let errors: Vec<_> = attachments.diagnostics.borrow().values().cloned().collect();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("lost-charm") && errors[0].contains("missing-head"));
    attachments.frame(0.5);
    assert!(attachments.output.is_empty());
}

#[test]
fn displayed_unavailable_follower_logs_once_by_name() {
    struct Skipped;
    impl exact_game::Game for Skipped {
        const ID: &'static str = "skipped-animation";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    let mut sim = exact_game::Sim::<Skipped>::new(()).unwrap();
    sim.run(17.);
    let w = sim.world_mut();
    w.spawn_named(
        "head",
        (
            Transform::default(),
            exact_game::Mesh::asset("rig.model"),
            exact_game::Animation::play("walk"),
        ),
    );
    w.spawn_named(
        "charm",
        (
            Transform::default(),
            exact_game::SocketFollow::new("head", "head"),
        ),
    );
    let mut a = scene::Attachments::default();
    for _ in 0..3 {
        a.feed(w, false, false, false, false);
    }
    let errors: Vec<_> = a.diagnostics.borrow().values().cloned().collect();
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].contains("charm")
            && errors[0].contains("rig.model")
            && errors[0].contains("not loaded")
    );
}

#[test]
fn attachment_diagnostics_are_shared_and_history_reset_keeps_them() {
    let mut w = World::new(60, 0);
    let e = w.spawn_named(
        "lost",
        (
            Transform::default(),
            exact_game::SocketFollow::new("missing", "head"),
        ),
    );
    let mut a = scene::Attachments::default();
    let mut b = scene::Attachments::default();
    b.diagnostics = a.diagnostics.clone();
    a.feed(&w, true, false, false, false);
    assert!(
        b.diagnostics.borrow().contains_key(&(e, "socket")),
        "second feed must see the first warning"
    );
    a.reset();
    assert!(a.diagnostics.borrow().contains_key(&(e, "socket")));
    b.feed(&w, true, false, false, false);
    assert_eq!(a.diagnostics.borrow().len(), 1);
    w.despawn(e);
    b.feed(&w, false, false, false, false);
    assert!(
        a.diagnostics.borrow().is_empty(),
        "dead entity warnings are pruned for both feeds"
    );
}

#[test]
fn diagnostics_replace_changed_error_clear_on_fresh_world_and_survive_restore() {
    let mut w = World::new(60, 0);
    let e = w.spawn_named(
        "charm",
        (
            Transform::default(),
            exact_game::SocketFollow::new("missing-a", "head"),
        ),
    );
    let mut a = scene::Attachments::default();
    a.feed(&w, true, false, false, false);
    let first = a.diagnostics.borrow()[&(e, "socket")].clone();
    w.get_mut::<exact_game::SocketFollow>(e).unwrap().target = "missing-b".into();
    a.feed(&w, false, true, false, false);
    assert_ne!(a.diagnostics.borrow()[&(e, "socket")], first);
    let saved = w.save();
    w.load(&saved).unwrap();
    a.diagnostics.borrow_mut().restored(&w);
    a.reset();
    a.feed(&w, true, false, false, false);
    assert_eq!(a.diagnostics.borrow().len(), 1);
    let mut fresh = World::new(60, 0);
    fresh.spawn(Transform::default());
    a.feed(&fresh, true, false, false, false);
    assert!(a.diagnostics.borrow().is_empty());
}

#[test]
fn skipped_animation_display_uses_saved_joint_with_current_owner_and_offset() {
    use exact_game::{
        asset::{Content, Model, Node},
        Animation, Mesh, Pose, SocketFollow,
    };
    struct Rig;
    impl exact_game::Game for Rig {
        const ID: &'static str = "stale-display";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    let model = Model {
        nodes: vec![Node::default()],
        ..Default::default()
    };
    let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
    sim.deliver_asset("rig.model", Ok(Content::Model(model.clone())))
        .unwrap();
    let mut pose = Pose::default();
    pose.previous = exact_game::animation::bind_pose(&model);
    pose.local = pose.previous.clone();
    pose.local[0] = 2.;
    sim.world_mut().spawn_named(
        "owner",
        (
            Transform::default(),
            Mesh::asset("rig.model"),
            Animation::play("idle"),
            pose,
        ),
    );
    sim.world_mut().spawn_named(
        "charm",
        (Transform::default(), SocketFollow::new("owner", "")),
    );
    sim.run(17.);
    let mut a = scene::Attachments::default();
    a.feed(sim.world(), true, false, false, false);
    a.frame(1.);
    sim.world_mut()
        .get_mut::<Transform>("owner")
        .unwrap()
        .position
        .x = 10.;
    sim.world_mut()
        .get_mut::<SocketFollow>("charm")
        .unwrap()
        .offset
        .position
        .x = 5.;
    a.feed(sim.world(), false, true, false, false);
    a.frame(1.);
    assert_eq!(a.output[0].matrix.w_axis.x, 17.);
    // Skipped ticks retain the local endpoint rather than replaying old bone interpolation.
    a.frame(0.5);
    assert_eq!(a.output[0].matrix.w_axis.x, 12.);
    let save = sim.world().save();
    sim.world_mut().load(&save).unwrap();
    let mut fresh = scene::Attachments::default();
    fresh.feed(sim.world(), true, false, false, false);
    fresh.frame(1.);
    a.frame(1.);
    assert_eq!(a.output[0].matrix, fresh.output[0].matrix);
}

#[test]
fn named_children_reorder_disappear_return_and_restore_without_changing_the_save() {
    let mut w = World::new(60, 7);
    w.spawn_named(
        "owner",
        (Transform::at(0., 0., -2.), Placed::child("標識 🏮")),
    );
    w.spawn((Transform::default(), Camera::default()));
    let saved = w.save();
    let mut p = Placements::default();
    p.child(0, "hud", None, [0., 0., 100., 50.]);
    p.child(1, "標識 🏮", None, [0., 50., 100., 50.]);
    p.feed(&w).unwrap();
    p.headless(Vec2::splat(200.), 0.);
    let before = p.placement(1).unwrap();
    assert!(!before.hidden);
    assert!(p.placement(0).is_none());
    let revision = w.revision::<Placed>();
    // Change only host metadata. No component write may be required to rebind.
    p.child(0, "標識 🏮", None, [0., 0., 100., 50.]);
    p.child(1, "hud", None, [0., 50., 100., 50.]);
    p.feed(&w).unwrap();
    p.headless(Vec2::splat(200.), 0.);
    assert_eq!(p.placement(0).unwrap().homography, before.homography);
    assert!(p.placement(1).is_none());
    assert_eq!(w.revision::<Placed>(), revision);
    assert_eq!(w.save(), saved);
    let mut reply = r#"{"world":{}}"#.to_owned();
    p.status(&w, r#"{"op":"state","entity":"owner"}"#, &mut reply);
    assert!(reply.contains(r#""child":0,"hidden":false"#), "{reply}");
    // A collapsed named child retains its identity, including at the tail.
    p.child(0, "標識 🏮", None, [0.; 4]);
    p.feed(&w).unwrap();
    p.headless(Vec2::splat(200.), 0.);
    assert!(p.placement(0).unwrap().hidden);
    p.child(0, "renamed", None, [0., 0., 100., 50.]);
    p.feed(&w).unwrap();
    p.headless(Vec2::splat(200.), 0.);
    assert!(p.children.iter().all(|c| c.plane.is_none()));
    let mut reply = r#"{"world":{}}"#.to_owned();
    p.status(&w, r#"{"op":"state","entity":"owner"}"#, &mut reply);
    assert!(reply.contains(r#""child":null,"hidden":true"#), "{reply}");
    p.child(1, "", None, [0.; 4]);
    p.child(0, "", None, [0.; 4]);
    assert!(p.children.is_empty());
    p.child(0, "標識 🏮", None, [0.; 4]);
    assert_eq!(p.children.len(), 1);
    p.child(0, "標識 🏮", None, [0., 0., 100., 50.]);
    w.load(&saved).unwrap();
    let mut fresh = Placements::default();
    fresh.child(0, "", None, [0.; 4]); // anonymous collapsed HUD before the sign
    fresh.child(1, "標識 🏮", None, [0., 0., 100., 50.]);
    fresh.feed(&w).unwrap();
    fresh.headless(Vec2::splat(200.), 0.);
    assert_eq!(fresh.placement(1).unwrap().homography, before.homography);
    assert_eq!(w.save(), saved);
    assert_eq!(
        w.require::<Placed>("owner").child,
        CanvasChild::from("標識 🏮")
    );
}

#[test]
fn named_children_refuse_ambiguity_and_numeric_aliases() {
    let mut w = World::new(60, 7);
    let owner = w.spawn((Transform::default(), Placed::child("sign")));
    let mut p = Placements::default();
    p.child(0, "sign", None, [0., 0., 100., 50.]);
    p.child(1, "sign", None, [0., 50., 100., 50.]);
    assert!(p
        .feed(&w)
        .unwrap_err()
        .to_string()
        .contains("duplicate testId"));
    p.child(1, "other", None, [0., 50., 100., 50.]);
    p.feed(&w).unwrap();
    let alias = w.spawn((Transform::default(), Placed::child(0)));
    assert!(p
        .feed(&w)
        .unwrap_err()
        .to_string()
        .contains("multiple owners"));
    w.despawn(alias);
    w.insert(owner, exact_game::Visible(false));
    p.feed(&w).unwrap();
    p.frame(&FrameInput::default(), Vec2::splat(200.));
    assert!(p.placement(0).unwrap().hidden);
    assert!(Placed::child("").validate().unwrap_err().contains("empty"));
}
