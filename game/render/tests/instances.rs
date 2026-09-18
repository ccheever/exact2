#![cfg(not(target_arch = "wasm32"))]
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
    let gpu = fixture::device().ok()?;
    let mut sim = Sim::<Test>::new(()).unwrap();
    sim.asset("panels.model", Some(&bin::to_vec(model)))
        .unwrap();
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
        bloom: None,
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
    let Ok(gpu) = fixture::device() else { return };
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
    assert_eq!(work, (5, 4));
    renderer.prepare_model("b.model", &m).unwrap();
    renderer.add_texture("shared.tex", &texture).unwrap();
    assert_eq!(renderer.asset_work(), work);
}
