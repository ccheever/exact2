//! The Material look's controls (LLP 1115.001 D9, S2): a checkbox takes
//! Material's size and colours. The
//! look is the process's (`app::show_material_look`), so these run in a
//! child process of their own.
use super::*;

const APP: &str = r#"component App
  view
    column padding=10 align-items="flex-start"
      input type="checkbox" checked=true testId="on"
      input type="checkbox" testId="off"
      input type="checkbox" checked=true disabled=true testId="faded"
"#;

/// Whether this is the child process that shows the Material look; the
/// parent runs the test there and returns false.
fn material_process() -> bool {
    const CHILD: &str = "EXACT_MATERIAL_LOOK_TEST";
    if std::env::var_os(CHILD).is_some() {
        crate::app::show_material_look();
        return true;
    }
    let name = std::thread::current().name().unwrap().to_owned();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

fn boot() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (400., 600.),
        2.4375,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// The colour at a point of the frame, in points.
fn pixel(p: &mut Presenter<()>, x: f32, y: f32) -> [u8; 4] {
    let frame = p.frame();
    let at = |v: f32| (v * 2.4375) as u32;
    let c = frame.pixel(at(x), at(y)).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn materials_checkbox_takes_its_size_and_colours() {
    if !material_process() {
        return;
    }
    let mut p = boot();
    let on = p.rect_of(id(&p, "on")).unwrap();
    assert_eq!(
        (on.2, on.3),
        (20.0, 20.0),
        "the mark-size row's Material 20"
    );
    // Checked: the accent's fill above the check; unchecked: the page shows
    // inside its outline.
    let accent = crate::paint::control::accents(None, false).0;
    assert_eq!(pixel(&mut p, on.0 + 10.0, on.1 + 3.0), accent);
    let off = p.rect_of(id(&p, "off")).unwrap();
    let page = pixel(&mut p, 2.0, 2.0);
    assert_eq!(pixel(&mut p, off.0 + 10.0, off.1 + 10.0), page);
    assert_ne!(
        pixel(&mut p, off.0 + 0.5, off.1 + 10.0),
        page,
        "its outline"
    );
    // Disabled: no longer the accent.
    let faded = p.rect_of(id(&p, "faded")).unwrap();
    assert_ne!(pixel(&mut p, faded.0 + 10.0, faded.1 + 3.0), accent);
    // A size reported before (a switch that became a checkbox) is forgotten.
    let view = id(&p, "off");
    assert!(p.host.set_intrinsic(view, Some((38.0, 22.0))).is_none());
    p.size_controls();
    p.frame();
    let off = p.rect_of(view).unwrap();
    assert_eq!((off.2, off.3), (20.0, 20.0));
}
