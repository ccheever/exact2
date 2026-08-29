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
    match fixture::device() {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("{e}; the readback fixture is skipped");
            None
        }
    }
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
    map.bind(&[line.clone(), Value::str("mv"), board, Value::Number(0.0)])
        .unwrap();
    let v = map.vertices(200.0, 160.0);
    // The line (6), three stations (18), the train (6).
    assert_eq!(v.len(), 30);
    let red = v
        .iter()
        .filter(|x| x.color[0] > 0.7 && x.color[1] < 0.3)
        .count();
    assert_eq!(red, 6, "the selected station is the red dot");
    assert!(
        map.bind(&[
            line,
            Value::Number(1.0),
            Value::List(vec![].into()),
            Value::Number(0.0)
        ])
        .is_err(),
        "a wrong input is refused by name"
    );
}

#[test]
fn the_map_renders_and_reads_back_on_this_machines_gpu() {
    let Some(gpu) = device() else { return };
    let mut map = MapSurface::new();
    let line = Value::List(vec![station("sf"), station("mv"), station("sj")].into());
    map.bind(&[
        line,
        Value::str("mv"),
        Value::List(vec![].into()),
        Value::Number(0.0),
    ])
    .unwrap();
    let frame = Frame {
        width: 200.0,
        height: 160.0,
        scale: 1.0,
        now_ms: 0.0,
    };
    let (px, wants) = fixture::render(&gpu, &mut map, &frame).unwrap();
    assert!(!wants, "a static picture wants no more frames");
    px.save("map");
    assert_eq!(
        px.at(5, 5),
        [247, 247, 247, 255],
        "the background is the clear color"
    );
    let mid = px.at(100, 80);
    assert!(
        mid[0] > 150 && mid[1] < 100,
        "the selected station's red dot sits mid-line: {mid:?}"
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
        sky.bind(&[Value::Number(1.0)]).is_err(),
        "the seed is a string"
    );
    sky.bind(&[Value::str("mv")]).unwrap();
    let frame = Frame {
        width: 64.0,
        height: 64.0,
        scale: 1.0,
        now_ms: 1234.0,
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
