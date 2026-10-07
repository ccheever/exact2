//! LLP 1104 D6–D7: host chrome and focus, never compiled default rows.
use exact_kernel::{Appearance, Frame, StyleMask, ViewId};
use exact_linux::{presenter::PainterChoice, Presenter};

fn boot() -> Presenter<()> {
    let plan = contract::compile(
        r##"component App
  view
    column width=400 height=600 background-color="#202020" color="#33aa55" font-size=20
      input width=100 height=30 testId="native" accent-color="#ff0000"
      input width=100 height=30 disabled=true testId="disabled"
      input width=100 height=30 appearance="none" testId="bare"
      input width=100 height=30 appearance="none" background-color="#0000ff" testId="authored"
      input width=100 height=30 padding=3 testId="padded"
      textarea width=100 rows=2 testId="area"
      input type="password" width=100 height=30 value="secret" testId="password"
      input type="search" width=100 height=30 testId="search"
      input type="email" width=100 height=30 testId="email"
      input type="number" width=100 height=30 testId="number"
"##,
    )
    .unwrap();
    let (p, err) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 600.),
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
fn native_geometry_uses_chrome_and_author_padding_without_changing_inheritance() {
    let p = boot();
    let k = p.host().kernel();
    assert!(k.env().control_text_styles.is_none());
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
        assert_eq!(n.computed_style(StyleMask::INHERITED).font_size, 20.0);
        assert_eq!(n.text_color().resolve(false).g(), 0xaa);
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
fn native_paint_has_light_and_dark_chrome_radius_disabled_opacity_and_accent_focus() {
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
        let disabled = pixel(&mut p, "disabled", 50, 4);
        for i in 0..3 {
            let expected = (fill[i] as i16 + 32) / 2;
            assert!((disabled[i] as i16 - expected).abs() <= 1, "{disabled:?}");
        }
        assert_eq!(pixel(&mut p, "bare", 50, 4), [32, 32, 32, 255]);
        assert_eq!(pixel(&mut p, "authored", 50, 4), [0, 0, 255, 255]);
    }
    let native = id(&p, "native");
    p.tap(native).unwrap();
    assert_eq!(pixel(&mut p, "native", 50, 0), [255, 0, 0, 255]);
    // An empty field's caret begins at the published content rect.
    let f = frame(&p, "native");
    let shot = p.frame();
    let c = shot.pixel(f.x as u32 + 9, f.y as u32 + 20).unwrap();
    assert_eq!([c.red(), c.green(), c.blue()], [0x33, 0xaa, 0x55]);
}
