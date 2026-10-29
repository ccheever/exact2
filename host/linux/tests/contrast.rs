//! Palette reports are process-wide; this test owns its process.
use exact_kernel::{Color, ColorValue};
use exact_linux::presenter::{ContrastPalette, PainterChoice};
use exact_linux::Presenter;

fn role(name: &str, dark: bool) -> Color {
    ColorValue::parse_light_dark(name).unwrap().resolve(dark)
}

fn pixel(p: &mut Presenter<()>, x: u32, y: u32) -> [u8; 4] {
    let frame = p.frame();
    let pixel = frame.pixel(x, y).unwrap();
    [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
}

#[test]
fn contrast_changes_live_and_restores_both_schemes_without_replacing_literal_colors() {
    exact_linux::app::show_neutral_look();
    let plan = contract::compile(
        r##"component App
  view
    column width=300 height=200
      box width=40 height=40 background-color="AccentColor"
      box width=40 height=40 background-color="#123456"
      input width=100 height=32 placeholder="Type here"
      list appearance="auto" listStyle="inset-grouped" width=250 height=80
        section
          button
            text "Channel"
"##,
    )
    .unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (300., 200.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let original: Vec<_> = [
        "AccentColor",
        "Canvas",
        "-exact-label",
        "-exact-secondary-label",
        "-exact-system-red",
        "ButtonText",
        "HighlightText",
    ]
    .into_iter()
    .flat_map(|name| [false, true].map(move |dark| (name, dark, role(name, dark))))
    .collect();
    let before = pixel(&mut p, 299, 199);
    let mut palette = ContrastPalette {
        window: [32, 32, 32, 255],
        text: [255; 4],
        link: [117, 233, 252, 255],
        disabled: [166, 166, 166, 255],
        highlight: [255, 255, 0, 255],
        highlighted_text: [0, 0, 0, 255],
        button: [32, 32, 32, 255],
        button_text: [255; 4],
    };
    for background in [[32, 32, 32, 255], [255, 247, 214, 255]] {
        palette.window = background;
        p.set_contrast_palette(Some(palette));
        assert!(p.dirty());
        assert_eq!(pixel(&mut p, 299, 199), background);
        assert_eq!(pixel(&mut p, 10, 10), palette.highlight);
        assert_eq!(pixel(&mut p, 200, 150), palette.window, "grouped card");
        assert_eq!(pixel(&mut p, 10, 50), [0x12, 0x34, 0x56, 255]);
        for dark in [false, true] {
            assert_eq!(
                role("-exact-secondary-label", dark),
                Color(u32::from_be_bytes(palette.text))
            );
            assert_eq!(
                role("GrayText", dark),
                Color(u32::from_be_bytes(palette.disabled))
            );
            assert_eq!(
                role("HighlightText", dark),
                Color(u32::from_be_bytes(palette.highlighted_text))
            );
        }
        p.set_contrast_palette(Some(palette));
        assert!(!p.dirty(), "an unchanged palette does not repaint");
    }
    #[cfg(target_os = "windows")]
    {
        let dialog = contract::compile(
            r#"component App
  view
    column width=400 height=400
      button commandfor="ask" command="show-modal" testId="open"
        text "Open"
      dialog id="ask" role="alertdialog" aria-label="Confirm"
        text "Keep this message?"
        button commandfor="ask" command="close"
          text "Cancel"
"#,
        )
        .unwrap();
        let (mut dialog, error) = Presenter::boot_with(
            &dialog.encode(),
            (),
            (400., 400.),
            1.,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none());
        dialog.serve_requests();
        dialog.serve_overlays();
        let k = dialog.host().kernel();
        let open = k.node_by_key(k.find_by_test_id("open")[0]).unwrap().id;
        dialog.tap(open).unwrap();
        dialog.run_commands(|| ());
        let serial = dialog.surface().unwrap().serial;
        dialog.set_contrast_palette(Some(palette));
        assert_eq!(pixel(&mut dialog, 60, 110), palette.window);
        palette.window = [16, 8, 32, 255];
        dialog.set_contrast_palette(Some(palette));
        assert_eq!(pixel(&mut dialog, 60, 110), palette.window);
        assert_eq!(
            dialog.surface().unwrap().serial,
            serial,
            "same dialog recolored in place"
        );
    }
    p.set_contrast_palette(None);
    assert_eq!(pixel(&mut p, 299, 199), before);
    for (name, dark, color) in original {
        assert_eq!(role(name, dark), color, "restore {name}, dark={dark}");
    }
    badges_keep_their_labels_in_contrast_themes(palette);
}

fn badges_keep_their_labels_in_contrast_themes(mut palette: ContrastPalette) {
    let source = r##"use Badge from "exact:content"
component App
  view
    column width=200 height=240
      row height=40 testId="neutral"
        Badge(label="12", tint=none)
      row height=40 testId="red"
        Badge(label="3", tint=some("-exact-system-red"))
      row height=40 testId="accent"
        Badge(label="APP", tint=some("AccentColor"))
      row height=40 testId="orange"
        Badge(label="NEW", tint=some("-exact-system-orange"))
      row height=40 testId="literal"
        Badge(label="Custom", tint=some("#123456"))
"##;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/badges.contract");
    let plan = contract::compile_path_source(&path, source).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (200., 240.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let before = p.frame().data().to_vec();
    for dark in [false, true] {
        palette.window = if dark {
            [32, 32, 32, 255]
        } else {
            [255, 247, 214, 255]
        };
        palette.highlight = if dark {
            [142, 227, 240, 255]
        } else {
            [144, 56, 58, 255]
        };
        palette.highlighted_text = if dark { [0, 0, 0, 255] } else { [255; 4] };
        p.set_contrast_palette(Some(palette));
        assert!(p
            .set_preferences(exact_runner::Preferences {
                dark,
                contrast: exact_runner::Contrast::Custom,
                ..Default::default()
            })
            .is_none());
        let regions: Vec<_> = ["neutral", "red", "accent", "orange", "literal"]
            .into_iter()
            .map(|name| {
                let k = p.host().kernel();
                let bounds = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().frame;
                (name, bounds)
            })
            .collect();
        let frame = p.frame();
        for (name, bounds) in regions {
            let mut fill = 0;
            let mut ink = 0;
            let expected = if name == "literal" {
                [0x12, 0x34, 0x56, 255]
            } else {
                palette.highlight
            };
            for y in bounds.y as u32..(bounds.y + bounds.height) as u32 {
                for x in bounds.x as u32..(bounds.x + bounds.width) as u32 {
                    let px = frame.pixel(x, y).unwrap();
                    let rgba = [px.red(), px.green(), px.blue(), px.alpha()];
                    fill += usize::from(rgba == expected);
                    ink += usize::from(rgba == palette.highlighted_text);
                }
            }
            assert!(fill > 20, "{name} fill, dark={dark}: {fill}");
            if name != "literal" {
                assert!(ink > 0, "{name} label, dark={dark}: {ink}");
            }
        }
    }
    p.set_contrast_palette(None);
    assert!(p.set_preferences(Default::default()).is_none());
    assert_eq!(p.frame().data(), before, "ordinary badges restored");
}
