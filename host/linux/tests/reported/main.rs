//! The Linux host's integration test that reports the platform's colours
//! (LLP 1115.001 D4), a binary of its own: the reported table is the
//! process's, and a reported accent is never withdrawn, so no test that
//! pins this host's own look (`tests/it`, `tests/pinned`) may share a
//! process with it. One test, so that what it sees before the first report
//! is a host that reported nothing.
use exact_kernel::{Color, ColorValue, ViewId};
use exact_linux::presenter::{Material, PainterChoice, Palette, MATERIAL_ROLES};
use exact_linux::Presenter;

/// A page that writes no background, with a swatch for each kind of role, a
/// checkbox, the same with an authored accent, a switch that is on and a
/// filled button.
const APP: &str = r##"component App
  view
    column width=400 height=600 align-items="flex-start"
      box testId="accent" width=40 height=40 background-color="AccentColor"
      box testId="field" width=40 height=40 background-color="Field"
      box testId="grouped" width=40 height=40 background-color="-exact-grouped-background"
      box testId="green" width=40 height=40 background-color="-exact-system-green"
      input type="checkbox" checked=true width=20 height=20 testId="check"
      input type="checkbox" checked=true width=20 height=20 accent-color="#ff0000" testId="authored"
      input type="checkbox" role="switch" checked=true width=40 height=24 testId="switch"
      button appearance="auto" buttonStyle="filled" width=120 height=40 testId="filled"
        text "Send"
"##;

/// Material 3's baseline scheme, as a phone with no wallpaper colour has it.
fn baseline() -> Palette {
    let mut palette = Palette {
        light: [0; MATERIAL_ROLES],
        dark: [0; MATERIAL_ROLES],
    };
    for (role, light, dark) in [
        (Material::Primary, 0xff67_50a4, 0xffd0_bcffu32),
        (Material::OnPrimary, 0xffff_ffff, 0xff38_1e72),
        (Material::SecondaryContainer, 0xffe8_def8, 0xff4a_4458),
        (Material::Surface, 0xfffe_f7ff, 0xff14_1218),
        (Material::OnSurface, 0xff1d_1b20, 0xffe6_e0e9),
        (Material::OnSurfaceVariant, 0xff49_454f, 0xffca_c4d0),
        (Material::SurfaceContainerLow, 0xfff7_f2fa, 0xff1d_1b20),
        (Material::SurfaceContainer, 0xfff3_edf7, 0xff21_1f26),
        (Material::SurfaceContainerHigh, 0xffec_e6f0, 0xff2b_2930),
        (Material::SurfaceContainerHighest, 0xffe6_e0e9, 0xff36_343b),
        (Material::OutlineVariant, 0xffca_c4d0, 0xff49_454f),
        (Material::Error, 0xffb3_261e, 0xfff2_b8b5),
    ] {
        palette.light[role as usize] = light;
        palette.dark[role as usize] = dark;
    }
    palette
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// The pixel `(x, y)` into `name`'s box.
fn pixel(p: &mut Presenter<()>, name: &str, x: u32, y: u32) -> [u8; 3] {
    let f = p.host().kernel().node(id(p, name)).unwrap().frame;
    let shot = p.frame();
    let c = shot.pixel(f.x as u32 + x, f.y as u32 + y).unwrap();
    [c.red(), c.green(), c.blue()]
}

/// The page's own colour: a pixel no node paints.
fn page(p: &mut Presenter<()>) -> [u8; 3] {
    let shot = p.frame();
    let c = shot.pixel(399, 599).unwrap();
    [c.red(), c.green(), c.blue()]
}

fn role(name: &str, dark: bool) -> Color {
    ColorValue::parse_light_dark(name)
        .unwrap_or_else(|| panic!("{name} is no colour"))
        .resolve(dark)
}

fn rgb(argb: u32) -> [u8; 3] {
    let [_, r, g, b] = argb.to_be_bytes();
    [r, g, b]
}

#[test]
fn a_reported_palette_is_what_the_roles_and_the_controls_paint_in() {
    let plan = contract::compile(APP).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 600.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");

    // Before any report, the look of a host that said none: the roles'
    // fallback pairs, and this painter's own accent on its controls.
    assert_eq!(page(&mut p), [0xff, 0xff, 0xff]);
    assert_eq!(pixel(&mut p, "accent", 20, 20), [0x00, 0x88, 0xff]);
    assert_eq!(pixel(&mut p, "check", 3, 3), [0x00, 0x75, 0xff]);
    assert_eq!(pixel(&mut p, "filled", 6, 20), [0x00, 0x75, 0xff]);
    assert_eq!(pixel(&mut p, "authored", 3, 3), [0xff, 0x00, 0x00]);
    assert!(!p.dirty(), "painted");

    // The device's palette: the page is painted again, and every role is
    // the device's.
    let device = baseline();
    p.set_palette(&device);
    assert!(p.dirty(), "a report paints again");
    let (light, dark) = (device.light, device.dark);
    let at = |role: Material| role as usize;
    assert_eq!(page(&mut p), rgb(light[at(Material::Surface)]));
    let primary = rgb(light[at(Material::Primary)]);
    assert_eq!(pixel(&mut p, "accent", 20, 20), primary);
    assert_eq!(
        pixel(&mut p, "field", 20, 20),
        rgb(light[at(Material::SurfaceContainerHighest)])
    );
    assert_eq!(
        pixel(&mut p, "grouped", 20, 20),
        rgb(light[at(Material::SurfaceContainerLow)])
    );
    // One accent: the controls, a filled button and what an author names.
    assert_eq!(pixel(&mut p, "check", 3, 3), primary);
    assert_eq!(pixel(&mut p, "switch", 8, 12), primary);
    assert_eq!(pixel(&mut p, "filled", 6, 20), primary);
    // An authored accent still wins, with white on it as before.
    assert_eq!(pixel(&mut p, "authored", 3, 3), [0xff, 0x00, 0x00]);
    assert_eq!(pixel(&mut p, "authored", 8, 14), [0xff, 0xff, 0xff]);
    // A system hue D4 leaves alone keeps its pair.
    assert_eq!(pixel(&mut p, "green", 20, 20), [0x34, 0xc7, 0x59]);

    // The dark scheme is the device's dark palette, and what is drawn on
    // the accent is `onPrimary`, where white would not read.
    p.set_system_scheme(true);
    assert_eq!(page(&mut p), rgb(dark[at(Material::Surface)]));
    let primary = rgb(dark[at(Material::Primary)]);
    let on_primary = rgb(dark[at(Material::OnPrimary)]);
    assert_eq!(pixel(&mut p, "check", 3, 3), primary);
    assert_eq!(pixel(&mut p, "check", 8, 14), on_primary, "the check");
    assert_eq!(pixel(&mut p, "switch", 28, 12), on_primary, "the thumb");
    assert_eq!(pixel(&mut p, "filled", 6, 20), primary);
    assert_eq!(pixel(&mut p, "authored", 8, 14), [0xff, 0xff, 0xff]);
    p.set_system_scheme(false);

    // D4's table, row by row.
    let whole = |m: Material, dark: bool| {
        let table = if dark { &device.dark } else { &device.light };
        Color(table[m as usize] << 8 | 0xff)
    };
    let faint = |m: Material, dark: bool, alpha: u32| Color(whole(m, dark).0 & !0xff | alpha);
    for dark in [false, true] {
        for (name, material) in [
            ("Canvas", Material::Surface),
            ("-exact-background", Material::Surface),
            ("CanvasText", Material::OnSurface),
            ("-exact-label", Material::OnSurface),
            ("FieldText", Material::OnSurface),
            ("HighlightText", Material::OnSurface),
            ("-exact-secondary-label", Material::OnSurfaceVariant),
            ("-exact-placeholder", Material::OnSurfaceVariant),
            ("-exact-separator", Material::OutlineVariant),
            ("-exact-opaque-separator", Material::OutlineVariant),
            ("AccentColor", Material::Primary),
            ("LinkText", Material::Primary),
            ("-exact-link", Material::Primary),
            ("ButtonText", Material::Primary),
            ("-exact-system-blue", Material::Primary),
            ("AccentColorText", Material::OnPrimary),
            ("Field", Material::SurfaceContainerHighest),
            ("-exact-fill", Material::SurfaceContainerHighest),
            ("-exact-secondary-fill", Material::SurfaceContainerHighest),
            ("-exact-tertiary-fill", Material::SurfaceContainerHighest),
            ("-exact-quaternary-fill", Material::SurfaceContainerHighest),
            ("ButtonFace", Material::SecondaryContainer),
            ("-exact-secondary-background", Material::SurfaceContainerLow),
            ("-exact-grouped-background", Material::SurfaceContainerLow),
            (
                "-exact-secondary-grouped-background",
                Material::SurfaceContainer,
            ),
            ("-exact-tertiary-background", Material::SurfaceContainer),
            (
                "-exact-tertiary-grouped-background",
                Material::SurfaceContainerHigh,
            ),
            ("-exact-system-red", Material::Error),
        ] {
            assert_eq!(role(name, dark), whole(material, dark), "{name} {dark}");
        }
        // Material's disabled content, its faintest, and a selection.
        let on_surface = |alpha| faint(Material::OnSurface, dark, alpha);
        assert_eq!(role("-exact-tertiary-label", dark), on_surface(0x61));
        assert_eq!(role("GrayText", dark), on_surface(0x61));
        assert_eq!(role("-exact-quaternary-label", dark), on_surface(0x1f));
        assert_eq!(
            role("Highlight", dark),
            faint(Material::Primary, dark, 0x66)
        );
    }
    assert_eq!(role("-exact-system-orange", false), Color(0xff95_00ff));

    // Told the same again, nothing is painted again; a new wallpaper's
    // palette is.
    p.frame();
    assert!(!p.dirty());
    p.set_palette(&device);
    assert!(!p.dirty(), "the same palette changes nothing");
    let mut wallpaper = device;
    wallpaper.light[at(Material::Primary)] = 0xff00_6a60;
    wallpaper.light[at(Material::Surface)] = 0xfff4_fbf8;
    p.set_palette(&wallpaper);
    assert!(p.dirty(), "a changed palette paints again");
    assert_eq!(page(&mut p), [0xf4, 0xfb, 0xf8]);
    assert_eq!(pixel(&mut p, "check", 3, 3), [0x00, 0x6a, 0x60]);
    assert_eq!(pixel(&mut p, "accent", 20, 20), [0x00, 0x6a, 0x60]);

    // What the reader sends is the twelve roles light, then dark.
    let words: Vec<i32> = (device.light.iter().chain(&device.dark))
        .map(|c| *c as i32)
        .collect();
    assert_eq!(Palette::from_words(&words), Some(device));
    assert_eq!(Palette::from_words(&words[1..]), None);
}
