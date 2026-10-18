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
    fn tick(_: &mut World, _: &Input, _: &Scene) {}
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
