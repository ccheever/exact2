//! The line map and the aurora, headless: rendered into a module-owned
//! texture on this machine's GPU and read back (LLP 1009 D4's fixture path,
//! `exact_gpu::fixture`). Skips, saying so, when no adapter exists.
//! `EXACT_GPU_OUT=<dir>` keeps each picture as a PPM.

use caltrain_gpu::{AuroraSurface, MapSurface};
use exact_gpu::fixture;
use exact_gpu::{Frame, Gpu, Surface, Value};

fn station(id: &str) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::str(id),
        Value::Number(1.0),
        Value::Number(100.0),
    ])
}

fn device() -> Option<Gpu> {
    // The shaders travel as files (LLP 1030 D8): registered as a host would.
    exact_gpu::shaders::load_dir(&caltrain_gpu::shader_dir(), &caltrain_gpu::REGISTRY).unwrap();
    fixture::device_or_skip(fixture::device())
}

#[test]
fn the_map_binds_its_inputs_and_lays_out_stations_on_a_line() {
    let mut map = MapSurface::new();
    let line = Value::List(vec![station("sf"), station("mv"), station("sj")].into());
    let board = Value::List(
        vec![Value::record(vec![
            Value::str("d1"),
            Value::Number(131.0),
            Value::str("Local"),
            Value::str("San Francisco"),
            Value::Number(600_000.0),
        ])]
        .into(),
    );
    map.bind(
        &[line.clone(), Value::str("mv"), board, Value::Number(0.0)],
        None,
    )
    .unwrap();
    let v = map.vertices(200.0, 160.0);
    // The line (6) and the train (6); the stations are the canvas's
    // children, laid out by the kernel (LLP 1014).
    assert_eq!(v.len(), 12);
    let blue = v
        .iter()
        .filter(|x| x.color[2] > 0.8 && x.color[0] < 0.3)
        .count();
    assert_eq!(blue, 6, "the train is the blue dot");
    assert!(
        map.bind(
            &[
                line,
                Value::Number(1.0),
                Value::List(vec![].into()),
                Value::Number(0.0)
            ],
            None
        )
        .is_err(),
        "a wrong input is refused by name"
    );
}

#[test]
fn the_map_renders_and_reads_back_on_this_machines_gpu() {
    let Some(gpu) = device() else { return };
    let mut map = MapSurface::new();
    let line = Value::List(vec![station("sf"), station("mv"), station("sj")].into());
    map.bind(
        &[
            line,
            Value::str("mv"),
            Value::List(vec![].into()),
            Value::Number(0.0),
        ],
        None,
    )
    .unwrap();
    let frame = Frame {
        width: 200.0,
        height: 160.0,
        scale: 1.0,
        now_ms: 0.0,
        children_generation: 0,
        seekable: false,
        period_ms: 0.0,
        shader_generation: exact_gpu::shaders::shader_generation(),
    };
    let (px, wants) = fixture::render(&gpu, &mut map, &frame).unwrap();
    assert!(!wants, "a static picture wants no more frames");
    px.save("map");
    assert_eq!(
        px.at(5, 5),
        [247, 247, 247, 255],
        "the background is the clear color"
    );
    let beside = px.at(120, 80);
    assert_eq!(
        beside,
        [247, 247, 247, 255],
        "no dots: the stations are the canvas's children"
    );
    let on_line = px.at(100, 40);
    assert!(
        on_line[0] > 170 && on_line[0] < 200 && on_line[0] == on_line[1],
        "the line is gray: {on_line:?}"
    );
}

#[test]
fn the_aurora_renders_a_lit_sky_and_wants_every_frame() {
    let Some(gpu) = device() else { return };
    let mut sky = AuroraSurface::new();
    assert!(
        sky.bind(&[Value::Number(1.0)], None).is_err(),
        "the seed is a string"
    );
    sky.bind(&[Value::str("mv")], None).unwrap();
    let frame = Frame {
        width: 64.0,
        height: 64.0,
        scale: 1.0,
        now_ms: 1234.0,
        children_generation: 0,
        seekable: false,
        period_ms: 0.0,
        shader_generation: exact_gpu::shaders::shader_generation(),
    };
    let (px, wants) = fixture::render(&gpu, &mut sky, &frame).unwrap();
    assert!(wants, "lit from the clock: wants every frame");
    px.save("aurora");
    let distinct: std::collections::HashSet<[u8; 4]> = (0..64)
        .flat_map(|y| (0..64).map(move |x| (x, y)))
        .map(|(x, y)| px.at(x, y))
        .collect();
    assert!(
        distinct.len() > 200,
        "a field, not a fill: {} colors",
        distinct.len()
    );
    let lit = px.count(|p| p[..3].iter().any(|c| *c > 60));
    assert!(lit > 200, "ribbons light the sky: {lit} lit pixels of 4096");
}
