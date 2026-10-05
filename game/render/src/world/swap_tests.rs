//! Presentation swaps: `DrawnMesh`, `DrawnLight` and `DrawnEnvironment` draw in
//! place of the simulated mesh, light and sky, and only their content moves
//! the feed.
use super::*;
use exact_game::{
    AmbientOcclusion, DirectionalLight, DrawnEnvironment, DrawnLight, DrawnMesh, Environment,
    LodLevel, ModelLod, PointLight,
};

fn model() -> Recording {
    Recording {
        models: vec![(MeshId(99), crate::MaterialId(0), glam::Mat4::IDENTITY, None)],
        ..Default::default()
    }
}

#[test]
fn a_drawn_mesh_replaces_the_simulated_shape_material_and_levels() {
    let mut w = World::new(60, 0);
    let cube = w.spawn((
        Transform::default(),
        Mesh::cube(1.),
        Material::rgb(1., 0., 0.),
    ));
    // A prop with no simulated mesh draws only what present gives it.
    let prop = w.spawn(Transform::at(3., 0., 0.));
    let plant = w.spawn((Transform::default(), Mesh::asset("seedling.model")));
    let mut f = Feed::default();
    let mut r = model();
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.contains(&cube.index()) && !r.slots.contains(&prop.index()));
    assert_eq!(r.instances.len(), 1);
    // The present: a sphere for the cube in its own colour, a cylinder for the
    // prop, and a grown model with a far level for the plant.
    let blue = Material::rgb(0., 0., 1.);
    w.insert(cube, DrawnMesh::new(Mesh::sphere(2.)).material(blue));
    w.insert(prop, DrawnMesh::new(Mesh::cylinder(0.5, 2.)));
    let far = ModelLod {
        levels: vec![LodLevel {
            distance: 20.,
            model: "grown-far.model".into(),
        }],
        hide: None,
    };
    w.insert(plant, DrawnMesh::model("grown.model").lod(far));
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.contains(&cube.index()) && r.slots.contains(&prop.index()));
    let sphere = r.meshes.len() - 2;
    let shape = |slot: u32| {
        r.batches
            .iter()
            .find(|b| r.slots[b.slots.start as usize..b.slots.end as usize].contains(&slot))
            .map(|b| b.mesh)
    };
    assert_eq!(shape(cube.index()), Some(MeshId(sphere)));
    let at = cube.index() as usize * 12;
    assert_eq!(
        &r.materials[at..at + 3],
        &[0., 0., 1.],
        "the drawn material"
    );
    assert_eq!(
        &r.materials[at + 9..at + 12],
        &[4., 4., 4.],
        "the sphere's size"
    );
    // The model and its far level: two records for the one entity.
    assert_eq!(r.instances.len(), 2);
    assert!(r.instances.iter().all(|i| i.transform == plant.index()));
    // Present rewrites the same rows each tick: unchanged content rebatches nothing.
    r.calls.clear();
    let rows: Vec<_> = [cube, prop, plant]
        .iter()
        .map(|&e| (e, w.get::<DrawnMesh>(e).unwrap().clone()))
        .collect();
    for (e, d) in rows {
        w.remove::<DrawnMesh>(e);
        w.insert(e, d);
    }
    f.feed_to(&w, &mut r).unwrap();
    assert!(!r.calls.contains(&Call::Batches), "{:?}", r.calls);
    // Taking the swaps away draws the simulation again.
    for e in [cube, prop, plant] {
        w.remove::<DrawnMesh>(e);
    }
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.contains(&cube.index()) && !r.slots.contains(&prop.index()));
    assert_eq!(&r.materials[at..at + 3], &[1., 0., 0.]);
    assert_eq!(r.instances.len(), 1);
}

#[test]
fn drawn_lights_and_a_camera_sky_replace_the_simulated_ones() {
    let mut w = World::new(60, 0);
    let camera = w.spawn((Transform::default(), Camera::default()));
    let sun = w.spawn((
        Transform::default(),
        DirectionalLight {
            illuminance: 100.,
            ..Default::default()
        },
    ));
    // A moon with no simulated light, and a lantern lit only as drawn.
    let moon = w.spawn(Transform::default());
    let lantern = w.spawn(Transform::at(1., 0., 0.));
    w.insert_resource(Environment {
        exposure: 0.5,
        ..Environment::default()
    });
    w.insert_resource(AmbientOcclusion::default());
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let frame = f.frame(&w, 1., 1.);
    assert_eq!(
        frame.sun.unwrap().illuminance,
        100. * scene::PHOTOMETRIC_SCALE
    );
    assert!(frame.fill.is_none() && frame.lights.is_empty());
    assert_eq!(frame.environment.exposure, 0.5);
    assert!(frame.ambient_occlusion.is_some());
    let night = |illuminance| {
        DrawnLight::Directional(DirectionalLight {
            illuminance,
            shadows: false,
            ..Default::default()
        })
    };
    w.insert(sun, night(10.));
    w.insert(moon, night(30.));
    w.insert(
        lantern,
        DrawnLight::Point(PointLight {
            intensity: 50.,
            ..Default::default()
        }),
    );
    w.insert(
        camera,
        DrawnEnvironment {
            environment: Environment {
                exposure: 1.5,
                ..Environment::default()
            },
            ambient_occlusion: None,
        },
    );
    f.feed_to(&w, &mut r).unwrap();
    let frame = f.frame(&w, 1., 1.);
    assert_eq!(
        frame.sun.unwrap().illuminance,
        10. * scene::PHOTOMETRIC_SCALE
    );
    assert_eq!(
        frame.fill.unwrap().illuminance,
        30. * scene::PHOTOMETRIC_SCALE
    );
    assert_eq!(frame.lights.len(), 1);
    assert_eq!(frame.lights[0].intensity, 50. * scene::PHOTOMETRIC_SCALE);
    assert_eq!(frame.environment.exposure, 1.5);
    assert!(frame.ambient_occlusion.is_none());
    // A hidden lantern's drawn light is hidden with it.
    w.insert(lantern, Visible(false));
    f.feed_to(&w, &mut r).unwrap();
    assert!(f.frame(&w, 1., 1.).lights.is_empty());
    for e in [sun, moon, lantern] {
        w.remove::<DrawnLight>(e);
    }
    w.remove::<DrawnEnvironment>(camera);
    f.feed_to(&w, &mut r).unwrap();
    let frame = f.frame(&w, 1., 1.);
    assert_eq!(
        frame.sun.unwrap().illuminance,
        100. * scene::PHOTOMETRIC_SCALE
    );
    assert!(frame.fill.is_none());
    assert_eq!(frame.environment.exposure, 0.5);
}

#[test]
fn opacity_fades_particles_and_hides_them_at_zero() {
    let mut w = World::new(60, 0);
    let root = w.spawn((Transform::default(), exact_game::Opacity(0.5)));
    let rain = w.spawn((
        Transform::default(),
        Parent(root),
        exact_game::Emitter::default(),
    ));
    w.propagate();
    let mut items: Vec<crate::quads::Item<exact_game::Emitter>> = Vec::new();
    crate::quads::feed(&w, &mut items, true, false, false);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].opacity, 0.5);
    w.insert(rain, exact_game::Opacity(0.));
    crate::quads::feed(&w, &mut items, false, false, false);
    assert!(items.is_empty(), "an emitter at zero opacity draws nothing");
    w.remove::<exact_game::Opacity>(rain);
    crate::quads::feed(&w, &mut items, false, false, false);
    assert_eq!(items.len(), 1);
}
