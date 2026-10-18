//! LLP 1104 D6–D7: host chrome and focus, never compiled default rows. Off
//! the neutral look the field is Material's filled text field (LLP
//! 1115.001.000 D6).
use exact_kernel::style::roles;
use exact_kernel::{Appearance, ColorValue, Frame, StyleMask, ViewId};
use exact_linux::{presenter::PainterChoice, Presenter};

fn boot() -> Presenter<()> {
    let plan = contract::compile(
        r##"component App
  view
    column width=400 height=1600 background-color="#202020" color="#33aa55" font-size=20
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
      input width=100 height=30 appearance="none" outline="none" testId="bare-quiet"
      input width=100 height=30 outline="none" testId="native-quiet"
      textarea width=100 rows=2 outline="none" testId="area-quiet"
"##,
    )
    .unwrap();
    let (p, err) = Presenter::boot_with(
        &plan.encode(),
        (),
        // Tall enough that no field shrinks below its own height.
        (400., 1600.),
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
        // Material's 16 points in on every side.
        assert_eq!((n.frame.width, n.frame.height), (132.0, 62.0));
        assert_eq!(
            n.field_content_rect(),
            Some(Frame {
                x: 16.0,
                y: 16.0,
                width: 100.0,
                height: 30.0
            })
        );
        assert_eq!(n.computed_style(StyleMask::INHERITED).font_size, 16.0);
        assert_eq!(n.text_color().resolve(false).g(), 0);
    }
    let padded = k.node(id(&p, "padded")).unwrap();
    assert_eq!((padded.frame.width, padded.frame.height), (138.0, 68.0));
    assert_eq!(padded.field_content_rect().unwrap().x, 19.0);
    assert_eq!(padded.field_content_rect().unwrap().y, 19.0);
    let area = k.node(id(&p, "area")).unwrap();
    let content = area.field_content_rect().unwrap();
    assert_eq!(area.frame.height, content.height + 32.0);
    assert_eq!(content.width, 100.0);
    for name in ["bare", "authored"] {
        let n = k.node(id(&p, name)).unwrap();
        assert!(n.field_content_rect().is_none());
        assert_eq!((n.frame.width, n.frame.height), (100.0, 30.0));
    }
}

/// A role's colour under a scheme, as eight-bit RGBA.
fn role(name: &str, dark: bool) -> [u8; 4] {
    let c = ColorValue::Role(roles::role(name).unwrap()).resolve(dark);
    [c.r(), c.g(), c.b(), c.a()]
}

/// `top` over an opaque `under`, as the painter blends it (to within one).
fn over(top: [u8; 4], under: [u8; 4]) -> [u8; 4] {
    let a = top[3] as f32 / 255.0;
    let mix = |t: u8, u: u8| (t as f32 * a + u as f32 * (1.0 - a)).round() as u8;
    [
        mix(top[0], under[0]),
        mix(top[1], under[1]),
        mix(top[2], under[2]),
        255,
    ]
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    (0..4).all(|i| a[i].abs_diff(b[i]) <= 1)
}

#[test]
fn native_paint_is_materials_filled_field() {
    let mut p = boot();
    let page = [32, 32, 32, 255];
    let h = frame(&p, "native").height as u32;
    for dark in [false, true] {
        p.set_system_scheme(dark);
        let fill = over(role("Field", dark), page);
        assert_eq!(pixel(&mut p, "native", 50, 4), fill);
        assert_eq!(pixel(&mut p, "native", 0, 0), page, "a rounded top corner");
        assert_ne!(pixel(&mut p, "native", 0, h - 1), page, "a square foot");
        // The indicator: one point of `onSurfaceVariant` along the foot.
        let line = over(role("-exact-secondary-label", dark), fill);
        assert!(near(pixel(&mut p, "native", 50, h - 1), line));
        assert_eq!(pixel(&mut p, "native", 50, h - 2), fill);
        // Disabled: the enabled container, as Compose Material 3 draws it
        // (`TextFieldDefaults.colors`), and its foot `onSurface` at 38 %.
        let off = fill;
        assert!(near(pixel(&mut p, "disabled", 50, 4), off));
        assert!(near(
            pixel(&mut p, "disabled", 50, h - 1),
            over(role("GrayText", dark), off)
        ));
        assert_eq!(pixel(&mut p, "bare", 50, 4), page);
        assert_eq!(pixel(&mut p, "authored", 50, 4), [0, 0, 255, 255]);
    }
    p.set_system_scheme(false);
    let native = id(&p, "native");
    p.tap(native).unwrap();
    // Focused: two points of the accent (its `accent-color`) along the foot, no ring.
    assert_eq!(pixel(&mut p, "native", 50, h - 1), [255, 0, 0, 255]);
    assert_eq!(pixel(&mut p, "native", 50, h - 2), [255, 0, 0, 255]);
    assert_ne!(pixel(&mut p, "native", 50, 0), [255, 0, 0, 255]);
    // An empty field's caret begins at the published content rect.
    let f = frame(&p, "native");
    let shot = p.frame();
    let c = shot.pixel(f.x as u32 + 16, f.y as u32 + 30).unwrap();
    assert_eq!([c.red(), c.green(), c.blue()], [0, 0, 0]);
}

fn look_fade(mut c: [u8; 4], alpha: f32) -> [u8; 4] {
    c[3] = (c[3] as f32 * alpha).round() as u8;
    c
}

#[test]
fn focused_bare_fields_ring_the_outer_box() {
    let mut p = boot();
    for name in ["bare", "authored"] {
        p.tap(id(&p, name)).unwrap();
        assert_eq!(pixel(&mut p, name, 50, 0), [0, 0x75, 0xff, 255], "{name}");
    }
}

/// `outline="none"` takes the painted ring off a focused bare field; a
/// field without it keeps the ring. Material's field has no ring either way:
/// its focus is its indicator, which `outline` does not reach.
#[test]
fn outline_none_paints_no_ring_on_a_focused_field() {
    let mut p = boot();
    // The accent over the box's top edge, where a ring is and no indicator.
    let ring = |p: &mut Presenter<()>, name: &str| {
        p.tap(id(p, name)).unwrap();
        assert_eq!(p.focus(), Some(id(p, name)), "{name} takes the focus");
        let f = frame(p, name);
        let shot = p.frame();
        (0..f.width as u32 + 4)
            .flat_map(|x| (0..4).map(move |y| (x, y)))
            .filter_map(|(x, y)| {
                shot.pixel(
                    (f.x as u32 + x).saturating_sub(2),
                    (f.y as u32 + y).saturating_sub(2),
                )
            })
            .any(|c| [c.red(), c.green(), c.blue()] == [0, 0x75, 0xff])
    };
    assert!(ring(&mut p, "bare"), "bare keeps its ring");
    assert!(!ring(&mut p, "bare-quiet"), "bare-quiet paints no ring");
    for name in ["email", "native-quiet", "area", "area-quiet"] {
        assert!(!ring(&mut p, name), "{name} is Material's: no ring");
        let f = frame(&p, name);
        let shot = p.frame();
        let foot = shot
            .pixel(f.x as u32 + 50, (f.y + f.height) as u32 - 1)
            .unwrap();
        assert_eq!(
            [foot.red(), foot.green(), foot.blue()],
            [0, 0x75, 0xff],
            "{name}'s indicator"
        );
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
    // Material's disabled text is `onSurface` at 38 %: never the full ink.
    for (name, dimmed) in [("disabled", true), ("disabled-authored", false)] {
        let f = frame(&p, name);
        let shot = p.frame();
        let red = (16..116)
            .flat_map(|x| (16..46).map(move |y| (x, y)))
            .map(|(x, y)| shot.pixel(f.x as u32 + x, f.y as u32 + y).unwrap().red())
            .min()
            .unwrap();
        assert_eq!(red > 0, dimmed, "{name}: {red}");
    }
}

#[test]
fn oversized_field_line_paints_at_the_kernel_baseline() {
    let plan = contract::compile(r##"component App
  view
    row align-items="baseline" padding-top=60 color="red" font-size=16 line-height="80px"
      input testId="field" value="Hg" width=100 height=40 color="red" font-size=16 line-height="80px"
      text testId="text" "Hg"
"##).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 200.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let field = frame(&p, "field");
    let text = frame(&p, "text");
    let shot = p.frame();
    let ink_rows = |f: Frame| -> Vec<u32> {
        (0..200)
            .filter(|&y| {
                (f.x as u32..(f.x + f.width) as u32).any(|x| {
                    let c = shot.pixel(x, y).unwrap();
                    c.red() > 100 && c.green() < 80 && c.blue() < 80
                })
            })
            .collect()
    };
    let expected = ink_rows(text);
    assert!(!expected.is_empty());
    assert_eq!(
        ink_rows(field),
        expected,
        "field ink and baseline-aligned text must occupy the same rows"
    );
}
