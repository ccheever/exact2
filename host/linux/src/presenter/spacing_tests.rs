//! LLP 1115.000 D10 on the Linux host (and so on Android's and Windows's,
//! which lay out and paint through this presenter): a control's spacing and
//! size are the plan's, the same on every host.
use super::*;

fn boot(app: &str, scale: f32) -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(app)
            .unwrap_or_else(|e| panic!("{e}"))
            .encode(),
        (),
        (400., 400.),
        scale,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn rect(p: &mut Presenter<()>, test_id: &str) -> (f32, f32, f32, f32) {
    let k = p.host().kernel();
    let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    p.rect_of(id).unwrap()
}

/// D10.1: a checkbox, a radio and a range sit where their row puts them,
/// with no margin of their own (Chrome's UA margins put the checkbox at 4,
/// 3 and each next control 7, 8 and 5 points further on).
#[test]
fn a_checkbox_a_radio_and_a_range_have_no_margin_of_their_own() {
    let mut p = boot(
        r#"component App
  view
    row width=400 testId="row"
      input type="checkbox" testId="check"
      input type="radio" testId="radio"
      input type="range" width=90 testId="range"
      box width=10 height=10 testId="after"
"#,
        1.,
    );
    let [check, radio, range, after] =
        ["check", "radio", "range", "after"].map(|id| rect(&mut p, id));
    assert_eq!((check.0, check.1), (0.0, 0.0), "the checkbox");
    assert_eq!((radio.0, radio.1), (check.2, 0.0), "the radio touches it");
    assert_eq!((range.0, range.1), (check.2 + radio.2, 0.0), "the range");
    assert_eq!(after.0, check.2 + radio.2 + 90.0, "and what follows");
}

/// D10.2: a range and a determinate `progress` are as wide as what holds
/// them, by the plan's `width: 100%` and nothing of this host's: across a
/// column and a box, what a row's other children leave, an authored width
/// where one is written, and their own size (129; `10em` by `1em`) where
/// the percentage has nothing to resolve against.
#[test]
fn a_range_and_a_determinate_progress_are_full_width() {
    let mut p = boot(
        r#"component App
  view
    column width=400 testId="root"
      input type="range" testId="range"
      progress value=0.5 testId="bar"
      box width=200 testId="box"
        input type="range" testId="box-range"
        progress value=0.5 testId="box-bar"
      row testId="row"
        box width=100 height=10 flex-shrink=0
        input type="range" testId="row-range"
      row width=200
        box width=150 height=10 flex-shrink=0
        progress value=0.5 testId="tight-bar"
      input type="range" width=90 testId="narrow"
      row
        column testId="fit"
          input type="range" testId="fit-range"
        column
          progress value=0.5 testId="fit-bar"
"#,
        1.,
    );
    let mut size = |id: &str| {
        let (_, _, w, h) = rect(&mut p, id);
        (w, h)
    };
    assert_eq!(size("range").0, 400.0);
    assert_eq!(size("bar"), (400.0, 16.0));
    assert_eq!(size("box-range").0, 200.0);
    assert_eq!(size("box-bar"), (200.0, 16.0));
    assert_eq!(size("row-range").0, 300.0, "what the row's box leaves");
    assert_eq!(size("tight-bar").0, 50.0, "below its own 160");
    assert_eq!(size("narrow").0, 90.0, "an authored width wins");
    assert_eq!(size("fit-range").0, 129.0, "its own width");
    assert_eq!(size("fit-bar"), (160.0, 16.0), "10em by 1em");
}

const RULED: &str = r##"component App
  view
    column width=200 height=100 background-color="#ffffff" testId="root"
      box height=20 testId="above"
      hr color="#ff0000" testId="rule"
      box height=20 testId="below"
"##;

/// The colour of device pixel (x, y) of the painted frame.
fn pixel(p: &mut Presenter<()>, x: u32, y: u32) -> [u8; 3] {
    let c = p.frame().pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue()]
}

/// D10.3: an `hr` is one hairline with no margin: one device pixel of the
/// scale this host draws at, in layout and in paint. The presenter tells the
/// kernel that scale when it boots and whenever it changes (a desktop's
/// `EXACT_SCALE`, the density an Android reader was told, a Windows
/// window's scale factor: all come through `resize_scaled`).
#[test]
fn an_hr_is_one_device_pixel_of_the_scale_this_host_draws_at() {
    const RED: [u8; 3] = [0xff, 0, 0];
    const WHITE: [u8; 3] = [0xff, 0xff, 0xff];
    // 1x: a point, at 20 with nothing above it but the box.
    let mut p = boot(RULED, 1.);
    assert_eq!(p.host().runner().display_scale(), 1.0);
    assert_eq!(rect(&mut p, "rule"), (0.0, 20.0, 200.0, 1.0));
    assert_eq!(rect(&mut p, "below").1, 21.0);
    assert_eq!(pixel(&mut p, 10, 19), WHITE);
    assert_eq!(pixel(&mut p, 10, 20), RED);
    assert_eq!(pixel(&mut p, 10, 21), WHITE);
    // 2x: half a point, and still one row of device pixels.
    let mut p = boot(RULED, 2.);
    assert_eq!(p.host().runner().display_scale(), 2.0);
    assert_eq!(rect(&mut p, "rule"), (0.0, 20.0, 200.0, 0.5));
    assert_eq!(rect(&mut p, "below").1, 20.5);
    assert_eq!(pixel(&mut p, 20, 39), WHITE);
    assert_eq!(pixel(&mut p, 20, 40), RED);
    assert_eq!(pixel(&mut p, 20, 41), WHITE, "one device pixel, not two");
    // The window moves to a 3x display: a third of a point.
    assert!(p.resize_scaled(400., 400., 3.).is_none());
    assert_eq!(p.host().runner().display_scale(), 3.0);
    let (_, y, _, h) = rect(&mut p, "rule");
    assert!((h - 1.0 / 3.0).abs() < 1e-4 && y == 20.0, "{y} {h}");
    assert_eq!(pixel(&mut p, 30, 59), WHITE);
    assert_eq!(pixel(&mut p, 30, 60), RED);
    assert_eq!(pixel(&mut p, 30, 61), WHITE);
}
