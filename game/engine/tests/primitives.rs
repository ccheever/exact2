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
