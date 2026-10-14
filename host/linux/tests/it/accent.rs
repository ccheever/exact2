//! A role drawn in the accent (`AccentColor`, `LinkText`, `-exact-link`) reads
//! the nearest `accent-color` its node or an ancestor authored, as Apple's
//! tint and the web's `var(--exact-accent, AccentColor)` do; with none it is
//! the app's tint. Before, the painter resolved it with no node, so it was
//! always the tint (QUEUE, LLP 1121 stage 1's review).

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
        at(&p, "after", 20),
        tint,
        "a sibling after the authored subtree"
    );
}
