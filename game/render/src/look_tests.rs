//! `state world:<entity> visible` (LLP 1127.000 D4) on one scene with an entity
//! in each primitive state: drawn, frustum, occluded, hidden, hidden-ancestor,
//! opacity 0 and outside-cascade. Model states (asset-pending, lod-hidden) are
//! in `tests/visibility`, custom-material in `tests/hooks.rs`.
use crate::WorldSurface;
use exact_game::*;
use exact_gpu::{fixture, Frame, Surface};

#[derive(Default, Args)]
struct Scene {
    /// An entity at opacity 0, which turns occlusion off for the frame.
    fade: bool,
}
struct States;
impl Game for States {
    const ID: &'static str = "visible-states";
    type Args = Scene;
    fn setup(w: &mut World, args: &Scene) {
        w.insert_resource(Shadows {
            distance: 12.,
            ..Shadows::default()
        });
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 3., 10.).looking_at(Vec3::ZERO, Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn_named(
            "sun",
            (
                Transform::at(0., 10., 0.).looking_at(Vec3::new(0.2, 0., -0.3), Vec3::Z),
                DirectionalLight::default(),
            ),
        );
        let cube = |w: &mut World, name: &str, at: [f32; 3], size: Vec3| {
            w.spawn_named(
                name,
                (
                    Transform::at(at[0], at[1], at[2]),
                    Mesh::cuboid(size),
                    Material::hex("#c8b48c"),
                ),
            )
        };
        cube(w, "drawn", [0., 0.5, 0.], Vec3::ONE);
        cube(w, "behind", [0., 0.5, 20.], Vec3::ONE);
        // The wall stands between the camera and `walled`.
        cube(w, "wall", [4., 1.5, 4.], Vec3::new(4., 3., 0.5));
        cube(w, "walled", [7., 0.5, 0.], Vec3::splat(0.5));
        cube(w, "far", [-6., 0.5, -40.], Vec3::ONE);
        w.spawn_named(
            "lamp",
            (
                Transform::at(0., 5., 2.).looking_at(Vec3::ZERO, Vec3::Y),
                SpotLight::default(),
                LightShadows,
            ),
        );
        w.spawn_named(
            "sparks",
            (
                Transform::at(-1., 1.5, -3.),
                Emitter::sparks().rate(200.).lifetime(1.),
            ),
        );
        if args.fade {
            let hider = cube(w, "hider", [-3., 0.5, 0.], Vec3::ONE);
            w.insert(hider, Visible(false));
            let child = cube(w, "child", [0., 1., 0.], Vec3::splat(0.5));
            w.insert(child, Parent(hider));
            let own = cube(w, "own", [-2., 0.5, 2.], Vec3::ONE);
            w.insert(own, Visible(false));
            cube(w, "faded", [2., 0.5, 2.], Vec3::ONE);
        }
    }
    fn tick(w: &mut World, _: &Input, _: &Scene) {
        emitter::step(w);
    }
    fn present(p: &mut Present<'_>, _: &Scene) {
        if let Some(e) = p.named("faded") {
            p.insert(e, Opacity(0.));
        }
    }
}

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

fn word(s: &mut WorldSurface<States>, name: &str) -> String {
    let reply = s
        .agent(&format!(
            r#"{{"op":"state","entity":"{name}","visible":true,"width":320,"height":180}}"#
        ))
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    value["visible"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_owned()
}

#[test]
fn visible_answers_one_word_for_each_state() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut occluding = WorldSurface::<States>::default();
    occluding
        .bind(&Scene { fade: false }.values(), None)
        .unwrap();
    fixture::render(&gpu, &mut occluding, &frame(0.)).unwrap();
    occluding
        .renderer_for_test()
        .unwrap()
        .set_occlusion_items(1);
    for ms in [20., 40.] {
        fixture::render(&gpu, &mut occluding, &frame(ms)).unwrap();
    }
    assert_eq!(word(&mut occluding, "drawn"), "drawn");
    assert_eq!(word(&mut occluding, "wall"), "drawn");
    assert_eq!(word(&mut occluding, "behind"), "frustum");
    assert_eq!(word(&mut occluding, "walled"), "occluded");

    let mut states = WorldSurface::<States>::default();
    states.bind(&Scene { fade: true }.values(), None).unwrap();
    for ms in [0., 20.] {
        fixture::render(&gpu, &mut states, &frame(ms)).unwrap();
    }
    assert_eq!(word(&mut states, "drawn"), "drawn");
    assert_eq!(word(&mut states, "behind"), "frustum");
    assert_eq!(word(&mut states, "own"), "hidden");
    assert_eq!(word(&mut states, "child"), "hidden-ancestor hider");
    assert_eq!(word(&mut states, "faded"), "opacity 0");
    assert_eq!(word(&mut states, "far"), "outside-cascade 2");
    // Without occlusion this frame, the walled cube is drawn behind the wall.
    assert_eq!(word(&mut states, "walled"), "drawn");
    let refused = states
        .agent(r#"{"op":"state","entity":"nobody","visible":true}"#)
        .unwrap();
    assert!(refused.contains("no entity named `nobody`"), "{refused}");
}

#[test]
fn an_unarmed_frame_builds_no_probe_record() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = WorldSurface::<States>::default();
    s.bind(&Scene { fade: false }.values(), None).unwrap();
    // Groups, shadow cascades, particles, the sky and the tone pass, unarmed.
    for ms in [0., 20., 40., 60.] {
        fixture::render(&gpu, &mut s, &frame(ms)).unwrap();
    }
    let built = |s: &mut WorldSurface<States>| s.renderer_for_test().unwrap().probe_built.get();
    assert_eq!(built(&mut s), 0);
    assert_eq!(word(&mut s, "drawn"), "drawn");
    let armed = built(&mut s);
    assert!(armed > 0, "a look frame records its draws");
    fixture::render(&gpu, &mut s, &frame(80.)).unwrap();
    assert_eq!(built(&mut s), armed, "the next live frame builds none");
}

#[test]
fn the_listing_has_each_local_shadow_draw_with_its_cull_and_the_summaries() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = WorldSurface::<States>::default();
    s.bind(&Scene { fade: false }.values(), None).unwrap();
    for ms in [0., 20., 40., 60., 80.] {
        fixture::render(&gpu, &mut s, &frame(ms)).unwrap();
    }
    let reply = s
        .agent(r#"{"op":"state","draws":true,"width":320,"height":180}"#)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    let draws = value["draws"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"));
    let local: Vec<_> = (draws.iter())
        .filter(|d| d["pass"] == "local shadow 0")
        .collect();
    assert!(!local.is_empty(), "{reply}");
    for d in &local {
        assert!(d["winding"].is_string() && d["variant"].is_u64(), "{d}");
        assert!(
            d["kept"].as_u64().unwrap() <= d["submitted"].as_u64().unwrap(),
            "{d}"
        );
    }
    assert!(
        local.iter().any(|d| d["kept"].as_u64() > Some(0)),
        "{reply}"
    );
    let bloom = draws.iter().find(|d| d["draw"] == "bloom");
    assert!(bloom.is_none_or(|d| d["summary"].is_string()), "{reply}");
    let particles = draws.iter().find(|d| d["fragments"].is_u64());
    assert!(particles.unwrap_or_else(|| panic!("{reply}"))["fragments"].as_u64() > Some(0));
}

static FAIL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Hooks whose compute stage fails while `FAIL` is set.
#[derive(Default)]
struct Failing;
impl crate::Hooks for Failing {
    fn compute(
        &mut self,
        _: &mut exact_gpu::wgpu::CommandEncoder,
        _: &crate::FrameView<'_>,
    ) -> Result<(), crate::RenderError> {
        if FAIL.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(crate::RenderError::Scene("told to fail".into()));
        }
        Ok(())
    }
}

#[test]
fn a_failed_look_leaves_no_probe_state_for_the_next_frame() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = WorldSurface::<States, (), false, Failing>::default();
    s.bind(&Scene { fade: false }.values(), None).unwrap();
    for ms in [0., 20., 40., 60.] {
        fixture::render(&gpu, &mut s, &frame(ms)).unwrap();
    }
    // The sparks are drawn: a look frame arms their counting, then its hook's
    // compute stage fails.
    FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
    let reply = s
        .agent(r#"{"op":"state","look":true,"width":320,"height":180}"#)
        .unwrap();
    FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(reply.contains("told to fail"), "{reply}");
    let built = s.renderer_for_test().unwrap().probe_built.get();
    fixture::render(&gpu, &mut s, &frame(80.)).unwrap();
    let r = s.renderer_for_test().unwrap();
    assert!(r.probe.is_none() && !r.quads.counting_armed());
    assert!(
        !r.quads.split(),
        "an ordinary frame draws translucency in its forward pass"
    );
    assert_eq!(r.probe_built.get(), built);
}

/// A skinned cube whose joint holds it 20 m from its rest box, blended or not:
/// the CPU routes' bounds (blended models, a direct cull) follow the palette.
#[derive(Default, Args)]
struct DeformedArgs {
    blend: bool,
}
struct Deformed;
impl Game for Deformed {
    const ID: &'static str = "look-deformed";
    const ASSETS: &'static [&'static str] = &["deformed.model"];
    type Args = DeformedArgs;
    fn setup(w: &mut World, _: &DeformedArgs) {
        w.spawn_named(
            "camera",
            (
                Transform::at(20., 3., 6.).looking_at(Vec3::new(20., 0.5, 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn_named(
            "deformed",
            (Transform::default(), Mesh::asset("deformed.model")),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &DeformedArgs) {}
}

#[test]
fn skinned_bounds_follow_the_palette_on_the_blended_and_direct_routes() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    for (blend, direct) in [(true, false), (false, true)] {
        let mut model = crate::test_model::skinned_model();
        let joint = model.skins[0].joints[0] as usize;
        model.nodes[joint].transform =
            Mat4::from_translation(Vec3::new(20., 0., 0.)).to_cols_array();
        model.materials.fill(Default::default());
        if blend {
            model.materials[0].alpha_mode = exact_game::asset::AlphaMode::Blend;
        }
        model.textures.clear();
        let mut s = WorldSurface::<Deformed, crate::ModelExecutor, true>::default();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.bind(&[exact_gpu::Value::Bool(blend)], None).unwrap();
        s.asset("deformed.model", Ok(&exact_game::bin::to_vec(&model)));
        fixture::render(&gpu, &mut s, &frame(0.)).unwrap();
        if direct {
            // As a device without indirect draws culls: every group drawn whole.
            let r = s.renderer_for_test().unwrap();
            r.cull.indirect_execution = false;
            r.cull.epoch += 1;
        }
        for ms in [20., 40.] {
            fixture::render(&gpu, &mut s, &frame(ms)).unwrap();
        }
        assert!(s.error().is_none(), "{:?}", s.error());
        assert_eq!(s.renderer_for_test().unwrap().cull.direct, direct);
        let reply = s
            .agent(r#"{"op":"screenshot","form":"look","top":30,"width":320,"height":180}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
        let centre = value["bounds"]["centre"][0].as_f64();
        assert!(
            centre.is_some_and(|x| (x - 20.).abs() < 3.),
            "blend {blend}, direct {direct}: {}",
            value["bounds"]
        );
    }
}

/// Sparks that fade softly into the ground.
struct SoftSparks;
impl Game for SoftSparks {
    const ID: &'static str = "look-soft-sparks";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn((
            Transform::at(0., 2., 8.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
        ));
        w.spawn((
            Transform::default(),
            Mesh::plane(20., 20.),
            Material::hex("#5d7a52"),
        ));
        let sparks = w.spawn((
            Transform::at(0., 0.5, 0.),
            Emitter::sparks().rate(400.).lifetime(2.),
        ));
        w.insert(
            sparks,
            ParticleLook {
                soft: 0.5,
                ..Default::default()
            },
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        emitter::step(w);
    }
}
/// Hooks that copy the scene, whose continuation draws particles hard.
#[derive(Default)]
struct Copying;
impl crate::Hooks for Copying {
    fn needs(&self) -> crate::Needs {
        crate::Needs::SCENE_COPY
    }
}

/// The variant the listing names for the soft sparks' draw, and the listing.
fn soft_sparks_variant<H: crate::Hooks + Default>() -> Option<(u64, String)> {
    let gpu = fixture::device_or_skip(fixture::device())?;
    let mut s = WorldSurface::<SoftSparks, (), false, H>::default();
    s.bind(&[], None).unwrap();
    for ms in [0., 20., 40., 60.] {
        fixture::render(&gpu, &mut s, &frame(ms)).unwrap();
    }
    let reply = s
        .agent(r#"{"op":"state","draws":true,"width":320,"height":180}"#)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    let draws = value["draws"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"));
    let sparks = draws.iter().find(|d| {
        d["draw"]
            .as_str()
            .is_some_and(|b| b.starts_with("particle dots"))
    });
    let variant = sparks.unwrap_or_else(|| panic!("{reply}"))["variant"].as_u64();
    Some((variant.unwrap_or_else(|| panic!("{reply}")), reply))
}

#[test]
fn a_quad_record_names_the_pipeline_its_draw_bound() {
    // Soft additive dots (4 × soft + additive) in a frame whose translucent
    // pass reads the depth.
    let Some((soft, reply)) = soft_sparks_variant::<()>() else {
        return;
    };
    assert_eq!(soft, 5, "{reply}");
    // Under a scene copy the same authored batch draws hard.
    let (copied, reply) = soft_sparks_variant::<Copying>().unwrap();
    assert_eq!(copied, 1, "{reply}");
}
