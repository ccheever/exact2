//! A role drawn in the accent (`AccentColor`, `LinkText`, `-exact-link`) reads
//! the nearest `accent-color` its node or an ancestor authored, as Apple's
//! tint and the web's `var(--exact-accent, AccentColor)` do; with none it is
//! the app's tint; `accent-color="AccentColor"` keeps the one around. Before,
//! the painter resolved it with no node, so it was always the tint (QUEUE,
//! LLP 1121 stage 1's review).

use exact_linux::presenter::{PainterChoice, Presenter};

#[test]
fn a_role_drawn_in_the_accent_reads_the_nearest_authored_one() {
    let plan = contract::compile(r##"component App
  view
    column width=400 height=300 background-color="white" align-items="flex-start" gap=4
      box width=40 height=20 background-color="AccentColor" testId="tint"
      column accent-color="#00c800" gap=4
        box width=40 height=20 background-color="AccentColor" testId="authored"
        box width=40 height=20 border-width=6 border-style="solid" border-color="-exact-link" background-color="white" testId="link"
        box width=40 height=20 background-color="-exact-secondary-label" testId="other"
        column accent-color="#c80000"
          box width=40 height=20 background-color="AccentColor" testId="nearer"
        column accent-color="AccentColor"
          box width=40 height=20 background-color="AccentColor" testId="kept"
      box width=40 height=20 background-color="AccentColor" testId="after"
"##).unwrap();
    let (mut p, err) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 300.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    let shot = p.frame();
    let at = |p: &Presenter<()>, name: &str, dx: u32| {
        let k = p.host().kernel();
        let f = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().frame;
        let c = shot.pixel(f.x as u32 + dx, f.y as u32 + 2).unwrap();
        [c.red(), c.green(), c.blue(), c.alpha()]
    };
    let tint = at(&p, "tint", 20);
    assert_ne!(tint, [0, 200, 0, 255], "no accent authored: the app's tint");
    assert_eq!(at(&p, "authored", 20), [0, 200, 0, 255]);
    assert_eq!(
        at(&p, "link", 2),
        [0, 200, 0, 255],
        "a link's role is the accent too"
    );
    assert_ne!(
        at(&p, "other", 20),
        [0, 200, 0, 255],
        "a role not drawn in the accent"
    );
    assert_eq!(
        at(&p, "nearer", 20),
        [200, 0, 0, 255],
        "the nearest authored one"
    );
    assert_eq!(
        at(&p, "kept", 20),
        [0, 200, 0, 255],
        "accent-color=AccentColor keeps the one around"
    );
    assert_eq!(
        at(&p, "after", 20),
        tint,
        "a sibling after the authored subtree"
    );
}

/// A spinner whose `accent-color` is the `AccentColor` role under an authored
/// accent paints in that accent, as the web's `var(--exact-accent,
/// AccentColor)` keeps it (LLP 1121 D9, stage 9 F1); with none authored, the
/// role's own (the design's, else the platform's).
#[test]
fn a_spinner_under_an_authored_accent_keeps_it_through_the_role() {
    let plan = contract::compile(r##"component App
  view
    column width=400 height=300 background-color="white" align-items="flex-start" gap=4
      column accent-color="#00c800"
        column accent-color="AccentColor"
          progress width=44 height=44 testId="kept"
      column accent-color="AccentColor"
        progress width=44 height=44 testId="role"
"##)
    .unwrap();
    let (mut p, err) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 300.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    let shot = p.frame();
    let green = |p: &Presenter<()>, name: &str| {
        let k = p.host().kernel();
        let f = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().frame;
        (f.y as u32..(f.y + f.height) as u32).any(|y| {
            (f.x as u32..(f.x + f.width) as u32).any(|x| {
                let c = shot.pixel(x, y).unwrap();
                c.red() < 40 && c.green() > 170 && c.blue() < 40
            })
        })
    };
    assert!(green(&p, "kept"), "the authored accent, kept through the role");
    assert!(!green(&p, "role"), "no accent authored: the role's own");
}
