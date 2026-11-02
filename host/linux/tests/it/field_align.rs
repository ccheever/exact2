//! `text-align` on a single-line field: its line sits where the web puts it
//! (start, centre, end of the content box), not always at the left (bench
//! t1-tip on Android, 2026-10-08).
use exact_kernel::ViewId;
use exact_linux::{presenter::PainterChoice, Presenter};

fn boot() -> Presenter<()> {
    let plan = contract::compile(
        r##"component App
  view
    column width=400 height=400 background-color="#ffffff" color="#000000" font-size=20 gap=10
      input appearance="none" width=300 height=30 value="Hi" text-align="left" testId="left"
      input appearance="none" width=300 height=30 value="Hi" text-align="center" testId="center"
      input appearance="none" width=300 height=30 value="Hi" text-align="right" testId="right"
      input appearance="none" width=300 height=30 value="Hi" text-align="end" direction="rtl" testId="rtl-end"
      input appearance="none" width=300 height=30 value="Hi" text-align="justify" direction="rtl" testId="rtl-justify"
      input appearance="none" width=60 height=30 value="A long line" direction="rtl" testId="rtl-wide"
      input appearance="none" width=300 height=30 value="Hi" text-align="center" caret-color="#ff0000" autofocus=true testId="focused"
"##,
    )
    .unwrap();
    let (p, err) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 400.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// The leftmost and rightmost columns of dark ink (text, caret) inside a field's frame.
fn ink(p: &mut Presenter<()>, name: &str) -> (u32, u32) {
    let f = p.host().kernel().node(id(p, name)).unwrap().frame;
    let shot = p.frame();
    let (mut lo, mut hi) = (u32::MAX, 0);
    for x in f.x as u32..(f.x + f.width) as u32 {
        for y in f.y as u32..(f.y + f.height) as u32 {
            let c = shot.pixel(x, y).unwrap();
            // Dark grey ink only: a focused field's accent ring (#0075ff) is not text.
            if c.red() < 128 && c.green() < 128 && c.blue() < 128 && c.alpha() > 128 {
                lo = lo.min(x - f.x as u32);
                hi = hi.max(x - f.x as u32);
            }
        }
    }
    assert!(lo <= hi, "{name} painted no ink");
    (lo, hi)
}

#[test]
fn a_fields_text_align_places_its_line() {
    let mut p = boot();
    let (l0, l1) = ink(&mut p, "left");
    let (c0, c1) = ink(&mut p, "center");
    let (r0, r1) = ink(&mut p, "right");
    let (e0, e1) = ink(&mut p, "rtl-end");
    let (j0, j1) = ink(&mut p, "rtl-justify");
    let (_, o1) = ink(&mut p, "rtl-wide");
    let w = l1 - l0;
    // Glyphs sit on a fraction-of-a-pixel grid: widths agree within a pixel.
    let same = |a: u32, b: u32| (b - a).abs_diff(w) <= 1;
    assert!(l0 < 10, "left starts at the start edge: {l0}");
    let centre = (c0 + c1) as f32 / 2.0;
    assert!(
        (centre - 150.0).abs() < 6.0,
        "centered about the middle: {c0}..{c1}"
    );
    assert!(
        r1 > 290 && same(r0, r1),
        "right ends at the end edge: {r0}..{r1}"
    );
    // `end` under `rtl` is the left edge, as CSS has it.
    assert!(e0 < 10 && same(e0, e1), "rtl end is the left: {e0}..{e1}");
    // `justify` on a field's one line is `start`: the right under `rtl`.
    assert!(
        j1 > 290 && same(j0, j1),
        "rtl justify is the right: {j0}..{j1}"
    );
    // An overflowing `rtl` line keeps its start edge, the right.
    assert!(
        o1 >= 55,
        "an overflowing rtl line keeps its right edge: ..{o1}"
    );
}

/// The columns where a field paints its red caret.
fn caret_columns(p: &mut Presenter<()>, name: &str) -> Vec<u32> {
    let f = p.host().kernel().node(id(p, name)).unwrap().frame;
    let shot = p.frame();
    (f.x as u32..(f.x + f.width) as u32)
        .filter(|&x| {
            (f.y as u32..(f.y + f.height) as u32).any(|y| {
                let c = shot.pixel(x, y).unwrap();
                // A 1 px caret at a fractional x blends half into the white.
                c.red() > 200
                    && c.green() < 200
                    && c.red() - c.green() > 60
                    && c.green() == c.blue()
            })
        })
        .map(|x| x - f.x as u32)
        .collect()
}

#[test]
fn a_centered_fields_caret_sits_at_its_text() {
    let mut p = boot();
    let (c0, c1) = ink(&mut p, "center");
    let caret = caret_columns(&mut p, "focused");
    assert!(!caret.is_empty(), "the focused field paints its caret");
    // At the end of "Hi": just after the centred text, not at the left edge.
    let at = caret[0];
    assert!(
        at + 2 >= c1 && at <= c1 + 4,
        "the caret just after the text: {at} vs {c0}..{c1}"
    );
}
