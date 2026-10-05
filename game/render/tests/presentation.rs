//! Presentation state through the Parent hierarchy, as drawn: an offset root
//! moves its children, an opacity root fades its children and their shadows.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Surface, Value};

fn frame() -> Frame {
    Frame {
        width: 128.,
        height: 128.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    }
}

#[derive(Default, Args)]
struct UnitArgs {
    bob: f32,
    opacity: f32,
}
// A two-part unit: a root block and a child block 2 m to its right, on a
// ground lit by a low sun, seen from the front and above.
struct Unit;
impl Game for Unit {
    const ID: &'static str = "presentation-unit";
    type Args = UnitArgs;
    fn setup(w: &mut World, _: &UnitArgs) {
        w.insert_resource(Environment {
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.spawn((
            Transform::at(1., 3., 4.5).looking_at(Vec3::new(1., 1., 0.), Vec3::Y),
            Camera::default(),
        ));
        w.spawn((Transform::default(), Mesh::plane(30., 30.)));
        let root = w.spawn_named("root", (Transform::at(0., 1., 0.), Mesh::cube(1.)));
        w.spawn_named(
            "child",
            (Transform::at(2., 0., 0.), Parent(root), Mesh::cube(1.)),
        );
        w.spawn((
            Transform::at(6., 6., 3.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &UnitArgs) {}
    fn present(w: &mut Present<'_>, args: &UnitArgs) {
        let root = w.named("root").unwrap();
        if args.bob != 0. {
            w.insert(root, Offset(Transform::at(0., args.bob, 0.)));
        }
        if args.opacity < 1. {
            w.insert(root, Opacity(args.opacity));
        }
    }
}
fn render(gpu: &exact_gpu::Gpu, bob: f32, opacity: f32) -> fixture::Pixels {
    let mut s = WorldSurface::<Unit>::default();
    s.bind(
        &[
            Value::Number(f64::from(bob)),
            Value::Number(f64::from(opacity)),
        ],
        None,
    )
    .unwrap();
    let (p, _) = fixture::render(gpu, &mut s, &frame()).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    p
}
// Rows and columns where the frame differs from `base`.
fn changed(a: &fixture::Pixels, base: &fixture::Pixels) -> Vec<(u32, u32)> {
    (0..128)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .filter(|&(x, y)| (0..3).any(|c| a.at(x, y)[c].abs_diff(base.at(x, y)[c]) > 24))
        .collect()
}

#[test]
fn a_bobbing_root_moves_its_children_as_drawn() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let rest = render(&gpu, 0., 1.);
    let up = render(&gpu, 0.75, 1.);
    let moved = changed(&up, &rest);
    // The child block (right of centre) moves with the root, not just the root.
    let right = moved.iter().filter(|&&(x, _)| x > 72).count();
    let left = moved.iter().filter(|&&(x, _)| x < 56).count();
    eprintln!("changed texels: left {left}, right {right}");
    assert!(left > 30 && right > 30, "left {left}, right {right}");
}

#[test]
fn a_faded_root_fades_its_children_and_their_shadows() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let solid = render(&gpu, 0., 1.);
    let half = render(&gpu, 0., 0.5);
    let gone = render(&gpu, 0., 0.);
    let ground = render(&gpu, 0., 0.);
    // At Opacity 0 the whole unit and its shadows vanish: only ground remains.
    assert!(changed(&gone, &ground).is_empty());
    let all = changed(&solid, &gone).len();
    let some = changed(&half, &gone).len();
    // Child texels (right of centre) dither too.
    let right = |v: &[(u32, u32)]| v.iter().filter(|&&(x, _)| x > 72).count();
    let (solid_right, half_right) = (
        right(&changed(&solid, &gone)),
        right(&changed(&half, &gone)),
    );
    eprintln!("unit texels: solid {all}, half {some}; child side {solid_right} vs {half_right}");
    assert!(some > all / 4 && some < all * 3 / 4, "{some} of {all}");
    assert!(
        half_right > 0 && half_right < solid_right,
        "{half_right} of {solid_right}"
    );
}

#[derive(Default, Args)]
struct HiddenArgs {
    hidden: bool,
}
// A hidden root over a faded model child with levels of detail.
struct Hidden;
impl Game for Hidden {
    const ID: &'static str = "presentation-hidden";
    type Args = HiddenArgs;
    fn setup(w: &mut World, args: &HiddenArgs) {
        w.insert_resource(Environment {
            fog: None,
            bloom: None,
            background: Some([0.; 3]),
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., 5.), Camera::default()));
        let quad = asset::MeshData {
            positions: vec![-1., -1., 0., 1., -1., 0., 1., 1., 0., -1., 1., 0.],
            normals: [0., 0., 1.].repeat(4),
            uvs: vec![0., 1., 1., 1., 1., 0., 0., 0.],
            indices: vec![0, 1, 2, 0, 2, 3],
            bounds: [-1., -1., 0., 1., 1., 0.],
            ..Default::default()
        };
        let near = w.generated("near.model", quad.clone()).unwrap();
        w.generated("far.model", quad).unwrap();
        let root = w.spawn_named("root", Transform::default());
        if args.hidden {
            w.insert(root, Visible(false));
        }
        w.spawn_named(
            "child",
            (
                Transform::default(),
                Parent(root),
                near,
                ModelLod {
                    levels: vec![LodLevel {
                        distance: 10.,
                        model: "far.model".into(),
                    }],
                    hide: None,
                },
            ),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &HiddenArgs) {}
    fn present(w: &mut Present<'_>, _: &HiddenArgs) {
        // Neither a fade nor a level of detail reveals it.
        let child = w.named("child").unwrap();
        w.insert(child, Opacity(0.5));
    }
}

#[test]
fn a_hidden_ancestor_overrides_opacity_and_levels_of_detail() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let lit = |hidden: bool| {
        let mut s = WorldSurface::<Hidden, exact_game_render::ModelExecutor, true>::default();
        s.bind(&[Value::Bool(hidden)], None).unwrap();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.prepare_assets(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        let (p, _) = fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert!(s.error().is_none(), "{:?}", s.error());
        (0..128)
            .flat_map(|y| (0..128).map(move |x| (x, y)))
            .filter(|&(x, y)| p.at(x, y)[1] > 20)
            .count()
    };
    let (shown, hidden) = (lit(false), lit(true));
    assert!(shown > 500, "the faded child draws: {shown}");
    assert_eq!(hidden, 0, "a hidden ancestor hides it");
}

#[derive(Default, Args)]
struct TintArgs {
    tinted: bool,
}
struct Tinted;
impl Game for Tinted {
    const ID: &'static str = "presentation-tint";
    type Args = TintArgs;
    fn setup(w: &mut World, _: &TintArgs) {
        w.insert_resource(Environment {
            fog: None,
            bloom: None,
            background: Some([0.; 3]),
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., 4.), Camera::default()));
        w.spawn_named("block", (Transform::default(), Mesh::cube(1.5)));
    }
    fn tick(_: &mut World, _: &Input, _: &TintArgs) {}
    fn present(p: &mut Present<'_>, args: &TintArgs) {
        if args.tinted {
            let e = p.named("block").unwrap();
            p.insert(
                e,
                Tint {
                    color: [1., 0.1, 0.1, 1.],
                    emissive: [0., 0., 0.5],
                },
            );
        }
    }
}

#[test]
fn a_primitive_tint_multiplies_its_colour_and_adds_emission() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let centre = |tinted: bool| {
        let mut s = WorldSurface::<Tinted>::default();
        s.bind(&[Value::Bool(tinted)], None).unwrap();
        let (p, _) = fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert!(s.error().is_none(), "{:?}", s.error());
        p.at(64, 64)
    };
    let (plain, tinted) = (centre(false), centre(true));
    eprintln!("plain {plain:?}, tinted {tinted:?}");
    assert!(
        tinted[1] < plain[1] / 2,
        "green multiplied down: {tinted:?}"
    );
    assert!(tinted[2] > plain[2] + 20, "blue emission added: {tinted:?}");
    assert!(tinted[0].abs_diff(plain[0]) < 10, "red kept: {tinted:?}");
}
