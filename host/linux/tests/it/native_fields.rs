//! LLP 1104 D6–D7: host chrome and focus, never compiled default rows.
use exact_kernel::{Appearance, Frame, StyleMask, ViewId};
use exact_linux::{presenter::PainterChoice, Presenter};

fn boot() -> Presenter<()> {
    let plan = contract::compile(
        r##"component App
  view
    column width=400 height=800 background-color="#202020" color="#33aa55" font-size=20
      input width=100 height=30 testId="native" accent-color="#ff0000"
      input width=100 height=30 disabled=true value="Disabled" testId="disabled"
      input width=100 height=30 appearance="none" testId="bare"
      input width=100 height=30 appearance="none" background-color="#0000ff" testId="authored"
      input width=100 height=30 padding=3 testId="padded"
      textarea width=100 rows=2 testId="area"
      input type="password" width=100 height=30 value="secret" testId="password"
      input type="search" width=100 height=30 testId="search"
      input type="email" width=100 height=30 testId="email"
      input type="number" width=100 height=30 testId="number"
      input width=100 height=30 disabled=true value="Disabled" color="black" testId="disabled-authored"
      column color="white" font-size=32 font-style="italic" letter-spacing=4
        input width=100 height=30 value="Visible" testId="contrast"
        textarea width=100 rows=1 value="Visible" testId="contrast-area"
"##,
    )
    .unwrap();
    let (p, err) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 800.),
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

fn frame(p: &Presenter<()>, name: &str) -> Frame {
    p.host().kernel().node(id(p, name)).unwrap().frame
}

fn pixel(p: &mut Presenter<()>, name: &str, x: u32, y: u32) -> [u8; 4] {
    let f = frame(p, name);
    let shot = p.frame();
    let c = shot.pixel(f.x as u32 + x, f.y as u32 + y).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn native_geometry_uses_chrome_and_stops_ancestor_typography() {
    let p = boot();
    let k = p.host().kernel();
    assert!(k.env().control_text_styles.is_some());
    assert_eq!(k.provisional_layouts(), 0);
    for name in [
        "native", "disabled", "password", "search", "email", "number",
    ] {
        let n = k.node(id(&p, name)).unwrap();
        assert_eq!(n.style.appearance, Appearance::Auto);
        assert!(!n.style.mask.has(exact_kernel::StyleId::BackgroundColor));
        assert_eq!(n.style.opacity, 1.0, "disabled is host paint");
        assert_eq!((n.frame.width, n.frame.height), (118.0, 44.0));
        assert_eq!(
            n.field_content_rect(),
            Some(Frame {
                x: 9.0,
                y: 7.0,
                width: 100.0,
                height: 30.0
            })
        );
        assert_eq!(n.computed_style(StyleMask::INHERITED).font_size, 16.0);
        assert_eq!(n.text_color().resolve(false).g(), 0);
    }
    let padded = k.node(id(&p, "padded")).unwrap();
    assert_eq!((padded.frame.width, padded.frame.height), (124.0, 50.0));
    assert_eq!(padded.field_content_rect().unwrap().x, 12.0);
    assert_eq!(padded.field_content_rect().unwrap().y, 10.0);
    let area = k.node(id(&p, "area")).unwrap();
    let content = area.field_content_rect().unwrap();
    assert_eq!(area.frame.height, content.height + 14.0);
    assert_eq!(content.width, 100.0);
    for name in ["bare", "authored"] {
        let n = k.node(id(&p, name)).unwrap();
        assert!(n.field_content_rect().is_none());
        assert_eq!((n.frame.width, n.frame.height), (100.0, 30.0));
    }
}

#[test]
fn native_paint_keeps_disabled_fill_and_treats_border_and_ink() {
    let mut p = boot();
    for (dark, border, fill) in [
        (false, [198, 198, 200, 255], [255, 255, 255, 255]),
        (true, [72, 72, 74, 255], [28, 28, 30, 255]),
    ] {
        p.set_system_scheme(dark);
        assert_eq!(pixel(&mut p, "native", 50, 0), border);
        assert_eq!(pixel(&mut p, "native", 50, 4), fill);
        assert_eq!(
            pixel(&mut p, "native", 0, 0),
            [32, 32, 32, 255],
            "rounded corner"
        );
        assert_eq!(pixel(&mut p, "disabled", 50, 4), fill);
        let disabled = pixel(&mut p, "disabled", 50, 0);
        for i in 0..3 {
            assert_eq!(disabled[i], ((border[i] as u16 + fill[i] as u16) / 2) as u8);
        }
        assert_eq!(pixel(&mut p, "bare", 50, 4), [32, 32, 32, 255]);
        assert_eq!(pixel(&mut p, "authored", 50, 4), [0, 0, 255, 255]);
    }
    p.set_system_scheme(false);
    let native = id(&p, "native");
    p.tap(native).unwrap();
    assert_eq!(pixel(&mut p, "native", 50, 0), [255, 0, 0, 255]);
    // An empty field's caret begins at the published content rect.
    let f = frame(&p, "native");
    let shot = p.frame();
    let c = shot.pixel(f.x as u32 + 9, f.y as u32 + 20).unwrap();
    assert_eq!([c.red(), c.green(), c.blue()], [0, 0, 0]);
}

#[test]
fn focused_bare_fields_ring_the_outer_box() {
    let mut p = boot();
    for name in ["bare", "authored"] {
        p.tap(id(&p, name)).unwrap();
        assert_eq!(pixel(&mut p, name, 50, 0), [0, 0x75, 0xff, 255], "{name}");
    }
}

#[test]
fn native_value_under_white_ancestor_uses_host_font_and_visible_ink() {
    let mut p = boot();
    for name in ["contrast", "contrast-area"] {
        let n = p.host().kernel().node(id(&p, name)).unwrap();
        let s = n.computed_style(StyleMask::INHERITED);
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.font_style, exact_kernel::FontStyle::Normal);
        assert_eq!(s.letter_spacing, 0.0);
        assert_eq!(n.text_color().resolve(false).g(), 0);
        let f = frame(&p, name);
        let shot = p.frame();
        assert!(
            (10..95).any(|x| (8..35).any(|y| {
                let c = shot.pixel(f.x as u32 + x, f.y as u32 + y).unwrap();
                c.red() < 100 && c.green() < 100 && c.blue() < 100
            })),
            "{name} must paint visible text on the white fill"
        );
    }
}

#[test]
fn disabled_native_ink_dims_but_authored_color_wins() {
    let mut p = boot();
    for (name, darkest) in [("disabled", 127), ("disabled-authored", 0)] {
        let f = frame(&p, name);
        let shot = p.frame();
        let red = (9..109)
            .flat_map(|x| (8..36).map(move |y| (x, y)))
            .map(|(x, y)| shot.pixel(f.x as u32 + x, f.y as u32 + y).unwrap().red())
            .min()
            .unwrap();
        assert_eq!(red, darkest, "{name}");
    }
}
