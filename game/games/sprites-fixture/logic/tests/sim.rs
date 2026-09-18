use exact_game::asset::{Filter, TextureData, Wrap};
use exact_game::*;
use sprites_fixture_logic::SmallGame;

fn atlas() -> Vec<u8> {
    let mut pixels = vec![0; 64 * 16 * 4];
    for y in 0..16 {
        for x in 0..64 {
            let (tile, px) = (x / 16, x % 16);
            let c = if tile < 2 {
                if (4..12).contains(&px) && (1..6).contains(&y) {
                    [255, 206, 148, 255]
                } else if (2..14).contains(&px) && (6..13).contains(&y) {
                    [45, 95, 230, 255]
                } else if y >= 13
                    && ((px >= 3 && px < 6 + tile * 2) || (px >= 10 - tile * 2 && px < 13))
                {
                    [35, 48, 92, 255]
                } else {
                    [0, 0, 0, 0]
                }
            } else if tile == 2 {
                if y > (px / 3) % 5 {
                    [180, 210, 190, 255]
                } else {
                    [0, 0, 0, 0]
                }
            } else if y < 3 {
                [118, 161, 84, 255]
            } else if (x + y) % 7 == 0 {
                [104, 70, 46, 255]
            } else {
                [67, 53, 45, 255]
            };
            pixels[(y * 64 + x) * 4..(y * 64 + x + 1) * 4].copy_from_slice(&c);
        }
    }
    let mut mips = vec![pixels];
    let (mut w, mut h) = (64, 16);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                let from = ((y * 2).min(h - 1) * w + (x * 2).min(w - 1)) * 4;
                next[(y * nw + x) * 4..(y * nw + x + 1) * 4]
                    .copy_from_slice(&mips.last().unwrap()[from..from + 4]);
            }
        }
        mips.push(next);
        w = nw;
        h = nh;
    }
    bin::to_vec(&TextureData {
        width: 64,
        height: 16,
        mips,
        srgb: true,
        wrap: [Wrap::Clamp; 2],
        filter: [Filter::Nearest; 3],
    })
}
fn sim() -> Sim<SmallGame> {
    let mut s = Sim::new(()).unwrap();
    s.load_assets(|_| Ok::<_, String>(atlas())).unwrap();
    s.viewport(1280., 720.);
    s
}
#[test]
fn atlas_asset() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/strip.tex");
    if std::env::var_os("EXACT_FIXTURE_BAKE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, atlas()).unwrap();
    }
    assert_eq!(std::fs::read(path).unwrap(), atlas());
}
#[test]
fn tick300_and_restore() {
    let mut a = sim();
    a.hold("KeyD", 2500.);
    let save = a.save().unwrap();
    a.run(2500.);
    println!("sprites tick300 0x{:016x}", a.world().hash());
    assert_eq!(a.world().hash(), 0xf598d0032d70cce5);
    assert_eq!(a.world().tick(), 300);
    assert_eq!(a.get::<Emitter>("leaves").unwrap().state.alive, 200);
    let mut b = sim();
    b.restore(&save).unwrap();
    b.run(2500.);
    assert_eq!(a.save().unwrap(), b.save().unwrap());
    let emitter = a.get::<Emitter>("leaves").unwrap();
    let player = a.position("player").unwrap();
    let origin = a.position("leaves").unwrap();
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
    assert!(s.take_error().is_none());
}
