use super::*;
use exact_game::{DirectionalLight, Environment, Parent, PointLight, Visible};

#[derive(Default, exact_game::Args)]
struct Options {
    objects: bool,
}
struct VisibilityScene;
type VisibilitySurface = WorldSurface<VisibilityScene, exact_game_render::ModelPresentation, true>;

fn render(gpu: &Gpu, surface: &mut VisibilitySurface, now: f64, name: &str) -> Pixels {
    let (pixels, _) = fixture::render(gpu, surface, &frame(now)).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    pixels.save(name);
    pixels
}
impl Game for VisibilityScene {
    const ID: &'static str = "parent-visibility-pixels";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = Options;
    fn setup(w: &mut World, args: &Options) {
        w.insert_resource(Environment {
            bloom: None,
            fog: None,
            ..Default::default()
        });
        let root = w.spawn_named("root", (Transform::default(), Visible(true)));
        w.spawn((
            Transform::at(0., 3., 6.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
            Visible(false),
        ));
        w.spawn((
            Transform::at(3., 6., 3.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.spawn((
            Transform::default(),
            Mesh::plane(30., 30.),
            Material::rgb(0.4, 0.4, 0.4),
        ));
        if args.objects {
            w.spawn((
                Transform::at(-1., 0.6, 0.),
                Parent(root),
                Mesh::cube(1.2),
                Material::rgb(0.9, 0.15, 0.05),
            ));
            w.spawn((
                Transform::at(1., 0.7, 0.),
                Parent(root),
                Mesh::asset("crate.model"),
            ));
            w.spawn((
                Transform::at(0., 1.5, 0.),
                Parent(root),
                PointLight {
                    color: [0.1, 1., 0.2],
                    intensity: 30_000.,
                    ..Default::default()
                },
            ));
        }
    }
    fn tick(_: &mut World, _: &Input, _: &Options) {}
}

#[test]
fn parent_visibility_pixels_remove_mesh_model_shadow_and_light_without_a_tick() {
    let Some(gpu) = gpu() else { return };
    let make = |objects| {
        let mut s = VisibilitySurface::default();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.bind(&[Value::Bool(objects)], None).unwrap();
        let mut model: exact_game::asset::Model =
            exact_game::bin::from_slice(include_bytes!("../../../bake/tests/fixtures/crate.model"))
                .unwrap();
        model.materials.fill(Default::default());
        model.textures.clear();
        model.validate().unwrap();
        s.asset("crate.model", Ok(&exact_game::bin::to_vec(&model)));
        s.prepare_assets(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        assert!(s.assets().requests.is_empty());
        assert!(!s.sim().unwrap().is_loading());
        s
    };
    let mut surface = make(true);
    let shown = render(&gpu, &mut surface, 0., "parent-visibility-shown");
    let mut empty = make(false);
    let baseline = render(&gpu, &mut empty, 0., "parent-visibility-empty");
    assert!(
        shown
            .data
            .iter()
            .zip(&baseline.data)
            .filter(|(a, b)| a != b)
            .count()
            > 1000
    );
    surface
        .sim()
        .unwrap()
        .world()
        .require_mut::<Visible>("root")
        .0 = false;
    // A direct test edit must notify the surface; binding identical arguments
    // requests a feed without advancing time or restarting the simulation.
    surface.bind(&[Value::Bool(true)], None).unwrap();
    let hidden = render(&gpu, &mut surface, 0., "parent-visibility-hidden");
    assert!(
        hidden.data == baseline.data,
        "hidden objects leave no color, model, shadow or light pixels"
    );
    let saved = surface.carry().unwrap().unwrap();
    let mut fresh = make(true);
    render(&gpu, &mut fresh, 0., "parent-visibility-before-restore");
    fresh.restore(&saved, exact_gpu::Restore::Open).unwrap();
    let restored = render(&gpu, &mut fresh, 0., "parent-visibility-restored");
    assert!(restored.data == baseline.data, "restored hidden pixels differ");
    surface
        .sim()
        .unwrap()
        .world()
        .require_mut::<Visible>("root")
        .0 = true;
    surface.bind(&[Value::Bool(true)], None).unwrap();
    let revealed = render(&gpu, &mut surface, 0., "parent-visibility-revealed");
    assert!(revealed.data == shown.data, "revealed pixels differ from initial frame");
    assert_eq!(surface.sim().unwrap().world().tick(), 0);
}
