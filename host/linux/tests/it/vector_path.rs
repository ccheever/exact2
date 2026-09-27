//! A `path` on Linux (LLP 1065 D7): the painter strokes the kernel's trim of
//! the path between the engine's `stroke-start` and `stroke-end`, so the
//! subpaths ink one after another — pen order — as the transition runs.

use exact_linux::{presenter::PainterChoice, Presenter};
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;
use tiny_skia::Pixmap;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// Whether any pixel in a box of path units (the fixture's 200×100 view box
/// fitted into its 300×150 box: 1.5 pixels a unit) is ink, not page.
fn inked(frame: &Pixmap, origin: (f32, f32), units: (f32, f32, f32, f32)) -> bool {
    let (x0, y0, x1, y1) = units;
    ((y0 * 1.5) as u32..(y1 * 1.5) as u32).any(|y| {
        ((x0 * 1.5) as u32..(x1 * 1.5) as u32).any(|x| {
            let c = frame
                .pixel(origin.0 as u32 + x, origin.1 as u32 + y)
                .unwrap()
                .demultiply();
            u32::from(c.red()) + u32::from(c.green()) + u32::from(c.blue()) < 600
        })
    })
}

#[test]
fn subpaths_ink_in_pen_order_as_stroke_end_runs() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/path.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (420., 600.),
        1.,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let id = |p: &Presenter<NoData>, name: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let mark = id(&p, "mark");
    let rect = p.rect_of(mark).unwrap();
    let origin = (rect.0, rect.1);
    // The loop's start, the wave's far half, the underline's far half.
    let (lead, wave, line) = (
        (12., 20., 60., 78.),
        (150., 25., 195., 60.),
        (120., 82., 180., 94.),
    );
    let ink = |p: &mut Presenter<NoData>| {
        let frame = p.frame();
        [lead, wave, line].map(|units| inked(&frame, origin, units))
    };
    assert_eq!(ink(&mut p), [false; 3], "stroke-end=0 draws nothing");
    assert_eq!(p.host().presented(mark).stroke, (0.0, 0.0));
    let sign = id(&p, "sign");
    p.tap(sign).unwrap();
    let mut seen = Vec::new();
    for ms in (100..=1700).step_by(100) {
        p.tick(ms as f64);
        let now = ink(&mut p);
        // A later subpath never inks before an earlier one.
        assert!(now[0] || !now[1], "{ms} ms: {now:?}");
        assert!(now[1] || !now[2], "{ms} ms: {now:?}");
        seen.push(now);
    }
    assert!(seen.contains(&[true, false, false]), "{seen:?}");
    assert!(seen.contains(&[true, true, false]), "{seen:?}");
    assert_eq!(seen.last(), Some(&[true; 3]));
    assert_eq!(p.host().presented(mark).stroke, (0.0, 1.0));
}
