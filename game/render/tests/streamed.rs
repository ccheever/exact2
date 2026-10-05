#![cfg(not(target_arch = "wasm32"))]
//! `Game::STREAMED` models never delay or bloat the first drawn frame: none is
//! prepared before it, and a few per frame after it, those shown first. The
//! garden's classic look prepared the art pass's 220 models inside its first
//! frame (671 mesh uploads against 37).
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::{ModelExecutor, WorldSurface};
use exact_gpu::{fixture, Frame, Surface};

const NAMES: [&str; 20] = [
    "s-00.model",
    "s-01.model",
    "s-02.model",
    "s-03.model",
    "s-04.model",
    "s-05.model",
    "s-06.model",
    "s-07.model",
    "s-08.model",
    "s-09.model",
    "s-10.model",
    "s-11.model",
    "s-12.model",
    "s-13.model",
    "s-14.model",
    "s-15.model",
    "s-16.model",
    "s-17.model",
    "s-18.model",
    "s-19.model",
];
struct Streams;
impl Game for Streams {
    const ID: &'static str = "streamed-preparation";
    const STREAMED: &'static [&'static str] = &NAMES;
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("shown", (Transform::default(), Mesh::asset("s-19.model")));
        w.spawn((
            Transform::at(0., 0., 4.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
        ));
    }
    // Paused: the surface must still ask for frames until the last is prepared.
    fn paused(_: &()) -> bool {
        true
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}

fn triangle() -> Vec<u8> {
    let mesh = asset::MeshData {
        positions: vec![-0.4, -0.4, 0., 0.4, -0.4, 0., 0., 0.4, 0.],
        normals: vec![0., 0., 1., 0., 0., 1., 0., 0., 1.],
        uvs: vec![0.; 6],
        indices: vec![0, 1, 2],
        bounds: [-0.4, -0.4, 0., 0.4, 0.4, 0.],
        ..Default::default()
    };
    bin::to_vec(&asset::Model {
        bounds: mesh.bounds,
        meshes: vec![mesh],
        materials: vec![Default::default()],
        nodes: vec![asset::Node {
            mesh: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    })
}

#[test]
fn streamed_models_wait_for_the_first_drawn_frame_then_prepare_a_few_per_frame_shown_first() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut surface = WorldSurface::<Streams, ModelExecutor, true>::default();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    surface.bind(&[], None).unwrap();
    let bytes = triangle();
    // What is shown is asked for alone, then the rest.
    assert_eq!(surface.assets().requests, ["s-19.model"]);
    surface.asset("s-19.model", Ok(&bytes));
    assert_eq!(surface.assets().requests.len(), NAMES.len() - 1);
    for name in &NAMES[..19] {
        surface.asset(name, Ok(&bytes));
    }
    let frame = Frame {
        width: 32.,
        height: 32.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let unprepared = |s: &WorldSurface<Streams, ModelExecutor, true>| {
        s.sim().unwrap().streamed_unprepared().len()
    };
    let state =
        |s: &mut WorldSurface<Streams, ModelExecutor, true>| s.agent(r#"{"op":"state"}"#).unwrap();

    // Every model has landed, yet the first frame draws without preparing one.
    let (_, wants) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    assert_eq!(unprepared(&surface), NAMES.len());
    let first = state(&mut surface);
    assert!(!first.contains("first draw pending"), "{first}");
    assert!(first.contains("streamed models preparing"), "{first}");
    assert!(wants, "a paused world still wants frames to prepare in");

    // Then eight per frame, the shown model first.
    let (_, wants) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    assert_eq!(unprepared(&surface), NAMES.len() - 8);
    assert!(surface.sim().unwrap().model_prepared("s-19.model"));
    assert!(wants);
    fixture::render(&gpu, &mut surface, &frame).unwrap();
    assert_eq!(unprepared(&surface), NAMES.len() - 16);
    fixture::render(&gpu, &mut surface, &frame).unwrap();
    assert_eq!(unprepared(&surface), 0);
    let (_, wants) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    assert!(!wants, "nothing left to prepare and the world is paused");
    let done = state(&mut surface);
    assert!(done.contains(r#""ready":true"#), "{done}");
}
