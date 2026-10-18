use exact_game::*;
struct Shapes;
impl Game for Shapes {
    const ID: &'static str = "primitive-pick";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn((
            Transform::at(0., 10., 0.).looking_at(Vec3::ZERO, Vec3::Z),
            Camera::orthographic(12.),
        ));
        w.spawn_named("ring", (Transform::at(-3., 0., 0.), Mesh::ring(0.5, 1.)));
        w.spawn_named("torus", (Transform::default(), Mesh::torus(0.8, 0.2)));
        w.spawn_named("cone", (Transform::at(3., 0., 0.), Mesh::cone(1., 2.)));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn new_shapes_validate_save_and_pick_the_surface_instead_of_the_hole() {
    for mesh in [
        Mesh::ring(0.5, 1.),
        Mesh::ring(0., 1.),
        Mesh::torus(1., 0.2),
        Mesh::cone(1., 2.),
    ] {
        mesh.validate().unwrap();
        let decoded: Mesh = bin::from_slice(&bin::to_vec(&mesh)).unwrap();
        assert_eq!(mesh, decoded);
    }
    for mesh in [
        Mesh::ring(-1., 1.),
        Mesh::ring(1., 1.),
        Mesh::torus(1., 2.),
        Mesh::cone(0., 1.),
    ] {
        assert!(mesh.validate().is_err());
    }
    let mut sim = Sim::<Shapes>::new(()).unwrap();
    sim.viewport(1200., 1200.);
    for name in ["ring", "torus"] {
        let center = sim.layout(name).unwrap().screen.center();
        assert!(sim.pick(center).is_none(), "{name}'s central hole is empty");
        let hit = sim.pick(center + Vec2::new(80., 0.)).unwrap();
        assert_eq!(sim.world().name(hit.entity), Some(name));
    }
    let center = sim.layout("cone").unwrap().screen.center();
    assert_eq!(
        sim.world().name(sim.pick(center).unwrap().entity),
        Some("cone")
    );
    let material = Material::rgb(0.1, 0.2, 0.3).alpha(0.4);
    assert_eq!(material.color, [0.1, 0.2, 0.3, 0.4]);
    for alpha in [-0.1, 1.1, f32::NAN] {
        assert!(std::panic::catch_unwind(|| Material::default().alpha(alpha)).is_err());
    }
}

#[test]
fn adding_primitives_preserves_the_existing_asset_variant_hash_ordinal() {
    let legacy = |ordinal| {
        let mut hash = hash::Hasher::default();
        hash.variant("Asset", ordinal);
        hash.begin_seq(1);
        hash.item();
        "crate.model".to_owned().write(&mut hash);
        hash.end_seq();
        hash.end_variant();
        hash.finish()
    };
    assert_eq!(hash::of(&Mesh::asset("crate.model")), legacy(5));
    assert_ne!(
        legacy(5),
        legacy(8),
        "the ordinal is part of the simulation hash"
    );
}

struct ConeRays;
impl Game for ConeRays {
    const ID: &'static str = "cone-rays";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("camera", (Transform::default(), Camera::orthographic(4.)));
        w.spawn_named("cone", (Transform::default(), Mesh::cone(1., 2.)));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn cone_picks_side_cap_inside_and_transformed_rays() {
    let mut sim = Sim::<ConeRays>::new(()).unwrap();
    sim.viewport(400., 400.);
    for transform in [
        Transform::default(),
        Transform {
            scale: Vec3::new(1., -1., 1.),
            ..Default::default()
        },
        Transform {
            position: Vec3::new(2., -1., 3.),
            rotation: Quat::from_rotation_z(0.7),
            scale: Vec3::new(2., 1.5, 0.5),
        },
    ] {
        *sim.world().require_mut::<Transform>("cone") = transform;
        let cone = sim.world().named("cone").unwrap();
        if transform.scale.y < 0. {
            // An unresolved socket still draws the ordinary primitive fallback.
            sim.world_mut()
                .insert(cone, SocketFollow::new("missing", "tip"));
        } else {
            sim.world_mut().remove::<SocketFollow>(cone);
        }
        let affine = Affine3A::from_scale_rotation_translation(
            transform.scale.abs(),
            transform.rotation,
            transform.position,
        );
        for (origin, direction, expected) in [
            (
                Vec3::new(2., 0., 0.),
                -Vec3::X,
                Some(Vec3::new(0.5, 0., 0.)),
            ),
            (
                Vec3::new(2., 0.5, 0.),
                -Vec3::X,
                Some(Vec3::new(0.25, 0.5, 0.)),
            ),
            (
                Vec3::new(0.25, -3., 0.),
                Vec3::Y,
                Some(Vec3::new(0.25, -1., 0.)),
            ),
            (Vec3::ZERO, Vec3::X, Some(Vec3::new(0.5, 0., 0.))),
            (Vec3::new(2., 0., 0.), Vec3::Z, None),
        ] {
            let eye = affine.transform_point3(origin);
            let direction = affine.transform_vector3(direction).normalize();
            let up = if direction.dot(Vec3::Y).abs() > 0.99 {
                Vec3::Z
            } else {
                Vec3::Y
            };
            *sim.world().require_mut::<Transform>("camera") =
                Transform::at(eye.x, eye.y, eye.z).looking_at(eye + direction, up);
            let hit = sim.pick(Vec2::splat(200.));
            if let Some(point) = expected {
                let hit = hit.expect("ray hits cone surface");
                let point = affine.transform_point3(point);
                assert!(
                    (hit.point - point).length() < 0.002,
                    "{:?} != {point:?}",
                    hit.point
                );
            } else {
                assert!(hit.is_none(), "outside ray misses");
            }
        }
    }
}

#[test]
fn parent_v4_asset_save_loads_with_its_original_hash() {
    // Written by dd5a46579's World::save with Asset at ordinal 5, before this lane.
    let bytes = include_bytes!("fixtures/asset-v4.bin");
    let mut w = World::new(120, 7);
    w.register::<Transform>()
        .register::<Mesh>()
        .register::<Material>();
    w.load(bytes).unwrap();
    assert_eq!(w.hash(), 0x70e72ecb97148060);
    assert_eq!(
        w.require::<Mesh>("legacy-asset").clone(),
        Mesh::asset("crate.model")
    );
    assert_eq!(w.save(), bytes);
}
