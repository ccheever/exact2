//! LLP 1021 D2: a popover opened at the press. The button's `press` hears
//! where the pointer was and writes it as the popover's `left`/`top`; the
//! neutral surface opens there, held in the window near its edge, and a
//! key's press (at 0, 0) leaves it at its invoker. Each press reads its own
//! path's point, never an earlier press's.
use super::*;
use exact_runner::Value;

const APP: &str = r#"component App
  state x = 0
  state y = 0
  state pointed = false
  action open(e: MouseEvent)
    pointed = e.clientX != 0 || e.clientY != 0
    x = e.clientX
    y = e.clientY
    showPopover("pop", source="open")
  view
    column width="100%" height="100%"
      button press=open id="open" testId="open" width=460 height=300
        text "Open"
      column id="pop" testId="pop" popover="auto" margin=0 width=120 height=80 left=(pointed ? `${x}px` : "auto") top=(pointed ? `${y}px` : "auto")
        text "Inside"
"#;

fn boot() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (480., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_overlays();
    p.frame();
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// A click at (`x`, `y`) and where the popover it opened is drawn.
fn click(p: &mut Presenter<()>, x: f32, y: f32, at: f64) -> (f32, f32, f32, f32) {
    p.pointer_down(x, y, at).unwrap();
    p.pointer_up(x, y, at + 50.).unwrap();
    p.run_commands(|| ());
    p.frame();
    p.rect_of(id(p, "pop")).expect("the popover is drawn")
}

#[test]
fn a_popover_opens_at_the_press_and_stays_in_the_window_by_its_edge() {
    let mut p = boot();
    let (x, y, ..) = click(&mut p, 30., 8., 0.);
    assert_eq!(p.host.runner().slot("x"), Some(&Value::Number(30.)));
    assert_eq!(p.host.runner().slot("y"), Some(&Value::Number(8.)));
    assert_eq!((x, y), (30., 8.));
    p.close_surface(200.);
    p.frame();
    // By the bottom-right corner: held inside the window.
    let (x, y, w, h) = click(&mut p, 450., 290., 400.);
    assert!(x + w <= 480. && y + h <= 400., "{x},{y} {w}x{h}");
    assert_eq!((x, y), (480. - w, 290.));
}

#[test]
fn a_keys_press_leaves_it_at_its_invoker() {
    let mut p = boot();
    let open = id(&p, "open");
    p.dispatch_press(open, 0., false);
    p.run_commands(|| ());
    p.frame();
    assert_eq!(p.host.runner().slot("pointed"), Some(&Value::Bool(false)));
    assert_eq!(p.press_event(false), Event::Press);
}

/// A finger's press (the contact's own activation, not the mouse's click)
/// carries where it lifted, not where the mouse last clicked; a key's after
/// it carries none.
#[test]
fn a_touch_carries_its_own_point_and_a_key_none() {
    let mut p = boot();
    click(&mut p, 30., 8., 0.);
    p.close_surface(100.);
    p.frame();
    p.set_pointer_kind(crate::presenter::PointerKind::Touch);
    p.pointer_down(200., 120., 200.).unwrap();
    p.pointer_up(200., 120., 250.).unwrap();
    p.run_commands(|| ());
    assert_eq!(p.host.runner().slot("x"), Some(&Value::Number(200.)));
    assert_eq!(p.host.runner().slot("y"), Some(&Value::Number(120.)));
    p.close_surface(300.);
    let open = id(&p, "open");
    p.dispatch_press(open, 400., false);
    p.run_commands(|| ());
    assert_eq!(p.host.runner().slot("pointed"), Some(&Value::Bool(false)));
}

/// Under an authored `position-area` CSS lets the area win: the popover is
/// placed against its invoker, not at the press.
#[test]
fn an_authored_position_area_wins_over_the_insets() {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&APP.replace(
            "margin=0 width=120",
            "margin=0 position-area=\"center\" width=120",
        )),
        (),
        (480., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_overlays();
    p.frame();
    let (x, y, w, h) = click(&mut p, 30., 8., 0.);
    let open = p.rect_of(id(&p, "open")).unwrap();
    assert_eq!(
        (x + w / 2., y + h / 2.),
        (open.0 + open.2 / 2., open.1 + open.3 / 2.),
        "centred on the invoker"
    );
}
