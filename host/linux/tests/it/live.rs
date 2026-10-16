//! The live table on Linux (LLP 1121 D13, stage 4): a batch or snapshot from
//! the dev server sets a running build's rows in place and paints, with no
//! restart (its state stays) and no layout for colours.

use exact_linux::presenter::{PainterChoice, Presenter};
use exact_runner::design;

#[test]
fn a_running_build_paints_a_batch_in_place() {
    let dir = std::env::temp_dir().join(format!("exact-linux-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let view = "component App\n  state n = 0\n  action bump\n    n = n + 1\n  view\n    column width=200 height=100 background-color=\"page\"\n      button press=bump testId=\"bump\"\n        text `${n}` testId=\"count\"\n      box width=40 height=20 background-color=\"ink\" testId=\"swatch\"\n";
    let plan = |tamber: &str| {
        std::fs::write(dir.join("app.tamber"), tamber).unwrap();
        std::fs::write(dir.join("app.contract"), view).unwrap();
        contract::compile_path(&dir.join("app.contract")).unwrap()
    };
    let a = plan("tamber t\n\ntheme\n  ink = #111111\n  page = #ffffff\n");
    let b = plan("tamber t\n\ntheme\n  ink = #ff0000\n  page = #ffffff\n");
    let _ = std::fs::remove_dir_all(&dir);
    let batch =
        design::batch(&design::shape(&a), &design::tokens(&a), &design::tokens(&b)).unwrap();
    let (mut p, err) = Presenter::boot_with(
        &a.encode(),
        (),
        (200., 100.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    let id = |p: &Presenter<()>, t: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
    };
    let (bump, count, swatch) = (id(&p, "bump"), id(&p, "count"), id(&p, "swatch"));
    p.tap(bump).unwrap();
    let at = |p: &mut Presenter<()>| {
        let f = p.host().kernel().node(swatch).unwrap().frame;
        let shot = p.frame();
        let c = shot.pixel(f.x as u32 + 5, f.y as u32 + 5).unwrap();
        ([c.red(), c.green(), c.blue()], f)
    };
    let (before, frame) = at(&mut p);
    assert_eq!(before, [0x11, 0x11, 0x11]);
    assert_eq!(p.apply_live(&batch.json()), None);
    let (after, moved) = at(&mut p);
    assert_eq!(after, [0xff, 0, 0]);
    assert_eq!(moved, frame, "a colour moves no geometry");
    let k = p.host().kernel();
    assert_eq!(
        k.node(count).unwrap().props.str(exact_kernel::PropId::Text),
        Some("1"),
        "no restart"
    );
    // Its base is past now; a snapshot of the first design applies whole.
    assert_eq!(p.apply_live(&batch.json()).as_deref(), Some("stale"));
    let back = design::snapshot(&design::shape(&a), &design::tokens(&a));
    assert_eq!(p.apply_live(&back.json()), None);
    assert_eq!(at(&mut p).0, [0x11, 0x11, 0x11]);
}
