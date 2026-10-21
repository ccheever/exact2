//! A look frame judges the frame the live renderer draws (LLP 1127.000 D1, D3,
//! D4): the same bytes on the direct and HDR paths with particles counted, and
//! bounds and levels of detail from where the frame drew each entity.
use crate::WorldSurface;
use exact_game::*;
use exact_gpu::{fixture, wgpu, Frame, Gpu, Surface};

fn frame(now_ms: f64) -> Frame {
    Frame {
        width: 320.,
        height: 180.,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.,
    }
}

/// Black particles at half alpha over a panel of radiance 3 (README "Direct
/// frames"): a direct frame clamps the panel to the target before they blend
/// (1 × 0.5, which sRGB encodes as 188), the HDR path after (3 × 0.5, 255).
struct Radiance<const FOUR: bool>;
impl<const FOUR: bool> Game for Radiance<FOUR> {
    const ID: &'static str = "look-radiance";
    const SAMPLES: SceneSamples = if FOUR {
        SceneSamples::Four
    } else {
        SceneSamples::One
    };
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.; 3].into()),
            ambient: 0.,
            fog: None,
            bloom: None,
            tone_mapping: ToneMapping::None,
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., 4.), Camera::default()));
        w.spawn((
            Transform::default(),
            Mesh::cuboid(Vec3::new(3., 3., 0.1)),
            Material {
                emissive: [3.; 3],
                ..Material::rgb(0., 0., 0.)
            },
        ));
        let shade = Emitter {
            rate: 0.,
            lifetime: 30.,
            speed: 0.,
            spread: 0.,
            gravity: Vec3::ZERO,
            size: [1., 1.],
            color: [[0., 0., 0., 0.5]; 2],
            additive: false,
            ..Default::default()
        };
        w.spawn((Transform::at(0., 0., 1.), shade.burst(1)));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        emitter::step(w);
    }
}

/// The live frame into a `format` target, then a camera look frame: the live
/// pixels, the look's reply, and how many of the look's frames drew direct.
fn live_and_look<const FOUR: bool>(
    gpu: &Gpu,
    format: wgpu::TextureFormat,
) -> (fixture::Pixels, serde_json::Value, u64) {
    let mut s = WorldSurface::<Radiance<FOUR>>::default();
    s.bind(&[], None).unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("live frame"),
        size: wgpu::Extent3d {
            width: 320,
            height: 180,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    for now_ms in [0., 100., 200.] {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        s.render(
            &frame(now_ms),
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &view,
            format,
        );
        gpu.queue.submit([encoder.finish()]);
        s.submitted();
        assert!(s.error().is_none(), "{:?}", s.error());
        // Direct at four samples too, as phones draw: the format and the tone
        // curve decide.
        s.renderer_for_test().unwrap().set_direct(true);
    }
    let live = fixture::read(gpu, &texture).unwrap();
    let before = s.renderer_for_test().unwrap().direct_frames;
    let reply = s.agent(r#"{"op":"screenshot","form":"look"}"#).unwrap();
    let direct = s.renderer_for_test().unwrap().direct_frames - before;
    (live, serde_json::from_str(&reply).unwrap(), direct)
}

#[test]
fn a_look_frame_draws_the_live_frames_bytes_on_the_direct_and_hdr_paths() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    for (format, direct, centre) in [
        (wgpu::TextureFormat::Rgba8UnormSrgb, 1, 188),
        // A target the shader encodes would blend encoded values: HDR.
        (wgpu::TextureFormat::Rgba8Unorm, 0, 255),
    ] {
        for (samples, (live, look, drew)) in [
            (1, live_and_look::<false>(&gpu, format)),
            (4, live_and_look::<true>(&gpu, format)),
        ] {
            let at = live.at(160, 90);
            let report = &look["look"];
            assert!(
                at[..3].iter().all(|c| c.abs_diff(centre) <= 2),
                "{format:?} at {samples}x: the shaded panel is {at:?}: {report}"
            );
            assert!(
                look["data"].as_str() == Some(crate::look::base64(&live.data).as_str()),
                "{format:?} at {samples}x: the look frame's bytes differ from the live frame's: {report}"
            );
            assert_eq!(drew, direct, "{format:?} at {samples}x: {report}");
            assert!(
                report["particles"][0]["covered"].as_u64() > Some(0),
                "{report}"
            );
        }
    }
}

/// Something that moves 100 m along x every tick, away from the camera: a
/// `blend`ed model (the CPU's ordered route) that its level of detail hides
/// beyond 200 m, else a cube under a direct cull; or with `attach`, a rig whose
/// joint stands 20 m along x and a model held at that joint while its own
/// Transform says 400 m away.
#[derive(Default, Args)]
struct PoseArgs {
    blend: bool,
    attach: bool,
}
struct Poses;
impl Game for Poses {
    const ID: &'static str = "look-poses";
    const ASSETS: &'static [&'static str] = &["deformed.model", "crate.model"];
    type Args = PoseArgs;
    fn setup(w: &mut World, args: &PoseArgs) {
        w.spawn_named(
            "camera",
            (
                Transform::at(-20., 5., 0.).looking_at(Vec3::new(200., 0., 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        if !args.attach {
            if !args.blend {
                w.spawn_named("mover", (Transform::default(), Mesh::cube(1.)));
                return;
            }
            let lod = ModelLod {
                levels: Vec::new(),
                hide: Some(200.),
            };
            w.spawn_named(
                "mover",
                (Transform::default(), Mesh::asset("crate.model"), lod),
            );
            return;
        }
        let rig = w.spawn_named("rig", (Transform::default(), Mesh::asset("deformed.model")));
        w.spawn_named(
            "held",
            (
                Transform::at(0., 0., -400.),
                Mesh::asset("crate.model"),
                ModelLod {
                    levels: Vec::new(),
                    hide: Some(150.),
                },
                SocketFollow::new(rig, "joint"),
            ),
        );
    }
    fn tick(w: &mut World, _: &Input, args: &PoseArgs) {
        if !args.attach {
            w.require_mut::<Transform>("mover").position.x += 100.;
        }
    }
}

fn poses(
    gpu: &Gpu,
    args: PoseArgs,
    direct: bool,
) -> WorldSurface<Poses, crate::ModelExecutor, true> {
    let mut rig = crate::test_model::skinned_model();
    let joint = rig.skins[0].joints[0] as usize;
    rig.nodes[joint].transform = Mat4::from_translation(Vec3::new(20., 0., 0.)).to_cols_array();
    rig.materials.fill(Default::default());
    rig.textures.clear();
    let mut held: asset::Model =
        bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model")).unwrap();
    held.materials.fill(Default::default());
    if args.blend {
        held.materials[0].alpha_mode = asset::AlphaMode::Blend;
    }
    held.textures.clear();
    let mut s = WorldSurface::<Poses, crate::ModelExecutor, true>::default();
    s.device_ready(wgpu::Features::empty());
    s.bind(&args.values(), None).unwrap();
    s.asset("deformed.model", Ok(&exact_game::asset::encode_model(&rig)));
    s.asset("crate.model", Ok(&exact_game::asset::encode_model(&held)));
    fixture::render(gpu, &mut s, &frame(0.)).unwrap();
    if direct {
        // As a device without indirect draws culls: every group drawn whole.
        let r = s.renderer_for_test().unwrap();
        r.cull.indirect_execution = false;
        r.cull.epoch += 1;
    }
    // Two ticks and a fraction of the next.
    for ms in [20., 41.] {
        fixture::render(gpu, &mut s, &frame(ms)).unwrap();
    }
    assert!(s.error().is_none(), "{:?}", s.error());
    assert_eq!(s.renderer_for_test().unwrap().cull.direct, direct);
    s
}

fn plan_bounds(s: &mut impl Surface) -> serde_json::Value {
    let reply = s
        .agent(r#"{"op":"screenshot","form":"look","top":30,"width":320,"height":180}"#)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    value["bounds"].clone()
}

#[test]
fn bounds_follow_a_moving_model_where_the_frame_drew_it() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    for (blend, direct) in [(true, false), (false, true)] {
        let args = PoseArgs {
            blend,
            attach: false,
        };
        let mut s = poses(&gpu, args, direct);
        let (now, alpha) = {
            let sim = s.sim().unwrap();
            let (w, alpha) = (sim.world(), sim.alpha());
            let mover = w.resolve("mover").unwrap();
            (w.get::<Transform>(mover).unwrap().position.x, alpha)
        };
        // Drawn between the last two ticks: 100 m behind the current row at alpha 0.
        let drawn = now - 100. * (1. - alpha);
        assert!(
            now - drawn > 10.,
            "alpha {alpha}: no fraction to tell apart"
        );
        let bounds = plan_bounds(&mut s);
        let x = bounds["centre"][0].as_f64().unwrap() as f32;
        assert!(
            (x - drawn).abs() < 2.,
            "blend {blend}, direct {direct}: bounds at {x}, drawn at {drawn}, current {now}: {bounds}"
        );
        if blend {
            // Drawn within its hide distance (and the selection's 5% band), though
            // its current row is beyond it: the frame's level selection says drawn.
            let eye = Vec3::new(-20., 5., 0.);
            let far = |x: f32| eye.distance(Vec3::new(x, 0., 0.));
            assert!(far(drawn) < 190. && far(now) > 200., "{drawn} {now}");
            let reply = s
                .agent(r#"{"op":"state","entity":"mover","visible":true,"width":320,"height":180}"#)
                .unwrap();
            let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(value["visible"], "drawn", "{reply}");
        }
    }
}

#[test]
fn an_attached_model_is_bounded_and_levelled_where_its_joint_holds_it() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    for direct in [false, true] {
        let args = PoseArgs {
            blend: false,
            attach: true,
        };
        let mut s = poses(&gpu, args, direct);
        let bounds = plan_bounds(&mut s);
        let centre = &bounds["centre"];
        let z = centre[2].as_f64().unwrap();
        assert!(
            z.abs() < 5. && (centre[0].as_f64().unwrap() - 20.).abs() < 5.,
            "direct {direct}: {bounds}"
        );
        // Its level is chosen at the joint, 40 m from the camera, not 400 m off.
        let reply = s
            .agent(r#"{"op":"state","entity":"held","visible":true,"width":320,"height":180}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(value["visible"], "drawn", "direct {direct}: {reply}");
    }
}

/// A model whose level of detail hides it beyond 100 m, 99 m from the game's
/// camera: within the selection's 5% band, where hysteresis keeps whatever
/// level it drew last; or, `far`, 120 m from it and hidden.
#[derive(Default, Args)]
struct BandArgs {
    far: bool,
}
struct Band;
impl Game for Band {
    const ID: &'static str = "look-lod-band";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = BandArgs;
    fn setup(w: &mut World, args: &BandArgs) {
        let z = if args.far { 120. } else { 99. };
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 0., z).looking_at(Vec3::ZERO, Vec3::Y),
                Camera::default(),
            ),
        );
        let lod = ModelLod {
            levels: Vec::new(),
            hide: Some(100.),
        };
        let at = Transform::default().with_scale(10.);
        w.spawn_named("crate", (at, Mesh::asset("crate.model"), lod));
    }
    fn tick(_: &mut World, _: &Input, _: &BandArgs) {}
}

type BandSurface = WorldSurface<Band, crate::ModelExecutor, true>;

/// A `Band` surface after three frames; `direct`, drawn as a device without
/// indirect draws draws: every group whole, with no per-instance cull.
fn band(gpu: &Gpu, far: bool, direct: bool) -> BandSurface {
    let mut model: asset::Model =
        bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model")).unwrap();
    model.materials.fill(Default::default());
    model.textures.clear();
    let mut s = BandSurface::default();
    s.device_ready(wgpu::Features::empty());
    s.bind(&BandArgs { far }.values(), None).unwrap();
    s.asset("crate.model", Ok(&exact_game::asset::encode_model(&model)));
    fixture::render(gpu, &mut s, &frame(0.)).unwrap();
    if direct {
        let r = s.renderer_for_test().unwrap();
        r.cull.indirect_execution = false;
        r.cull.epoch += 1;
    }
    for ms in [20., 40.] {
        fixture::render(gpu, &mut s, &frame(ms)).unwrap();
    }
    assert!(s.error().is_none(), "{:?}", s.error());
    assert_eq!(s.renderer_for_test().unwrap().cull.direct, direct);
    s
}

fn crate_visible(s: &mut BandSurface) -> String {
    let reply = s
        .agent(r#"{"op":"state","entity":"crate","visible":true,"width":320,"height":180}"#)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    value["visible"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_owned()
}

#[test]
fn a_direct_frame_draws_the_finest_level_beyond_its_hide_distance_and_says_so() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    // The GPU cull hides it beyond 100 m; a direct frame draws level 0 whole.
    let mut culled = band(&gpu, true, false);
    let (hidden, _) = fixture::render(&gpu, &mut culled, &frame(60.)).unwrap();
    assert_eq!(crate_visible(&mut culled), "lod-hidden 120 m");
    let mut direct = band(&gpu, true, true);
    let (drawn, _) = fixture::render(&gpu, &mut direct, &frame(60.)).unwrap();
    let shown = (drawn.data.chunks(4).zip(hidden.data.chunks(4))).filter(|(a, b)| a != b);
    assert!(shown.count() > 50, "the direct frame draws the model");
    assert_eq!(crate_visible(&mut direct), "drawn");
}

#[test]
fn an_inspection_from_beyond_a_hide_distance_leaves_the_live_levels_as_they_were() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = band(&gpu, false, false);
    let (before, _) = fixture::render(&gpu, &mut s, &frame(60.)).unwrap();
    assert_eq!(crate_visible(&mut s), "drawn");
    // From 110 m the inspection's own frame hides it.
    let orbit = s
        .agent(r#"{"op":"screenshot","form":"look","yaw":0,"pitch":0,"distance":110,"width":320,"height":180}"#)
        .unwrap();
    assert!(orbit.contains("\"data\""), "{orbit}");
    let (after, _) = fixture::render(&gpu, &mut s, &frame(80.)).unwrap();
    let changed = (after.data.chunks(4).zip(before.data.chunks(4))).filter(|(a, b)| a != b);
    assert_eq!(changed.count(), 0, "pixels the next live frame changed");
    assert_eq!(crate_visible(&mut s), "drawn");
    let camera = s
        .agent(r#"{"op":"screenshot","form":"look","width":320,"height":180}"#)
        .unwrap();
    let camera: serde_json::Value = serde_json::from_str(&camera).unwrap();
    assert_eq!(camera["data"], crate::look::base64(&before.data));
}

/// One entity with up to three drawables: `mesh`, a model whose opaque square
/// is 200 m to the side, out of view, and whose half-transparent square is
/// delivered in front of the camera or 200 m to the other side; `sprite`, a
/// white 2 m sprite in front of the camera; `emitter`, one with no live particle.
#[derive(Default, Args)]
struct MixedArgs {
    mesh: bool,
    sprite: bool,
    emitter: bool,
}
struct Mixed;
impl Game for Mixed {
    const ID: &'static str = "look-mixed";
    const ASSETS: &'static [&'static str] = &["mixed.model", "white.tex"];
    type Args = MixedArgs;
    fn setup(w: &mut World, args: &MixedArgs) {
        w.insert_resource(Environment {
            background: Some([0.; 3].into()),
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 0., 5.).looking_at(Vec3::ZERO, Vec3::Y),
                Camera::default(),
            ),
        );
        let e = w.spawn_named("mixed", (Transform::default(),));
        if args.mesh {
            w.insert(e, Mesh::asset("mixed.model"));
        }
        if args.sprite {
            w.insert(e, Sprite::new("white.tex", [2., 2.]));
        }
        if args.emitter {
            let empty = Emitter {
                rate: 0.,
                lifetime: 1.,
                ..Default::default()
            };
            w.insert(e, empty);
        }
    }
    fn tick(w: &mut World, _: &Input, _: &MixedArgs) {
        emitter::step(w);
    }
}

/// A 2 m square facing the camera, opaque at 200 m along x and blended at
/// `glass` along x.
fn mixed_model(glass: f32) -> asset::Model {
    use asset::{AlphaMode, MaterialData, MeshData, Model, Node};
    let square = |material| MeshData {
        positions: vec![-1., -1., 0., 1., -1., 0., 1., 1., 0., -1., 1., 0.],
        normals: [0., 0., 1.].repeat(4),
        uvs: [0.; 2].repeat(4),
        indices: vec![0, 1, 2, 0, 2, 3],
        bounds: [-1., -1., 0., 1., 1., 0.],
        material,
        ..Default::default()
    };
    let at = |x: f32| Mat4::from_translation(Vec3::new(x, 0., 0.)).to_cols_array();
    Model {
        meshes: vec![square(0), square(1)],
        materials: vec![
            MaterialData::default(),
            MaterialData {
                base_color: [1., 1., 1., 0.5],
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            },
        ],
        nodes: vec![
            Node {
                name: "opaque".into(),
                mesh: Some(0),
                transform: at(200.),
                ..Default::default()
            },
            Node {
                name: "glass".into(),
                mesh: Some(1),
                transform: at(glass),
                ..Default::default()
            },
        ],
        bounds: [-201., -1., 0., 201., 1., 0.],
        ..Default::default()
    }
}

type MixedSurface = WorldSurface<Mixed, crate::ModelExecutor, true>;

/// A `Mixed` surface after three frames, on the path the adapter takes: the
/// last frame's pixels lit, `visible`'s word for the entity, and whether the
/// frame drew direct (an adapter without indirect draws, as the iOS
/// simulator's Metal, draws every group whole, with no per-instance cull).
fn mixed(gpu: &Gpu, args: MixedArgs, glass: f32) -> (usize, String, bool) {
    let mut s = MixedSurface::default();
    s.device_ready(wgpu::Features::empty());
    s.bind(&args.values(), None).unwrap();
    s.asset(
        "mixed.model",
        Ok(&exact_game::asset::encode_model(&mixed_model(glass))),
    );
    let white = asset::TextureData {
        width: 1,
        height: 1,
        mips: vec![vec![255; 4]],
        ..Default::default()
    };
    s.asset("white.tex", Ok(&bin::to_vec(&white)));
    let mut pixels = None;
    for ms in [0., 20., 40.] {
        pixels = Some(fixture::render(gpu, &mut s, &frame(ms)).unwrap().0);
    }
    assert!(s.error().is_none(), "{:?}", s.error());
    // The probed capability decides the path; these scenes fit every limit.
    let cull = &s.renderer_for_test().unwrap().cull;
    let direct = cull.direct;
    assert_eq!(direct, !cull.indirect_execution);
    let pixels = pixels.unwrap();
    let lit = (pixels.data.chunks(4)).filter(|p| p[..3].iter().any(|&c| c > 10));
    let reply = s
        .agent(r#"{"op":"state","entity":"mixed","visible":true,"width":320,"height":180}"#)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    let word = value["visible"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"));
    (lit.count(), word.to_owned(), direct)
}

#[test]
fn an_entity_whose_blended_part_draws_is_drawn_though_its_opaque_part_is_out_of_view() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let model = || MixedArgs {
        mesh: true,
        ..Default::default()
    };
    let (lit, word, _) = mixed(&gpu, model(), 0.);
    assert!(lit > 1000, "{lit} pixels lit");
    assert_eq!(word, "drawn");
    // Nothing in view: no pixel lit. The GPU cull drops every part; a direct
    // frame submits its groups whole, unculled.
    let (lit, word, direct) = mixed(&gpu, model(), -200.);
    assert_eq!(lit, 0);
    if direct {
        eprintln!("skipped the GPU cull's `frustum`: this adapter has no indirect draws");
        assert_eq!(word, "drawn");
    } else {
        assert_eq!(word, "frustum");
    }
}

#[test]
fn an_entity_is_drawn_when_its_sprite_draws_whatever_its_emitter_or_mesh_did() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    // An emitter with no live particle beside a sprite in view.
    let args = MixedArgs {
        sprite: true,
        emitter: true,
        ..Default::default()
    };
    let (lit, word, _) = mixed(&gpu, args, 0.);
    assert!(lit > 1000, "{lit} pixels lit");
    assert_eq!(word, "drawn");
    // A model out of view (both its parts) beside a sprite in view.
    let args = MixedArgs {
        mesh: true,
        sprite: true,
        ..Default::default()
    };
    let (lit, word, _) = mixed(&gpu, args, -200.);
    assert!(lit > 1000, "{lit} pixels lit");
    assert_eq!(word, "drawn");
}

#[derive(Default, Args)]
struct ExplosionArgs {
    sphere: bool,
    above: bool,
    particle: bool,
}
struct Explosion;
impl Game for Explosion {
    const ID: &'static str = "look-explosion";
    type Args = ExplosionArgs;
    fn setup(w: &mut World, args: &ExplosionArgs) {
        w.insert_resource(Environment {
            background: Some(Color::hex("#808080")),
            bloom: None,
            fog: None,
            tone_mapping: ToneMapping::None,
            ..Default::default()
        });
        w.spawn((
            Transform::at(0., 3., 6.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
        ));
        w.spawn((
            Transform::default(),
            Mesh::plane(30., 30.),
            Material::rgb(0.3, 0.3, 0.3),
        ));
        if args.sphere {
            // Rivals' explosion: an alpha-blended sphere, drawn as 528 triangles.
            w.spawn((
                Transform::at(1., if args.above { 1. } else { -2. }, 0.).with_scale(0.4),
                Mesh::sphere(1.),
                Material {
                    color: [1., 0.6, 0.2, 0.9],
                    ..Material::glow([6., 2.4, 0.5])
                },
            ));
        }
        if args.particle {
            w.spawn((
                Transform::at(-1., -2., 0.),
                Emitter {
                    speed: 0.,
                    gravity: Vec3::ZERO,
                    size: [0.4; 2],
                    color: [[1., 0.6, 0.2, 0.9]; 2],
                    additive: false,
                    ..Emitter::sparks().rate(0.).lifetime(30.).burst(1)
                },
            ));
        }
    }
    fn tick(w: &mut World, _: &Input, _: &ExplosionArgs) {
        emitter::step(w);
    }
}

fn explosion_look(
    gpu: &Gpu,
    direct: Option<bool>,
    args: ExplosionArgs,
) -> (serde_json::Value, serde_json::Value) {
    let mut s = WorldSurface::<Explosion>::default();
    s.bind(&args.values(), None).unwrap();
    if let Some(direct) = direct {
        let format = if direct {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("explosion control"),
            size: wgpu::Extent3d {
                width: 320,
                height: 180,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        for now in [0., 100.] {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            s.render(
                &frame(now),
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &view,
                format,
            );
            gpu.queue.submit([encoder.finish()]);
            s.submitted();
            assert!(s.error().is_none(), "{:?}", s.error());
            s.renderer_for_test().unwrap().set_direct(direct);
        }
    } else {
        for now in [0, 100] {
            s.agent(&format!(r#"{{"op":"clock","now":{now}}}"#))
                .unwrap();
        }
    }
    let saved = s.carry().unwrap().unwrap();
    let picture: serde_json::Value = serde_json::from_str(
        &s.agent(r#"{"op":"screenshot","form":"look","width":320,"height":180}"#)
            .unwrap(),
    )
    .unwrap();
    let draws: serde_json::Value =
        serde_json::from_str(&s.agent(r#"{"op":"state","draws":true}"#).unwrap()).unwrap();
    assert_eq!(s.carry().unwrap().unwrap(), saved);
    (picture, draws)
}

#[test]
fn blended_explosion_triangles_are_not_particles_or_a_cover_for_them() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    for direct in [None, Some(false), Some(true)] {
        let (bare, _) = explosion_look(&gpu, direct, ExplosionArgs::default());
        for above in [false, true] {
            for particle in [false, true] {
                let (picture, listing) = explosion_look(
                    &gpu,
                    direct,
                    ExplosionArgs {
                        sphere: true,
                        above,
                        particle,
                    },
                );
                let look = &picture["look"];
                assert_eq!(look["gaps"], serde_json::json!([]), "{direct:?}: {look}");
                let tells = look["tells"].as_array().unwrap();
                if particle {
                    assert_eq!(tells.len(), 1, "{look}");
                    assert_eq!(tells[0]["name"], "particles-uncovered");
                    assert_eq!(
                        look["particles"],
                        serde_json::json!([{"batch":"particle dots · alpha", "live":1, "covered":0}])
                    );
                } else {
                    assert!(tells.is_empty(), "{look}");
                    assert!(look["particles"].is_null(), "{look}");
                }
                // Removing the hidden sphere changes no pixel; the visible one contributes.
                assert_eq!(
                    picture["data"] == bare["data"],
                    !above,
                    "{direct:?}, above={above}"
                );
                let primitives: Vec<_> = listing["draws"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|d| d["draw"] == "blended primitives")
                    .collect();
                assert_eq!(
                    primitives
                        .iter()
                        .map(|d| d["submitted"].as_u64().unwrap())
                        .sum::<u64>(),
                    528,
                    "{listing}"
                );
                for draw in primitives {
                    assert_eq!(
                        draw["pass"],
                        if particle { "translucent" } else { "forward" }
                    );
                    assert_eq!(draw["variant"], if direct == Some(true) { 8 } else { 0 });
                    assert_eq!(draw["winding"], "ccw");
                    assert!(draw.get("fragments").is_none(), "{draw}");
                    assert_eq!(draw["covered"], above);
                }
            }
        }
    }
}
