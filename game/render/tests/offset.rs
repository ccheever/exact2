//! exact_game::Offset: a presentation component that moves only the drawn pose.
use exact_game::*;
use exact_game_render::WorldSurface;
#[cfg(not(target_arch = "wasm32"))]
use exact_gpu::fixture;
use exact_gpu::{Frame, Surface};

#[derive(Default, Args)]
struct Look {
    #[live]
    shift: f64,
}
struct Shifted;
impl Game for Shifted {
    const ID: &'static str = "offset-proof";
    type Args = Look;
    fn setup(w: &mut World, _: &Look) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            ..Default::default()
        });
        w.spawn_named(
            "box",
            (
                Transform::default(),
                Mesh::cube(1.),
                Material::rgb(1., 1., 1.),
            ),
        );
        w.spawn((Transform::at(0., 0., 10.), Camera::orthographic(8.)));
        w.spawn((
            Transform::at(0., 0., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &Look) {}
    fn paused(_: &Look) -> bool {
        true
    }
    fn present(w: &mut World, look: &Look) {
        let e = w.named("box").unwrap();
        w.insert(e, Offset(Transform::at(look.shift as f32, 0., 0.)));
    }
}

/// The lit box's horizontal centre in pixels, and the world hash.
fn drawn(gpu: &exact_gpu::Gpu, shift: f64) -> (f64, u64) {
    let mut surface = WorldSurface::<Shifted>::default();
    surface.bind(&[Value::Number(shift)], None).unwrap();
    let frame = Frame {
        width: 160.,
        height: 160.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let image = fixture::render(gpu, &mut surface, &frame).unwrap().0;
    let lit: Vec<u32> = (0..160)
        .flat_map(|y| (0..160).map(move |x| (x, y)))
        .filter(|&(x, y)| image.at(x, y)[0] > 60)
        .map(|(x, _)| x)
        .collect();
    assert!(!lit.is_empty(), "the box draws");
    let centre = lit.iter().map(|&x| f64::from(x)).sum::<f64>() / lit.len() as f64;
    (centre, surface.sim().unwrap().world().hash())
}

#[test]
fn an_offset_moves_the_drawn_box_and_nothing_simulated() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let (still, still_hash) = drawn(&gpu, 0.);
    let (moved, moved_hash) = drawn(&gpu, 2.);
    // 8 world units span 160 pixels: 2 units is 40 pixels to the right.
    assert!((moved - still - 40.).abs() < 3., "{still} -> {moved}");
    assert_eq!(still_hash, moved_hash);
}
