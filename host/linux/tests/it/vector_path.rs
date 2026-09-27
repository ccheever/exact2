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

const SVG: &str = r##"component App
  state on = false
  action go writes on
    on = true
  view
    column
      button press=go testId="go" width=20 height=20
      path testId="dots" width=100 height=20 d="M10 10 Z M30 10 H90" stroke="#000" stroke-width=10 stroke-linecap="round" fill="none" stroke-end=(on ? 1 : 0)
      path testId="dash" width=100 height=20 d="M0 10 H100" stroke="#000" stroke-width=10 stroke-dasharray="20 10" stroke-end=(on ? 0.5 : 1) fill="none"
      path testId="hole" width=40 height=40 d="M0 0 H40 V40 H0 Z M10 10 H30 V30 H10 Z" fill="#000" fill-rule="evenodd"
      path testId="stretch" width=100 height=20 viewBox="0 0 10 10" preserveAspectRatio="none" d="M0 0 H10 V10 H0 Z" fill="#000"
      path testId="tint" width=40 height=20 d="M0 0 H40 V20 H0 Z" fill=(on ? "#0000ff" : "#ff0000") transition="fill 1s linear"
      column testId="clip" width=40 height=40 background-color="#000" clip-path="path('m0 0 h20 v20 h-20 z')"
      path testId="thin" width=100 height=40 viewBox="0 0 10 4" d="M0 2 H10" stroke="#000" stroke-width=2 vector-effect="non-scaling-stroke"
"##;

/// SVG's painting on Linux (LLP 1065): a zero-length subpath's dot, dashes
/// the trim reveals, `fill-rule`, `preserveAspectRatio`, a `fill`
/// transition, and `clip-path` in the full path grammar.
#[test]
fn dots_dashes_rules_aspect_fill_motion_and_clip_paths() {
    let plan = contract::compile(SVG).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (200., 300.),
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
    // A pixel at (x, y) in a node's box.
    let at = |p: &mut Presenter<NoData>, name: &str, x: f32, y: f32| {
        let node = id(p, name);
        let r = p.rect_of(node).unwrap();
        let c = p
            .frame()
            .pixel((r.0 + x) as u32, (r.1 + y) as u32)
            .unwrap()
            .demultiply();
        [c.red(), c.green(), c.blue()]
    };
    let ink = |p: &mut Presenter<NoData>, name: &str, x: f32, y: f32| {
        at(p, name, x, y).iter().map(|c| u32::from(*c)).sum::<u32>() < 200
    };
    // Nothing drawn: no dot. Dashes: 0–20, 30–50, 60–80, 90–100.
    assert!(!ink(&mut p, "dots", 10., 10.));
    assert!(ink(&mut p, "dash", 35., 10.) && ink(&mut p, "dash", 65., 10.));
    assert!(!ink(&mut p, "dash", 25., 10.));
    // Even-odd leaves the inner square a hole; nonzero would fill it.
    assert!(ink(&mut p, "hole", 5., 5.) && !ink(&mut p, "hole", 20., 20.));
    // `none` stretches the unit square across the whole box.
    assert!(ink(&mut p, "stretch", 95., 10.) && ink(&mut p, "stretch", 5., 10.));
    // A relative `h`/`v` clip: the top-left quarter only.
    assert!(ink(&mut p, "clip", 10., 10.) && !ink(&mut p, "clip", 30., 30.));
    assert_eq!(at(&mut p, "tint", 20., 10.), [255, 0, 0]);
    // Two pixels wide, not twenty: the stroke is the box's, not the view box's.
    assert!(ink(&mut p, "thin", 50., 20.) && !ink(&mut p, "thin", 50., 16.));
    let go = id(&p, "go");
    p.tap(go).unwrap();
    p.tick(500.0);
    // The pen has started: the dot at the path's start shows, round.
    assert!(ink(&mut p, "dots", 10., 10.) && !ink(&mut p, "dots", 20., 10.));
    // Half the dashed line: its first dashes, not moved; nothing past 50.
    assert!(ink(&mut p, "dash", 35., 10.) && ink(&mut p, "dash", 45., 10.));
    assert!(!ink(&mut p, "dash", 65., 10.) && !ink(&mut p, "dash", 25., 10.));
    // Red to blue, halfway, premultiplied as Chrome does.
    let [r, g, b] = at(&mut p, "tint", 20., 10.);
    assert!(
        (r as i32 - 128).abs() <= 1 && g == 0 && (b as i32 - 127).abs() <= 1,
        "{r} {g} {b}"
    );
    p.tick(1500.0);
    assert_eq!(at(&mut p, "tint", 20., 10.), [0, 0, 255]);
}
