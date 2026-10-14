//! The window a shell shows the page in (LLP 1115.001 D3.3, D3.6): what the
//! presenter makes of the raw insets, and the colour behind the page.
use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

fn boot(source: &str) -> Presenter<Nothing> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Nothing,
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

/// A phone's window: a status bar under a cutout, a gesture bar below.
fn phone() -> Window {
    Window {
        size: (400., 800.),
        bars: [24., 0., 16., 0.],
        cutout: [30., 0., 0., 0.],
        keyboard: [0.; 4],
        gestures: [30., 12., 16., 12.],
    }
}

fn frame_of(p: &Presenter<Nothing>, name: &str) -> (f32, f32, f32, f32) {
    let kernel = p.host().kernel();
    let f = kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .frame;
    (f.x, f.y, f.width, f.height)
}

fn env(p: &mut Presenter<Nothing>) -> serde_json::Value {
    let layout: serde_json::Value = serde_json::from_str(&p.layout_json(None, false)).unwrap();
    layout["env"].clone()
}

/// A page that fills its viewport and pads by the safe area.
fn page(root: &str) -> String {
    format!(
        "component App\n  view\n    main testId=\"root\" width=\"100%\" height=\"100%\" {root} padding-top=\"env(safe-area-inset-top)\" padding-bottom=\"env(safe-area-inset-bottom)\" box-sizing=\"border-box\"\n      box testId=\"fill\" width=\"100%\" height=\"100%\"\n"
    )
}

#[test]
fn a_page_that_does_not_ask_for_the_edges_is_laid_out_inside_them() {
    let mut p = boot(&page(""));
    assert!(p.set_window(phone()).is_none());
    // The bars and the cutout, whichever reaches further: 30 above, 16 below.
    assert_eq!(p.chrome().frame, [0., 30., 400., 754.]);
    assert_eq!(frame_of(&p, "root"), (0., 0., 400., 754.));
    // Nothing is applied twice: the page's own `env()` padding is zero here.
    assert_eq!(frame_of(&p, "fill"), (0., 0., 400., 754.));
    let env = env(&mut p);
    assert_eq!(env["safe-area-inset-top"], 0);
    assert_eq!(env["safe-area-inset-bottom"], 0);
    assert_eq!(env["keyboard-inset-height"], 0);
}

#[test]
fn a_page_that_covers_the_window_is_told_the_insets() {
    let mut p = boot(&page("viewport-fit=\"cover\""));
    assert!(p.set_window(phone()).is_none());
    assert_eq!(p.chrome().frame, [0., 0., 400., 800.]);
    assert_eq!(frame_of(&p, "root"), (0., 0., 400., 800.));
    assert_eq!(frame_of(&p, "fill"), (0., 30., 400., 754.));
    let env = env(&mut p);
    assert_eq!(env["safe-area-inset-top"], 30);
    assert_eq!(env["safe-area-inset-bottom"], 16);
    // A rotation: the bars move, and the page is laid out again.
    let turned = Window {
        size: (800., 400.),
        bars: [24., 0., 0., 0.],
        cutout: [0., 0., 0., 30.],
        ..phone()
    };
    assert!(p.set_window(turned).is_none());
    assert_eq!(frame_of(&p, "fill"), (0., 24., 800., 376.));
}

#[test]
fn the_keyboard_ends_the_viewport_at_its_top() {
    for (root, without, with) in [
        // Inside the bars: 754 tall from 30; the keyboard covers 284 of it.
        ("", [0., 30., 400., 754.], [0., 30., 400., 470.]),
        // Covering the window: the keyboard covers 300 of 800.
        (
            "viewport-fit=\"cover\"",
            [0., 0., 400., 800.],
            [0., 0., 400., 500.],
        ),
    ] {
        let mut p = boot(&page(root));
        assert!(p.set_window(phone()).is_none());
        assert_eq!(p.chrome().frame, without, "{root}");
        let typing = Window {
            keyboard: [0., 0., 300., 0.],
            ..phone()
        };
        assert!(p.set_window(typing).is_none());
        assert_eq!(p.chrome().frame, with, "{root}");
        let overlap = without[1] + without[3] - 500.;
        let env = env(&mut p);
        assert_eq!(env["keyboard-inset-height"], overlap, "{root}");
        // The keyboard's edge has no inset of its own.
        assert_eq!(env["safe-area-inset-bottom"], 0, "{root}");
        assert_eq!(frame_of(&p, "root").3, with[3], "{root}");
        // It goes: the page has its window back.
        assert!(p.set_window(phone()).is_none());
        assert_eq!(p.chrome().frame, without, "{root}");
        assert_eq!(self::env(&mut p)["keyboard-inset-height"], 0, "{root}");
    }
}

#[test]
fn a_page_the_keyboard_overlays_is_not_laid_out_again() {
    let mut p = boot(&page(
        "viewport-fit=\"cover\" interactive-widget=\"overlays-content\"",
    ));
    let typing = Window {
        keyboard: [0., 0., 300., 0.],
        ..phone()
    };
    assert!(p.set_window(typing).is_none());
    assert_eq!(p.chrome().frame, [0., 0., 400., 800.]);
    let env = env(&mut p);
    assert_eq!(env["keyboard-inset-height"], 300);
    assert_eq!(env["safe-area-inset-bottom"], 16);
}

#[test]
fn the_field_being_edited_stays_above_the_keyboard() {
    let rows: String = (0..20)
        .map(|i| format!("        box height=60 width=200 testId=\"r{i}\"\n"))
        .collect();
    let mut p = boot(&format!(
        "component App\n  view\n    main testId=\"root\" width=\"100%\" height=\"100%\" viewport-fit=\"cover\" display=\"flex\" flex-direction=\"column\"\n      scroll testId=\"port\" flex=1 width=\"100%\"\n        column\n{rows}        input testId=\"note\" width=200 height=40\n"
    ));
    assert!(p.set_window(phone()).is_none());
    let id = |p: &Presenter<Nothing>, name: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let note = id(&p, "note");
    // Scrolled so the field shows at the bottom of the window, and focused.
    p.wheel_at(100., 400., 0., 10_000.);
    assert!(p.set_focus(Some(note), 0.).is_none());
    let before = p.box_of(note).unwrap().rect;
    assert!(
        before.1 + before.3 <= 800.0 && before.1 > 500.0,
        "{before:?}"
    );
    let typing = Window {
        keyboard: [0., 0., 300., 0.],
        ..phone()
    };
    assert!(p.set_window(typing).is_none());
    assert_eq!(p.viewport, (400., 500.));
    let after = p.box_of(note).unwrap().rect;
    assert!(
        after.1 >= 0.0 && after.1 + after.3 <= 500.0,
        "the field is in what the keyboard leaves: {after:?}"
    );
}

/// On a phone a picture is shown (frames are submitted and acknowledged),
/// and the boxes a reveal goes by are the last picture's, painted before
/// the keyboard rose: the field is kept in view once the page has been
/// painted in the viewport the keyboard left (seen on a Pixel, where the
/// field had stayed under the keyboard).
#[test]
fn under_a_shown_picture_the_field_rises_with_the_paint_after_the_keyboard() {
    let rows: String = (0..20)
        .map(|i| format!("        box height=60 width=200 testId=\"r{i}\"\n"))
        .collect();
    let mut p = boot(&format!(
        "component App\n  view\n    main testId=\"root\" width=\"100%\" height=\"100%\" viewport-fit=\"cover\" display=\"flex\" flex-direction=\"column\"\n      scroll testId=\"port\" flex=1 width=\"100%\"\n        column\n{rows}        input testId=\"note\" width=200 height=40\n"
    ));
    assert!(p.set_window(phone()).is_none());
    let shown = |p: &mut Presenter<Nothing>| {
        let frame = p.display_frame().expect("a frame");
        assert!(p.display_complete(&frame));
    };
    let note = {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id("note")[0]).unwrap().id
    };
    shown(&mut p);
    p.wheel_at(100., 400., 0., 10_000.);
    assert!(p.set_focus(Some(note), 0.).is_none());
    shown(&mut p);
    let typing = Window {
        keyboard: [0., 0., 300., 0.],
        ..phone()
    };
    assert!(p.set_window(typing).is_none());
    assert_eq!(p.viewport, (400., 500.));
    // The picture in the new viewport, whose boxes show the field under
    // the keyboard; then the one the reveal asked for.
    shown(&mut p);
    assert!(p.dirty(), "the reveal scrolled the list");
    shown(&mut p);
    let after = p.box_of(note).unwrap().rect;
    assert!(
        after.1 >= 0.0 && after.1 + after.3 <= 500.0,
        "the field is in what the keyboard leaves: {after:?}"
    );
}

#[test]
fn the_colour_behind_the_page_is_the_roots_or_the_platforms_surface() {
    // The author's opaque root background reaches the edges, in any scheme.
    let mut p = boot(&page("background-color=\"#102030\""));
    assert_eq!(p.chrome().background, [0x10, 0x20, 0x30, 0xff]);
    assert!(!p.chrome().dark);
    // Unsaid, it is the platform's surface for the scheme: never white at night.
    let mut q = boot(&page(""));
    assert_eq!(q.chrome().background, [255; 4]);
    q.set_system_scheme(true);
    assert_eq!(q.chrome().background, crate::paint::page_color(true));
    assert_ne!(q.chrome().background, [255; 4]);
    assert!(q.chrome().dark);
    // The app's own scheme decides what the bars' icons are drawn over.
    p.set_system_scheme(true);
    p.app_scheme(Some(false));
    assert!(!p.chrome().dark);
}

#[test]
fn an_unset_background_is_painted_in_the_schemes_surface() {
    let mut p =
        boot("component App\n  view\n    column width=100 height=100\n      text \"hello\"\n");
    let corner = |p: &mut Presenter<Nothing>| {
        let c = p.frame().pixel(399, 799).unwrap().demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    };
    assert_eq!(corner(&mut p), [255; 4], "a light system's page is white");
    p.set_system_scheme(true);
    assert_eq!(corner(&mut p), crate::paint::page_color(true));
    assert_ne!(corner(&mut p), [255; 4], "and a dark one's is not");
    p.set_system_scheme(false);
    assert_eq!(corner(&mut p), [255; 4]);
}

#[test]
fn a_window_that_is_no_window_is_refused() {
    let mut p = boot(&page(""));
    let none = Window {
        size: (0., 800.),
        ..phone()
    };
    assert!(p.set_window(none).is_some());
    assert!(p.window().is_none());
    assert_eq!(p.chrome().frame, [0., 0., 400., 800.]);
}

const SWIPE_ROW: &str = r#"component App
  state count = 0
  action reply
    count = count + 1
  view
    column width=400 height=500
      box testId="row" width=400 height=100 swiperight=reply touch-action="pan-y" transition="translate -exact-spring(300, 30, 1)"
        text "swipe me"
      text `${count}` testId="count"
"#;

/// A swipe of 100 points to the right along the row, from `x`: how many
/// times its `swiperight` has fired after it.
fn swiped(p: &mut Presenter<Nothing>, x: f32, at: f64) -> String {
    let _ = p.pointer_down(x, 40., at).unwrap();
    for step in 1..=10 {
        let _ = p.pointer_move(x + 10. * step as f32, 40., at + step as f64 * 10.);
    }
    let _ = p.pointer_up(x + 100., 40., at + 110.);
    let k = p.host().kernel();
    let count = k.node_by_key(k.find_by_test_id("count")[0]).unwrap();
    count
        .props
        .str(PropId::Text)
        .unwrap_or_default()
        .to_string()
}

#[test]
fn an_authored_swipe_yields_the_edges_the_systems_gesture_starts_from() {
    // No shell told a window (desktop Linux): no system gesture, no edge.
    let mut p = boot(SWIPE_ROW);
    assert_eq!(swiped(&mut p, 5., 0.), "1");
    // A phone with gesture navigation: 12 points each side are Back's.
    let mut p = boot(SWIPE_ROW);
    assert!(p.set_window(phone()).is_none());
    assert!(p.in_gesture_edge(11.) && !p.in_gesture_edge(12.));
    assert!(p.in_gesture_edge(388.) && !p.in_gesture_edge(387.));
    assert_eq!(swiped(&mut p, 5., 0.), "0", "the system's edge: no swipe");
    assert_eq!(swiped(&mut p, 20., 1000.), "1", "past it: the row's");
    // A phone with a button for Back has no such edge: the row swipes
    // from the very side, where a fixed width would have refused it.
    let mut p = boot(SWIPE_ROW);
    let buttons = Window {
        gestures: [30., 0., 48., 0.],
        ..phone()
    };
    assert!(p.set_window(buttons).is_none());
    assert_eq!(swiped(&mut p, 5., 0.), "1");
}
