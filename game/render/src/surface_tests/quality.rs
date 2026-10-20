//! LLP 1139.003 D3: a rung changes the picture and never the world.
use super::*;
use exact_game::{DirectionalLight, Material, Quality, Shadows};

#[derive(Default, exact_game::Args)]
struct LadderArgs {
    /// 1 asks for `Quality::Low`, anything else `High`.
    q: u32,
    /// 1 inserts the game's own `Shadows`.
    authored: u32,
}
struct Ladder;
impl Game for Ladder {
    const ID: &'static str = "quality-ladder";
    const HZ: u32 = 120;
    type Args = LadderArgs;
    fn quality(args: &LadderArgs) -> Quality {
        if args.q == 1 {
            Quality::Low
        } else {
            Quality::High
        }
    }
    fn setup(w: &mut World, args: &LadderArgs) {
        if args.authored == 1 {
            w.insert_resource(Shadows::default());
        }
        w.spawn((
            Transform::default(),
            Mesh::plane(20., 20.),
            Material::hex("#d0d0d0"),
        ));
        w.spawn_named("cube", (Transform::at(0., 1.5, 0.), Mesh::cube(2.)));
        w.spawn((
            Transform::at(3., 6., 1.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.spawn((
            Transform::at(0., 12., 0.01).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
        ));
    }
    fn tick(w: &mut World, _: &Input, _: &LadderArgs) {
        let tick = w.tick() as f32;
        if let Some(mut t) = w.get_mut::<Transform>("cube") {
            t.position.x = (tick * 0.01).sin();
        }
    }
}

/// The hash and tick at 5 s (600 ticks), the quality row, the raster scale
/// and the frame's mean brightness, for one rung.
fn run(gpu: &Gpu, q: u32, authored: u32) -> (String, u64, serde_json::Value, f32, f64) {
    let mut s = WorldSurface::<Ladder>::default();
    s.bind(
        &[Value::Number(q.into()), Value::Number(authored.into())],
        None,
    )
    .unwrap();
    let mut pixels = None;
    for i in 0..=10 {
        pixels = Some(
            fixture::render(gpu, &mut s, &frame(i as f64 * 500.))
                .unwrap()
                .0,
        );
        assert!(s.error().is_none(), "{:?}", s.error());
    }
    let state: serde_json::Value =
        serde_json::from_str(&s.agent(r#"{"op":"state"}"#).unwrap()).unwrap();
    let world = &state["world"];
    let data = pixels.unwrap().data;
    let mean = data
        .chunks(4)
        .map(|p| f64::from(p[0]) + f64::from(p[1]) + f64::from(p[2]))
        .sum::<f64>()
        / (data.len() / 4) as f64;
    (
        world["hash"].as_str().unwrap().to_owned(),
        world["tick"].as_u64().unwrap(),
        world["quality"].clone(),
        exact_gpu::Surface::render_scale(&s),
        mean,
    )
}

#[test]
fn a_rung_changes_the_picture_and_never_the_world_hash() {
    let Some(gpu) = gpu() else { return };
    let (high_hash, high_tick, high_row, high_scale, high_mean) = run(&gpu, 0, 0);
    let (low_hash, low_tick, low_row, low_scale, low_mean) = run(&gpu, 1, 0);
    let (_, _, kept_row, _, kept_mean) = run(&gpu, 1, 1);
    assert_eq!((high_tick, low_tick), (600, 600));
    assert_eq!(
        high_hash, low_hash,
        "the world hash is the same on every rung"
    );
    assert_eq!(high_row["rung"], "high");
    assert_eq!(high_row["samples"], 4);
    assert_eq!(low_row["rung"], "low");
    assert_eq!(low_row["samples"], 1);
    assert_eq!(kept_row["rung"], "low");
    assert_eq!((high_scale, low_scale), (1.0, 0.5));
    // Low drops the sun's shadow the game left unsaid, so the frame is lighter;
    // the game's own `Shadows` stands, and the frame darkens with it again.
    assert!(
        low_mean > high_mean + 1.0,
        "low {low_mean} high {high_mean}"
    );
    assert!(
        (kept_mean - high_mean).abs() < (low_mean - high_mean) / 4.,
        "kept {kept_mean} high {high_mean} low {low_mean}"
    );
}
