//! LLP 1139.003 D3: a rung changes the picture and never the world.
use super::*;
use exact_game::{DirectionalLight, Material, Quality, Shadows};

#[derive(Default, exact_game::Args)]
struct LadderArgs {
    /// 1 asks for `Quality::Low`, 2 `Auto`, anything else `High`.
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
        match args.q {
            1 => Quality::Low,
            2 => Quality::Auto,
            _ => Quality::High,
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

fn bound(q: u32) -> WorldSurface<Ladder> {
    let mut s = WorldSurface::<Ladder>::default();
    s.bind(&[Value::Number(q.into()), Value::Number(0.)], None)
        .unwrap();
    s
}
fn state(s: &mut WorldSurface<Ladder>) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&s.agent(r#"{"op":"state"}"#).unwrap()).unwrap()
        ["world"]
        .clone()
}

#[test]
fn auto_takes_the_tier_when_the_canvas_binds() {
    for (tier, rung, scale) in [
        (exact_gpu::GpuTier::Low, "low", 0.5),
        (exact_gpu::GpuTier::Mid, "mid", 0.75),
        (exact_gpu::GpuTier::High, "high", 1.0),
    ] {
        exact_gpu::set_thread_tier(Some(tier));
        let mut s = bound(2);
        exact_gpu::set_thread_tier(None);
        let row = &state(&mut s)["quality"];
        assert_eq!(
            (row["asked"].as_str(), row["rung"].as_str()),
            (Some("auto"), Some(rung))
        );
        assert_eq!(exact_gpu::Surface::render_scale(&s), scale);
    }
}

struct FullSize;
impl Game for FullSize {
    const ID: &'static str = "quality-full-size";
    type Args = ();
    fn quality(_: &()) -> Quality {
        Quality::Low
    }
    fn render_scale(_: &()) -> f32 {
        1.0
    }
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}

#[test]
fn a_render_scale_the_game_returns_stands_on_every_rung() {
    let mut s = WorldSurface::<FullSize>::default();
    s.bind(&[], None).unwrap();
    assert_eq!(exact_gpu::Surface::render_scale(&s), 1.0);
}

/// A look frame's size and mean, for one rung, at 64x64 points.
fn look(gpu: &Gpu, q: u32) -> (u64, f64) {
    let mut s = bound(q);
    for i in 0..=4 {
        fixture::render(gpu, &mut s, &frame(i as f64 * 500.)).unwrap();
    }
    let reply: serde_json::Value = serde_json::from_str(
        &s.agent(r#"{"op":"screenshot","form":"look","width":64,"height":64}"#)
            .unwrap(),
    )
    .unwrap();
    (
        reply["width"].as_u64().unwrap(),
        reply["look"]["mean"].as_f64().unwrap(),
    )
}

#[test]
fn a_look_frame_is_its_rungs_picture() {
    let Some(gpu) = gpu() else { return };
    let (high_width, high_mean) = look(&gpu, 0);
    let (low_width, low_mean) = look(&gpu, 1);
    assert_eq!((high_width, low_width), (64, 32), "the rung's scale");
    assert!(
        low_mean > high_mean,
        "no sun shadow under Low: low {low_mean} high {high_mean}"
    );
}

#[test]
fn a_new_tier_remakes_the_renderer_under_the_same_world() {
    let Some(gpu) = gpu() else { return };
    let run = |retier: bool| {
        exact_gpu::set_thread_tier(Some(exact_gpu::GpuTier::High));
        let mut s = bound(2);
        for i in 0..=5 {
            fixture::render(&gpu, &mut s, &frame(i as f64 * 500.)).unwrap();
        }
        if retier {
            exact_gpu::set_thread_tier(Some(exact_gpu::GpuTier::Low));
            assert!(
                exact_gpu::Surface::retier(&mut s),
                "a drawn world releases its renderer"
            );
        }
        for i in 6..=10 {
            fixture::render(&gpu, &mut s, &frame(i as f64 * 500.)).unwrap();
            assert!(s.error().is_none(), "{:?}", s.error());
        }
        exact_gpu::set_thread_tier(None);
        let world = state(&mut s);
        (
            world["tick"].as_u64(),
            world["hash"].as_str().map(str::to_owned),
            world["quality"]["rung"].as_str().map(str::to_owned),
        )
    };
    let (tick, hash, rung) = run(false);
    let (retick, rehash, rerung) = run(true);
    assert_eq!((tick, rung.as_deref()), (Some(600), Some("high")));
    assert_eq!((retick, rerung.as_deref()), (Some(600), Some("low")));
    assert_eq!(hash, rehash, "the world went on through the new renderer");
}
