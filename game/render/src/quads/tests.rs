use super::*;
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn r14_native_children_reserve_order_before_frame() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut renderer =
        crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let q = &mut renderer.quads;
    let plane = crate::placed::Plane {
        placement: exact_gpu::Placement {
            hidden: false,
            homography: [0.; 9],
            depth: 0.,
            clip_depth: [[0.; 3]; 2],
        },
        center: Vec3::ZERO,
        x: Vec3::X,
        y: Vec3::Y,
    };
    q.children = (0..5000).map(|i| (i, plane)).collect();
    q.prepare(&gpu.device, &gpu.queue);
    let capacity = q.order.capacity();
    q.frame::<false>(&FrameInput::default(), &BTreeMap::new(), true);
    assert_eq!(q.order.len(), 5000);
    assert_eq!(q.order.capacity(), capacity);
}
#[test]
fn emitter_reach_contains_every_derived_particle_quad() {
    let mut w = World::new(60, 3);
    let shapes = [
        exact_game::emitter::Shape::Point,
        exact_game::emitter::Shape::Sphere(1.5),
        exact_game::emitter::Shape::Cone(0.7, 2.),
    ];
    let mut ids = Vec::new();
    for (i, shape) in shapes.into_iter().enumerate() {
        for drag in [0., 0.8] {
            ids.push(w.spawn(Emitter {
                shape,
                drag,
                speed: 4. + i as f32,
                spread: 2.5,
                gravity: Vec3::new(1., -9.81, 0.5),
                size: [0.3, 0.9],
                ..Emitter::sparks().rate(200.).lifetime(1.5).seed(i as u64)
            }));
        }
    }
    for _ in 0..120 {
        exact_game::emitter::step(&w);
    }
    let t = glam::Mat4::from_scale_rotation_translation(
        Vec3::new(2., 0.5, 1.),
        glam::Quat::from_rotation_z(0.7),
        Vec3::new(3., 1., -2.),
    );
    let (right, up) = (Vec3::X, Vec3::Y);
    for id in ids {
        let e = w.get::<Emitter>(id).unwrap();
        let reach = emitter_reach(&e, &t);
        let mut seen = 0;
        for alpha in [0., 0.5, 1.] {
            e.particles(60, alpha, |p| {
                seen += 1;
                let centre = t.transform_point3(p.position);
                let x = right * p.size * t.x_axis.truncate().length();
                let y = up * p.size * t.y_axis.truncate().length();
                for corner in [x + y, x - y, -x + y, -x - y] {
                    let d = (centre + corner * 0.5).distance(t.w_axis.truncate());
                    assert!(d <= reach * 1.0001, "{d} beyond reach {reach}");
                }
            });
        }
        assert!(seen > 100);
    }
}
#[test]
fn every_kind_and_owner_ordinal_has_the_same_total_order() {
    let mut kinds = vec![
        Kind::Particle(false, NO_LOOK, false, false),
        Kind::Primitive(false),
        Kind::Model(0, 0),
        Kind::Sprite(0),
    ];
    #[cfg(not(target_arch = "wasm32"))]
    kinds.push(Kind::Child(0));
    let mut rows = Vec::new();
    for kind in kinds {
        for index in [2, 0, 1] {
            rows.push(Order {
                kind,
                depth: 3.,
                viewmodel: false,
                layer: 1,
                slot: 7,
                index,
            });
        }
    }
    rows.reverse();
    rows.sort_unstable_by(Order::compare);
    let actual: Vec<_> = rows.iter().map(|o| (o.kind.rank(), o.index)).collect();
    let mut expected = vec![
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 1),
        (1, 2),
        (2, 0),
        (2, 1),
        (2, 2),
        (3, 0),
        (3, 1),
        (3, 2),
    ];
    #[cfg(not(target_arch = "wasm32"))]
    expected.extend([(4, 0), (4, 1), (4, 2)]);
    assert_eq!(actual, expected);
    // Rank is independently pinned; a mistaken enum rank must fail too.
    assert_eq!(Kind::Model(0, 0).rank(), 0);
    assert_eq!(Kind::Sprite(0).rank(), 1);
    assert_eq!(Kind::Particle(false, NO_LOOK, false, false).rank(), 2);
    assert_eq!(Kind::Primitive(false).rank(), 3);
    #[cfg(not(target_arch = "wasm32"))]
    assert_eq!(Kind::Child(0).rank(), 4);
}
#[test]
fn revisions_reuse_owned_values_and_membership_compacts_in_order() {
    let mut w = World::new(60, 0);
    let first = w.spawn((Transform::default(), Emitter::default()));
    let entity = w.spawn((
        Transform::default(),
        Emitter {
            state: exact_game::emitter::EmitterState {
                births: vec![Default::default()],
                ..Default::default()
            },
            ..Default::default()
        },
        Sprite::new("shared.tex", [1., 1.]),
    ));
    w.propagate();
    let mut emitters = Vec::new();
    let mut sprites = Vec::new();
    feed::<Emitter>(&w, &mut emitters, true, false, false);
    feed::<Sprite>(&w, &mut sprites, true, false, false);
    let births = emitters[1].value.state.births.as_ptr();
    let texture = sprites[0].value.texture.as_ptr();
    w.get_mut::<Emitter>(entity).unwrap().rate = 90.;
    w.get_mut::<Sprite>(entity).unwrap().frame = [16, 0, 16, 16];
    feed::<Emitter>(&w, &mut emitters, false, false, false);
    feed::<Sprite>(&w, &mut sprites, false, false, false);
    assert_eq!(emitters[1].value.state.births.as_ptr(), births);
    assert_eq!(sprites[0].value.texture.as_ptr(), texture);
    assert_eq!(emitters[1].value.rate, 90.);
    assert_eq!(sprites[0].value.frame, [16, 0, 16, 16]);
    // Sentinel proves the unchanged-revision branch skipped the copy.
    emitters[1].value.rate = 91.;
    sprites[0].value.frame = [32, 0, 16, 16];
    feed::<Emitter>(&w, &mut emitters, false, false, false);
    feed::<Sprite>(&w, &mut sprites, false, false, false);
    assert_eq!(emitters[1].value.state.births.as_ptr(), births);
    assert_eq!(emitters[1].value.rate, 91.);
    assert_eq!(sprites[0].value.frame, [32, 0, 16, 16]);
    w.despawn(first);
    feed::<Emitter>(&w, &mut emitters, false, false, false);
    assert_eq!(emitters.len(), 1);
    assert_eq!(emitters[0].entity, entity);
    assert_eq!(emitters[0].value.state.births.as_ptr(), births);
}
