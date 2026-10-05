#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::{asset::*, *};
use exact_game_render::{Feed, Renderer};
use exact_gpu::{fixture, wgpu};
use glam::camera::rh::{proj::directx, view};

fn material(color: [f32; 4], mode: AlphaMode) -> MaterialData {
    MaterialData {
        base_color: color,
        metallic: 0.,
        roughness: 1.,
        alpha_mode: mode,
        double_sided: true,
        ..Default::default()
    }
}
fn panel(material: u32) -> MeshData {
    MeshData {
        positions: vec![-0.8, -0.8, 0., 0.8, -0.8, 0., 0.8, 0.8, 0., -0.8, 0.8, 0.],
        normals: vec![0., 0., 1., 0., 0., 1., 0., 0., 1., 0., 0., 1.],
        uvs: vec![0., 1., 1., 1., 1., 0., 0., 0.],
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        bounds: [-0.8, -0.8, 0., 0.8, 0.8, 0.],
        ..Default::default()
    }
}
struct Test;
impl Game for Test {
    const ID: &'static str = "instances";
    const ASSETS: &'static [&'static str] = &["panels.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("model", (Transform::default(), Mesh::asset("panels.model")));
        w.spawn((Transform::at(0., 0., 5.), Camera::default()));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
fn image(model: &Model, eye: Vec3, tint: Option<Material>) -> Option<fixture::Pixels> {
    image_with_sun(model, eye, tint, None)
}
fn image_with_sun(
    model: &Model,
    eye: Vec3,
    tint: Option<Material>,
    sun: Option<exact_game_render::Sun>,
) -> Option<fixture::Pixels> {
    image_with_pose(model, eye, tint, sun, Transform::default())
}
fn image_with_pose(
    model: &Model,
    eye: Vec3,
    tint: Option<Material>,
    sun: Option<exact_game_render::Sun>,
    pose: Transform,
) -> Option<fixture::Pixels> {
    let gpu = crate::test_device::device_or_skip(exact_gpu::fixture::device())?;
    let mut sim = Sim::<Test>::new(()).unwrap();
    sim.asset("panels.model", Some(&bin::to_vec(model)))
        .unwrap();
    *sim.world().get_mut::<Transform>("model").unwrap() = pose;
    if let Some(tint) = tint {
        let e = sim.world().named("model").unwrap();
        sim.world_mut().insert(e, tint);
    }
    if sun.is_some() {
        sim.world_mut().spawn((
            Transform {
                position: Vec3::new(0., 0., -1.),
                scale: Vec3::new(4., 4., 0.05),
                ..Default::default()
            },
            Mesh::cube(1.),
            Material::rgb(1., 1., 1.),
        ));
    }
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer.prepare_model("panels.model", model).unwrap();
    let mut feed = Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut f = exact_game_render::FrameInput {
        view: view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y),
        proj: directx::orthographic(-2., 2., -2., 2., 0.1, 20.),
        camera_position: eye,
        sun,
        ..Default::default()
    };
    f.environment = exact_game_render::Environment {
        background: Some([0.; 3]),
        zenith: [1.; 3],
        horizon: [1.; 3],
        ground: [1.; 3],
        ambient: if sun.is_some() { 0.05 } else { 1. },
        fog: None,
        sun_disc: 0.,
        bloom: None,
        ..Default::default()
    };
    let stats = renderer.draw(&texture.create_view(&Default::default()), (256, 256), &f);
    assert_eq!(
        stats.instances,
        model.nodes.len() as u64 + u64::from(sun.is_some())
    );
    assert_eq!(
        sim.world().len(),
        2 + usize::from(sun.is_some()),
        "mesh nodes never become entities"
    );
    Some(fixture::read(&gpu, &texture).unwrap())
}
#[test]
fn transparent_nodes_sort_by_camera_and_double_sided_draws_back_faces() {
    let mut model = Model {
        meshes: vec![panel(0), panel(1)],
        materials: vec![
            material([1., 0., 0., 0.65], AlphaMode::Blend),
            material([0., 0., 1., 0.65], AlphaMode::Blend),
        ],
        bounds: [-0.8, -0.8, -0.3, 0.8, 0.8, 0.3],
        ..Default::default()
    };
    for (i, z) in [-0.3, 0.3].into_iter().enumerate() {
        model.nodes.push(Node {
            mesh: Some(i as u32),
            transform: glam::Mat4::from_translation(Vec3::new(0., 0., z)).to_cols_array(),
            ..Default::default()
        });
    }
    let Some(front) = image(&model, Vec3::new(0., 0., 5.), None) else {
        return;
    };
    let back = image(&model, Vec3::new(0., 0., -5.), None).unwrap();
    let a = front.at(128, 128);
    let b = back.at(128, 128);
    assert!(a[2] > a[0] + 20, "blue in front: {a:?}");
    assert!(b[0] > b[2] + 20, "red in front from back: {b:?}");
}
#[test]
fn model_local_offsets_materials_tint_glow_and_alpha_mask() {
    let mut model = Model {
        meshes: vec![panel(0), panel(1)],
        materials: vec![
            material([1., 1., 1., 1.], AlphaMode::Opaque),
            material([1., 1., 1., 0.], AlphaMode::Mask),
        ],
        bounds: [-1.8, -0.8, 0., 1.8, 0.8, 0.],
        ..Default::default()
    };
    for (i, x) in [-1., 1.].into_iter().enumerate() {
        model.nodes.push(Node {
            mesh: Some(i as u32),
            transform: glam::Mat4::from_translation(Vec3::new(x, 0., 0.)).to_cols_array(),
            ..Default::default()
        });
    }
    let Some(p) = image(
        &model,
        Vec3::new(0., 0., 5.),
        Some(Material::rgb(0., 1., 0.).emissive(0., 0., 1.)),
    ) else {
        return;
    };
    let left = p.at(64, 128);
    let right = p.at(192, 128);
    assert!(
        left[1] > 150 && left[2] > 150 && left[0] < 100,
        "entity tint and glow: {left:?}"
    );
    assert_eq!(right, [0, 0, 0, 255], "alpha mask removes right mesh node");
}

#[test]
fn mirrored_single_sided_nodes_keep_their_front_face() {
    let mut m = Model {
        meshes: vec![panel(0)],
        materials: vec![MaterialData {
            base_color: [1., 0., 0., 1.],
            metallic: 0.,
            ..Default::default()
        }],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        bounds: [-0.8, -0.8, 0., 0.8, 0.8, 0.],
        ..Default::default()
    };
    let Some(front) = image(&m, Vec3::new(0., 0., 5.), None) else {
        return;
    };
    m.nodes[0].transform = glam::Mat4::from_scale(Vec3::new(-1., 1., 1.)).to_cols_array();
    assert_eq!(front, image(&m, Vec3::new(0., 0., 5.), None).unwrap());
    for alpha_mode in [AlphaMode::Opaque, AlphaMode::Blend] {
        let mut entity_model = m.clone();
        entity_model.nodes[0].transform = glam::Mat4::IDENTITY.to_cols_array();
        entity_model.materials[0].alpha_mode = alpha_mode;
        let reflected = Transform::default().with_scale(Vec3::new(-1., 1., 1.));
        assert_eq!(
            image(&entity_model, Vec3::new(0., 0., 5.), None).unwrap(),
            image_with_pose(&entity_model, Vec3::new(0., 0., 5.), None, None, reflected).unwrap(),
            "mirrored entity owner keeps its visible face ({alpha_mode:?})"
        );
    }
    let back = image(&m, Vec3::new(0., 0., -5.), None).unwrap();
    assert_eq!(back.at(128, 128), [0, 0, 0, 255]);

    let sun = exact_game_render::Sun {
        direction: Vec3::new(1., 0., -1.),
        illuminance: 8.,
        ..Default::default()
    };
    let eye = Vec3::new(0., 0., 5.);
    let mirrored = image_with_sun(&m, eye, None, Some(sun)).unwrap();
    m.nodes[0].transform = glam::Mat4::IDENTITY.to_cols_array();
    let normal = image_with_sun(&m, eye, None, Some(sun)).unwrap();
    let unshadowed = image_with_sun(
        &m,
        eye,
        None,
        Some(exact_game_render::Sun {
            shadows: None,
            ..sun
        }),
    )
    .unwrap();
    let a = normal.at(210, 128);
    let b = mirrored.at(210, 128);
    let c = unshadowed.at(210, 128);
    assert!(
        c[0] > a[0] + 20,
        "panel must cast onto the receiver: {a:?} / {c:?}"
    );
    assert!(
        (0..3).all(|i| a[i].abs_diff(b[i]) <= 2),
        "mirrored shadow must match: {a:?} / {b:?}"
    );
}

#[test]
fn models_and_materials_share_named_textures_defaults_and_samplers() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(renderer.asset_work(), (0, 0));
    let m = Model {
        meshes: vec![panel(0), panel(1)],
        materials: vec![
            MaterialData {
                base_color_texture: Some(0),
                ..Default::default()
            };
            2
        ],
        textures: vec!["shared.tex".into()],
        nodes: vec![
            Node {
                mesh: Some(0),
                ..Default::default()
            },
            Node {
                mesh: Some(1),
                ..Default::default()
            },
        ],
        bounds: [-0.8, -0.8, 0., 0.8, 0.8, 0.],
        ..Default::default()
    };
    renderer.prepare_model("a.model", &m).unwrap();
    let texture = TextureData {
        width: 1,
        height: 1,
        mips: vec![vec![240, 0, 0, 255]],
        srgb: true,
        filter: [Filter::Nearest; 3],
        ..Default::default()
    };
    renderer.add_texture("shared.tex", &texture).unwrap();
    let work = renderer.asset_work();
    assert_eq!(work, (10, 4)); // Both attachment windings are prepared on delivery.
    renderer.prepare_model("b.model", &m).unwrap();
    renderer.add_texture("shared.tex", &texture).unwrap();
    assert_eq!(renderer.asset_work(), work);
}

// A blended viewmodel part behind a wall: drawn in the viewmodel layer's depth
// range, it shows through; unmarked, the wall hides it.
fn blended_behind_wall(marked: bool) -> Option<[u8; 4]> {
    let gpu = crate::test_device::device_or_skip(exact_gpu::fixture::device())?;
    let model = Model {
        meshes: vec![panel(0)],
        materials: vec![material([1., 0., 0., 0.8], AlphaMode::Blend)],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        bounds: [-0.8, -0.8, 0., 0.8, 0.8, 0.],
        ..Default::default()
    };
    let mut sim = Sim::<Test>::new(()).unwrap();
    sim.asset("panels.model", Some(&bin::to_vec(&model)))
        .unwrap();
    let w = sim.world_mut();
    w.spawn((
        Transform {
            position: Vec3::new(0., 0., 1.),
            scale: Vec3::new(4., 4., 0.1),
            ..Default::default()
        },
        Mesh::cube(1.),
        Material::rgb(0., 0., 1.),
    ));
    if marked {
        let e = w.named("model").unwrap();
        w.insert(e, ViewModel);
    }
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer.prepare_model("panels.model", &model).unwrap();
    let mut feed = Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let eye = Vec3::new(0., 0., 5.);
    let mut f = exact_game_render::FrameInput {
        view: view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y),
        proj: directx::orthographic(-2., 2., -2., 2., 0.1, 20.),
        camera_position: eye,
        sun: None,
        ..Default::default()
    };
    f.environment.fog = None;
    f.environment.bloom = None;
    renderer.draw(&texture.create_view(&Default::default()), (64, 64), &f);
    Some(fixture::read(&gpu, &texture).unwrap().at(32, 32))
}
#[test]
fn blended_viewmodel_parts_draw_in_front_of_walls() {
    let Some(plain) = blended_behind_wall(false) else {
        return;
    };
    let marked = blended_behind_wall(true).unwrap();
    assert!(
        plain[2] > plain[0],
        "the wall hides an unmarked panel: {plain:?}"
    );
    assert!(
        marked[0] > marked[2],
        "the viewmodel panel shows: {marked:?}"
    );
}

#[test]
fn node_materials_tint_and_light_named_nodes_of_one_instance() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut model = Model {
        meshes: vec![panel(0), panel(0)],
        materials: vec![material([1., 1., 1., 1.], AlphaMode::Opaque)],
        bounds: [-1.8, -0.8, 0., 1.8, 0.8, 0.],
        ..Default::default()
    };
    for (i, (name, x)) in [("uniform", -1.), ("visor", 1.)].into_iter().enumerate() {
        model.nodes.push(Node {
            name: name.into(),
            mesh: Some(i as u32),
            transform: glam::Mat4::from_translation(Vec3::new(x, 0., 0.)).to_cols_array(),
            ..Default::default()
        });
    }
    let mut sim = Sim::<Test>::new(()).unwrap();
    sim.asset("panels.model", Some(&bin::to_vec(&model)))
        .unwrap();
    let e = sim.world().named("model").unwrap();
    sim.world_mut().insert(
        e,
        NodeMaterials(vec![
            NodeMaterial {
                node: "uniform".into(),
                color: [1., 0.1, 0.1, 1.],
                ..Default::default()
            },
            NodeMaterial {
                node: "visor".into(),
                emissive: [0., 0., 4.],
                ..Default::default()
            },
        ]),
    );
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer.prepare_model("panels.model", &model).unwrap();
    let mut feed = Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 128,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let eye = Vec3::new(0., 0., 5.);
    let mut f = exact_game_render::FrameInput {
        view: view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y),
        proj: directx::orthographic(-2., 2., -1., 1., 0.1, 20.),
        camera_position: eye,
        sun: None,
        ..Default::default()
    };
    f.environment.fog = None;
    f.environment.bloom = None;
    f.environment.background = Some([0.; 3]);
    let stats = renderer.draw(&texture.create_view(&Default::default()), (128, 64), &f);
    // One merged draw: each part's look follows its vertices.
    assert_eq!(stats.instances, 1);
    let image = fixture::read(&gpu, &texture).unwrap();
    let (uniform, visor) = (image.at(32, 32), image.at(96, 32));
    assert!(
        uniform[0] > 2 * uniform[1].max(1),
        "uniform tinted red: {uniform:?}"
    );
    assert!(
        visor[2] > 200 && visor[2] > visor[0] + 40,
        "visor glows blue: {visor:?}"
    );
}

#[test]
fn every_part_of_a_many_part_merge_finds_its_own_look() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    // Eleven panels in a row, one merged draw; every fourth has no look.
    let mut model = Model {
        meshes: vec![panel(0); 11],
        materials: vec![material([1., 1., 1., 1.], AlphaMode::Opaque)],
        bounds: [-2.7, -0.8, 0., 2.7, 0.8, 0.],
        ..Default::default()
    };
    let mut looks = Vec::new();
    for i in 0..11 {
        let name = format!("part{i}");
        model.nodes.push(Node {
            name: name.clone(),
            mesh: Some(i),
            transform: glam::Mat4::from_scale_rotation_translation(
                Vec3::new(0.25, 1., 1.),
                glam::Quat::IDENTITY,
                Vec3::new(-2.5 + 0.5 * i as f32, 0., 0.),
            )
            .to_cols_array(),
            ..Default::default()
        });
        if i % 4 != 3 {
            let mut color = [0.05, 0.05, 0.05, 1.];
            color[i as usize % 3] = 1.;
            looks.push(NodeMaterial {
                node: name,
                color,
                ..Default::default()
            });
        }
    }
    let mut sim = Sim::<Test>::new(()).unwrap();
    sim.asset("panels.model", Some(&bin::to_vec(&model)))
        .unwrap();
    let e = sim.world().named("model").unwrap();
    sim.world_mut().insert(e, NodeMaterials(looks));
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer.prepare_model("panels.model", &model).unwrap();
    let mut feed = Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 192,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let eye = Vec3::new(0., 0., 5.);
    let mut f = exact_game_render::FrameInput {
        view: view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y),
        proj: directx::orthographic(-3., 3., -1., 1., 0.1, 20.),
        camera_position: eye,
        sun: None,
        ..Default::default()
    };
    f.environment.fog = None;
    f.environment.bloom = None;
    f.environment.background = Some([0.; 3]);
    let stats = renderer.draw(&texture.create_view(&Default::default()), (192, 64), &f);
    assert_eq!(stats.instances, 1, "one merged draw");
    let image = fixture::read(&gpu, &texture).unwrap();
    for i in 0..11 {
        // 32 pixels a unit: part i's centre.
        let p = image.at(((0.5 + 0.5 * i as f32) * 32.) as u32, 32);
        if i % 4 == 3 {
            let (low, high) = (p[..3].iter().min().unwrap(), p[..3].iter().max().unwrap());
            assert!(*low > 40 && *high < 2 * *low, "part {i} untinted: {p:?}");
        } else {
            let c = i % 3;
            let other = (0..3).filter(|&k| k != c).map(|k| p[k]).max().unwrap();
            assert!(p[c] > 2 * other.max(1), "part {i} channel {c}: {p:?}");
        }
    }
}

#[test]
fn static_parts_sharing_a_material_draw_once_and_look_the_same() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut model = Model {
        meshes: vec![panel(0), panel(0), panel(0)],
        materials: vec![material([0.9, 0.6, 0.3, 1.], AlphaMode::Opaque)],
        bounds: [-2.4, -0.8, -0.1, 2.4, 0.8, 0.1],
        ..Default::default()
    };
    // The third is mirrored (negative x scale): merging must rewind it.
    for (i, (x, sx)) in [(-1.6, 1.), (0., 0.5), (1.6, -1.)].into_iter().enumerate() {
        model.nodes.push(Node {
            name: format!("part{i}"),
            mesh: Some(i as u32),
            transform: glam::Mat4::from_scale_rotation_translation(
                Vec3::new(sx, 1., 1.),
                glam::Quat::from_rotation_y(0.3),
                Vec3::new(x, 0., 0.),
            )
            .to_cols_array(),
            ..Default::default()
        });
    }
    for m in &mut model.materials {
        m.double_sided = false;
    }
    // The reference gives each part its own (equal) material: nothing merges.
    let mut apart = model.clone();
    apart.materials = vec![apart.materials[0].clone(); 3];
    for (i, mesh) in apart.meshes.iter_mut().enumerate() {
        mesh.material = i as u32;
    }
    let render = |separate: bool| {
        let model = if separate { &apart } else { &model };
        let mut sim = Sim::<Test>::new(()).unwrap();
        sim.asset("panels.model", Some(&bin::to_vec(model)))
            .unwrap();
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer.prepare_model("panels.model", model).unwrap();
        let mut feed = Feed::default();
        feed.feed(sim.world(), &mut renderer).unwrap();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 160,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let eye = Vec3::new(0., 0., 5.);
        let mut f = exact_game_render::FrameInput {
            view: view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y),
            proj: directx::orthographic(-2.5, 2.5, -1., 1., 0.1, 20.),
            camera_position: eye,
            ..Default::default()
        };
        f.environment.fog = None;
        f.environment.bloom = None;
        f.environment.background = Some([0.; 3]);
        let stats = renderer.draw(&texture.create_view(&Default::default()), (160, 64), &f);
        (stats, fixture::read(&gpu, &texture).unwrap())
    };
    let (merged, a) = render(false);
    let (parts, b) = render(true);
    assert_eq!(parts.instances, 3);
    assert_eq!(merged.instances, 1);
    assert!(merged.draws < parts.draws, "{merged:?} vs {parts:?}");
    assert_eq!(merged.triangles, parts.triangles);
    let lit = |p: &fixture::Pixels| {
        (0..64)
            .flat_map(|y| (0..160).map(move |x| (x, y)))
            .filter(|&(x, y)| p.at(x, y)[0] > 20)
            .count()
    };
    assert!(lit(&a) > 2000, "{}", lit(&a));
    let mut differ = 0;
    for y in 0..64 {
        for x in 0..160 {
            let (p, q) = (a.at(x, y), b.at(x, y));
            if (0..3).any(|c| p[c].abs_diff(q[c]) > 2) {
                differ += 1;
            }
        }
    }
    assert!(
        differ <= 8,
        "{differ} pixels differ between merged and per-part draws"
    );
}

struct Swing;
impl Game for Swing {
    const ID: &'static str = "instances-swing";
    const ASSETS: &'static [&'static str] = &["arm.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let mut clip = Animation::play("swing").speed(0.);
        clip.time = 0.5;
        w.spawn_named(
            "model",
            (Transform::default(), Mesh::asset("arm.model"), clip),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        animation::step(w);
    }
}

#[test]
fn animated_rigid_parts_sharing_a_material_draw_once_and_follow_their_nodes() {
    use exact_game::asset::{Clip, Track, TrackPath};
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    // An arm: the root panel turns about Z, its child panel 1.8 m along it.
    let mut model = Model {
        meshes: vec![panel(0), panel(0)],
        materials: vec![material([0.9, 0.6, 0.3, 1.], AlphaMode::Opaque)],
        bounds: [-3., -3., 0., 3., 3., 0.],
        ..Default::default()
    };
    model.nodes.push(Node {
        name: "upper".into(),
        mesh: Some(0),
        transform: glam::Mat4::from_translation(Vec3::new(-0.9, 0., 0.)).to_cols_array(),
        ..Default::default()
    });
    model.nodes.push(Node {
        name: "lower".into(),
        parent: Some(0),
        mesh: Some(1),
        transform: glam::Mat4::from_translation(Vec3::new(1.8, 0., 0.)).to_cols_array(),
        ..Default::default()
    });
    model.clips = vec![Clip {
        name: "swing".into(),
        tracks: vec![Track {
            node: 0,
            path: TrackPath::Rotation,
            times: vec![0., 1.],
            values: [
                glam::Quat::IDENTITY.to_array(),
                glam::Quat::from_rotation_z(1.2).to_array(),
            ]
            .concat(),
            ..Default::default()
        }],
        ..Default::default()
    }];
    let mut apart = model.clone();
    apart.materials = vec![apart.materials[0].clone(); 2];
    apart.meshes[1].material = 1;
    let render = |separate: bool, looks: bool| {
        let model = if separate { &apart } else { &model };
        let mut sim = Sim::<Swing>::new(()).unwrap();
        sim.asset("arm.model", Some(&bin::to_vec(model))).unwrap();
        sim.run(100.);
        if looks {
            let e = sim.world().named("model").unwrap();
            let blue = NodeMaterial {
                node: "lower".into(),
                color: [0.1, 0.4, 1., 1.],
                ..Default::default()
            };
            sim.world_mut().insert(e, NodeMaterials(vec![blue]));
        }
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer.prepare_model("arm.model", model).unwrap();
        let mut feed = Feed::default();
        feed.feed(sim.world(), &mut renderer).unwrap();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let eye = Vec3::new(0., 0., 5.);
        let mut f = exact_game_render::FrameInput {
            view: view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y),
            proj: directx::orthographic(-3., 3., -3., 3., 0.1, 20.),
            camera_position: eye,
            alpha: 1.,
            ..Default::default()
        };
        f.environment.fog = None;
        f.environment.bloom = None;
        f.environment.background = Some([0.; 3]);
        let stats = renderer.draw(&texture.create_view(&Default::default()), (128, 128), &f);
        (stats, fixture::read(&gpu, &texture).unwrap())
    };
    let (merged, a) = render(false, false);
    let (parts, b) = render(true, false);
    assert_eq!((merged.instances, parts.instances), (1, 2));
    // A per-part look on the merged, animated draw.
    let (_, looked) = render(false, true);
    let lit = |p: &fixture::Pixels, x: u32, y: u32| p.at(x, y)[0] > 20;
    // Turned 0.6 rad: the lower panel's centre rises above the bind pose's row.
    let (cx, cy) = (
        64. + (-0.9 + 1.8 * 0.6f32.cos()) / 6. * 128.,
        64. - 1.8 * 0.6f32.sin() / 6. * 128.,
    );
    assert!(
        lit(&a, cx as u32, cy as u32),
        "the lower part follows its node"
    );
    assert!(
        !lit(&a, 64 + 19, 64),
        "nothing left at the bind pose's lower part"
    );
    let mut differ = 0;
    for y in 0..128 {
        for x in 0..128 {
            let (p, q) = (a.at(x, y), b.at(x, y));
            if (0..3).any(|c| p[c].abs_diff(q[c]) > 2) {
                differ += 1;
            }
        }
    }
    assert!(
        differ <= 8,
        "{differ} pixels differ between merged and per-part draws"
    );
    let lower = looked.at(cx as u32, cy as u32);
    assert!(lower[2] > lower[0], "the lower part is blue: {lower:?}");
    assert_eq!(
        looked.at(40, 64),
        a.at(40, 64),
        "the upper part keeps its look"
    );
}

#[test]
fn material_overrides_recolour_one_material_and_keep_instances_together() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut model = Model {
        meshes: vec![panel(0), panel(1)],
        materials: vec![
            // Authored pure red: blue and green are zero factors.
            material([0.9, 0., 0., 1.], AlphaMode::Opaque),
            material([1., 1., 1., 1.], AlphaMode::Opaque),
        ],
        bounds: [-1.8, -0.8, 0., 1.8, 0.8, 0.],
        ..Default::default()
    };
    for (i, (name, x)) in [("armour", -1.), ("body", 1.)].into_iter().enumerate() {
        model.nodes.push(Node {
            name: name.into(),
            mesh: Some(i as u32),
            transform: glam::Mat4::from_translation(Vec3::new(x, 0., 0.)).to_cols_array(),
            ..Default::default()
        });
    }
    let blue = MaterialOverrides(vec![MaterialOverride {
        material: 0,
        color: Some([0.1, 0.3, 0.9, 1.]),
        ..Default::default()
    }]);
    let render = |looks: Option<&MaterialOverrides>, second: bool| {
        let mut sim = Sim::<Test>::new(()).unwrap();
        sim.asset("panels.model", Some(&bin::to_vec(&model)))
            .unwrap();
        let e = sim.world().named("model").unwrap();
        if let Some(looks) = looks {
            sim.world_mut().insert(e, looks.clone());
        }
        if second {
            // Another team's soldier: the same model, its own colours.
            sim.world_mut()
                .spawn((Transform::at(0., 0., -3.), Mesh::asset("panels.model")));
        }
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer.prepare_model("panels.model", &model).unwrap();
        let mut feed = Feed::default();
        feed.feed(sim.world(), &mut renderer).unwrap();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 128,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let eye = Vec3::new(0., 0., 5.);
        let mut f = exact_game_render::FrameInput {
            view: view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y),
            proj: directx::orthographic(-2., 2., -1., 1., 0.1, 20.),
            camera_position: eye,
            sun: None,
            ..Default::default()
        };
        f.environment.fog = None;
        f.environment.bloom = None;
        f.environment.background = Some([0.; 3]);
        let stats = renderer.draw(&texture.create_view(&Default::default()), (128, 64), &f);
        (stats, fixture::read(&gpu, &texture).unwrap())
    };
    let (one, plain) = render(None, false);
    let (_, recoloured) = render(Some(&blue), false);
    let (two, _) = render(Some(&blue), true);
    let (armour, body) = (plain.at(32, 32), plain.at(96, 32));
    assert!(armour[0] > armour[2] + 30, "authored red: {armour:?}");
    let armour = recoloured.at(32, 32);
    assert!(armour[2] > armour[0] + 30, "overridden blue: {armour:?}");
    assert_eq!(
        recoloured.at(96, 32),
        body,
        "the other material keeps its look"
    );
    // Instances with different overrides share their batches.
    assert_eq!((two.draws, two.instances), (one.draws, 2 * one.instances));
}
