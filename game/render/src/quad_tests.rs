use crate::{
    exact_gpu::{fixture, wgpu, Frame, Surface},
    WorldSurface,
};
use exact_game::{
    asset::{AlphaMode, Filter, TextureData},
    *,
};
struct Layers;
impl Game for Layers {
    const ID: &'static str = "quad-order";
    const ASSETS: &'static [&'static str] = &["white.tex"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            ..Environment::default()
        });
        for (i, (color, layer)) in [
            ([0., 0., 1., 0.5], 1),
            ([1., 0., 0., 0.5], 0),
            ([0., 1., 0., 0.5], 1),
        ]
        .into_iter()
        .enumerate()
        {
            w.spawn((
                Transform::default().with_scale(if i == 0 { [-1., 1., 1.] } else { [1.; 3] }),
                Sprite {
                    color,
                    layer,
                    ..Sprite::new("white.tex", [2., 2.])
                },
            ));
        }
        for (x, cutoff) in [(-3., 0.75), (3., 0.25)] {
            w.spawn((
                Transform::at(x, 0., 0.),
                Sprite {
                    color: [1., 1., 1., 0.5],
                    alpha: AlphaMode::Mask,
                    cutoff,
                    ..Sprite::new("white.tex", [2., 2.])
                },
            ));
        }
        w.spawn((
            Transform::at(0., 0., 10.),
            Camera::orthographic(10.).integer_scale(),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn equal_depth_uses_layer_then_slot_and_mask_respects_cutoff_with_signed_scale() {
    let gpu = fixture::device().unwrap();
    let data = bin::to_vec(&TextureData {
        width: 1,
        height: 1,
        mips: vec![vec![255; 4]],
        filter: [Filter::Nearest; 3],
        ..TextureData::default()
    });
    let mut s = WorldSurface::<Layers, (), true>::default();
    s.device_ready();
    s.bind(&[], None).unwrap();
    for _ in 0..4 {
        for name in s.assets() {
            s.asset(&name, Ok(&data));
        }
        s.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    }
    let frame = Frame {
        width: 100.,
        height: 100.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let pixels = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    let [r, g, b, _] = pixels.at(50, 50);
    assert!(
        g > b && b > r,
        "layer/slot order must be red, blue, green: {r},{g},{b}"
    );
    assert_eq!(pixels.at(20, 50), [0, 0, 0, 255]);
    assert!(pixels.at(80, 50)[0] > 200);
    let frame = Frame {
        height: 35.,
        scale: 2.,
        ..frame
    };
    let pixels = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    let entity = s
        .sim()
        .unwrap()
        .world()
        .query::<&Sprite>()
        .iter()
        .next()
        .unwrap()
        .0;
    let layout = s.sim().unwrap().layout(entity).unwrap().screen;
    assert!((layout.w - 6.).abs() < 1e-5);
    let colored = (0..pixels.width)
        .filter(|&x| {
            let [r, g, b, _] = pixels.at(x, 35);
            g > b && b > r
        })
        .count();
    assert_eq!(
        colored, 12,
        "CSS layout width × display scale must match the quad"
    );
}

struct Cosmetic;
impl Game for Cosmetic {
    const ID: &'static str = "cosmetic-sprite";
    type Args = ();
    fn setup(w: &mut World, args: &()) {
        Layers::setup(w, args);
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn retired_sprite_waits_for_redelivery_and_reuses_identical_texture() {
    let gpu = fixture::device().unwrap();
    let data = bin::to_vec(&TextureData {
        width: 1,
        height: 1,
        mips: vec![vec![255; 4]],
        ..TextureData::default()
    });
    let mut s = WorldSurface::<Cosmetic, (), true>::default();
    s.device_ready();
    s.bind(&[], None).unwrap();
    assert_eq!(s.assets(), ["white.tex"]);
    s.asset("white.tex", Ok(&data));
    let frame = Frame {
        width: 100.,
        height: 100.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let before = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    for name in ["away.tex", "white.tex"] {
        for (_, sprite) in s.sim().unwrap().world().query::<&mut Sprite>().iter() {
            sprite.texture = name.into();
        }
        assert_eq!(s.assets(), [name]);
        s.retired_assets();
    }
    let pending = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    assert_eq!(
        pending.at(50, 50),
        [0, 0, 0, 255],
        "retired texture must not draw"
    );
    s.asset("white.tex", Ok(&data));
    let after = fixture::render(&gpu, &mut s, &frame).unwrap().0;
    assert!(
        before == after,
        "identical redelivery must restore the sprite"
    );
    let state = s.agent(r#"{"op":"state"}"#).unwrap();
    assert!(
        state.contains(r#""beforeReady":{"textureUploads":1"#),
        "{state}"
    );
    assert!(
        state.contains(r#""afterReady":{"textureUploads":0"#),
        "{state}"
    );
}
