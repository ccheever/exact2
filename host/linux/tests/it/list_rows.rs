//! LLP 1115.000 D16 where this painter is Android's reader: a process that
//! says no look (every test of this binary, and the reader) paints Material,
//! and a Material 3 list item has no divider of its own. So a list whose
//! rows are cells is drawn there with no line between its rows, and nothing
//! of the neutral look's row (its line, its washes), which is the desktop's
//! and is held in `tests/neutral`.
use exact_kernel::ViewId;
use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;

const APP: &str = r##"component App
  state taps = 0
  action go
    taps = taps + 1
  view
    column width=400 padding=20 background-color="#ffffff"
      list width=300 testId="rows"
        text "One" height=40 testId="row-1"
        row press=go height=40 testId="row-2"
          text "Two"
        text "Last" height=40 testId="row-last"
"##;

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// The pixel `(x, y)` into `name`'s box.
fn pixel(p: &mut Presenter<()>, name: &str, x: f32, y: f32) -> [u8; 3] {
    let (left, top, _, _) = p.rect_of(id(p, name)).unwrap();
    let shot = p.frame();
    let c = shot.pixel((left + x) as u32, (top + y) as u32).unwrap();
    [c.red(), c.green(), c.blue()]
}

#[test]
fn a_process_that_says_no_look_draws_no_line_between_a_lists_rows() {
    let plan = contract::compile(APP).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 300.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    // The list is marked, and the kernel says where a platform's separator
    // would go: this painter draws none there.
    let k = p.host().kernel();
    assert!(k.row_separator(id(&p, "row-1")));
    assert!(k.row_separator(id(&p, "row-2")));
    assert!(!k.row_separator(id(&p, "row-last")));
    let white = [255; 3];
    for row in ["row-1", "row-2", "row-last"] {
        for y in [38.0, 39.0] {
            assert_eq!(pixel(&mut p, row, 290.0, y), white, "{row} at {y}");
        }
    }
    // Nor the neutral look's wash where a pointer hovers or a contact holds.
    let row = id(&p, "row-2");
    p.hover(row).unwrap();
    assert_eq!(pixel(&mut p, "row-2", 290.0, 20.0), white);
    let (x, y, _, _) = p.rect_of(row).unwrap();
    assert!(p.pointer_down(x + 290.0, y + 20.0, 0.0).unwrap());
    assert_eq!(pixel(&mut p, "row-2", 290.0, 20.0), white);
    p.pointer_cancel(0.0).unwrap();
}
