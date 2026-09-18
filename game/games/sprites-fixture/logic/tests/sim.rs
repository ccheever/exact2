use exact_game::*;
use sprites_fixture_logic::SmallGame;

fn atlas() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../art/strip.png");
    bin::to_vec(&exact_game_bake::sprite(path).unwrap())
}
fn sim() -> Sim<SmallGame> {
    let mut s = Sim::new(()).unwrap();
    s.load_assets(|_| Ok::<_, String>(atlas())).unwrap();
    s.viewport(1280., 720.);
    s
}
#[test]
fn atlas_asset() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/strip.tex");
    assert_eq!(std::fs::read(path).unwrap(), atlas());
}
#[test]
fn tick300_and_restore() {
    let mut a = sim();
    a.hold("KeyD", 2500.);
    let save = a.save().unwrap();
    a.run(2500.);
    println!("sprites tick300 0x{:016x}", a.world().hash());
    exact_game::World::assert_pin(
        include_str!("../../pins.json"),
        "sprites-fixture",
        300,
        a.world().hash(),
    );
    assert_eq!(a.world().tick(), 300);
    assert_eq!(a.get::<Emitter>("leaves").unwrap().state.alive, 200);
    let mut b = sim();
    b.restore(&save).unwrap();
    b.run(2500.);
    assert_eq!(a.save().unwrap(), b.save().unwrap());
    let emitter = a.get::<Emitter>("leaves").unwrap();
    let player = a.global_position("player").unwrap();
    let origin = a.global_position("leaves").unwrap();
    let mut samples = vec![];
    emitter.particles(60, 1., |p| {
        let p = p.position + origin - player;
        if p.x.abs() < 6. && p.y.abs() < 5. {
            samples.push([p.x, p.y, p.z]);
        }
    });
    println!("leaf overlap samples {:?}", samples);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/leaf-pixels.json");
    if std::env::var_os("EXACT_FIXTURE_BAKE").is_some() {
        std::fs::write(&path, json::to_string(&samples).unwrap()).unwrap();
    }
    assert!(samples.iter().any(|s| s[2] < 0.) && samples.iter().any(|s| s[2] > 0.));
    let rect = a.layout("player").unwrap().screen;
    assert_eq!(rect.w, 96.);
    assert_eq!(rect.h, 128.);
    assert_eq!(
        a.pick(rect.center()).unwrap().entity,
        a.world().named("player").unwrap()
    );
}
#[test]
fn orthographic_integer_scale() {
    let camera = Camera::orthographic(180.).integer_scale();
    for (h, ratio) in [(720., 4.), (800., 4.), (90., 1.)] {
        let p = camera.matrix(Vec2::new(h * 16. / 9., h));
        assert!((p.y_axis.y * h * 0.5 - ratio).abs() < 1e-5);
    }
}
#[test]
fn primitive_refuses_sprite_by_name() {
    use exact_game_render::{exact_gpu::Surface, WorldSurface};
    struct SpriteOnly;
    impl Game for SpriteOnly {
        const ID: &'static str = "sprite-only";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn((
                Transform::default(),
                Sprite::new("undeclared.tex", [1., 1.]),
            ));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let error = WorldSurface::<SpriteOnly>::default()
        .bind(&[], None)
        .unwrap_err();
    assert!(error.0.contains("Sprite `undeclared.tex`"));
}
#[test]
fn rendered_atlas_and_mid_fall_restore() {
    use exact_game_render::{
        exact_gpu::{fixture, wgpu, Frame, Restore, Surface},
        WorldSurface,
    };
    let gpu = fixture::device().unwrap();
    let mut s = WorldSurface::<SmallGame, (), true>::default();
    s.device_ready();
    s.bind(&[], None).unwrap();
    for _ in 0..4 {
        for n in s.assets() {
            s.asset(&n, Ok(&atlas()));
        }
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    }
    let mut f = Frame {
        width: 1280.,
        height: 720.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let first = fixture::render(&gpu, &mut s, &f).unwrap().0;
    f.now_ms = 2500.;
    let falling = fixture::render(&gpu, &mut s, &f).unwrap().0;
    assert_ne!(first, falling);
    falling.save("sprites-mid-fall");
    let save = s.carry().unwrap();
    for mode in [Restore::Open, Restore::Carry] {
        s.restore(&save, mode).unwrap();
        f.now_ms = 0.;
        let restored = fixture::render(&gpu, &mut s, &f).unwrap().0;
        let changed = falling
            .data
            .iter()
            .zip(&restored.data)
            .filter(|(a, b)| a != b)
            .count();
        println!("restore/carry changed channels: {changed}");
        restored.save("sprites-restored");
        assert!(
            falling == restored,
            "restore/carry pixels differ in {changed} channels"
        );
    }
    let mut end = sim();
    end.hold("KeyD", 2500.);
    end.run(2500.);
    s.restore(&end.save().unwrap(), Restore::Open).unwrap();
    f.now_ms = 0.;
    let pixels = fixture::render(&gpu, &mut s, &f).unwrap().0;
    let rect = s.sim().unwrap().layout("player").unwrap().screen;
    let samples: Vec<[f32; 3]> = json::from_str(include_str!("leaf-pixels.json")).unwrap();
    for [x, y, z] in samples {
        let px = (rect.x + rect.w / 2. + x * rect.w / 24.).round() as u32;
        let py = (rect.y + rect.h / 2. - y * rect.h / 32.).round() as u32;
        let [r, _, b, _] = pixels.at(px, py);
        assert!(
            if z > 0. { r > b } else { b > 100 && r < 80 },
            "leaf {x},{y},{z}: {r},{b}"
        );
    }
    assert!(s.take_error().is_none());
}
