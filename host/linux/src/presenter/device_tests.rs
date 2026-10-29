//! The device's settings reach the app through the presenter's doors
//! (LLP 1115.001 D3.1, D3.2): what a shell tells with [`Presenter::set_device`].
use super::*;
use exact_kernel::StyleId;

/// An app that reads every preference a device sets, and sizes a box in `rem`.
const READS: &str = "shape Media\n  prefersColorScheme: string\n  prefersReducedMotion: bool\n  prefersContrast: string\n  pointer: string\n  hover: string\ncomponent App\n  resource media = exactViewport() as shape Media\n  view\n    column width=300 height=300\n      text `${media.prefersColorScheme} ${media.prefersReducedMotion} ${media.prefersContrast} ${media.pointer} ${media.hover}` testId=\"facts\"\n      box testId=\"rem\" width=\"10rem\" height=\"1rem\"\n";

fn boot(source: &str) -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

/// A phone as its owner left it out of the box.
fn phone() -> Device {
    Device {
        night: false,
        font_scale: 1.0,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text: false,
        high_contrast: false,
        touch: true,
        tv: false,
        touch_slop: 8.0,
        dates: String::new(),
    }
}

fn facts(p: &Presenter<()>) -> String {
    let kernel = p.host().kernel();
    let node = kernel
        .node_by_key(kernel.find_by_test_id("facts")[0])
        .unwrap();
    node.props.str(PropId::Text).unwrap_or_default().to_owned()
}

fn width(p: &Presenter<()>, name: &str) -> f32 {
    let kernel = p.host().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .frame
        .width
}

#[test]
fn a_host_that_is_told_nothing_reports_a_light_system_and_a_mouse() {
    let p = boot(READS);
    assert_eq!(facts(&p), "light false no-preference fine hover");
    assert_eq!(width(&p, "rem"), 160.0);
    assert!(p.device().is_none() && !p.dark());
}

#[test]
fn text_scaling_reflows_default_fields_and_preserves_authored_fonts_and_drafts() {
    let source = r#"component App
  view
    column width=350
      text "Readable text" testId="body"
      textarea value="Keep this draft" rows=1 width=300 testId="field"
      textarea value="Fixed author font" rows=1 width=300 font-size=12 testId="fixed"
      button testId="button"
        text "Action"
      box width="10rem" height=1 testId="rem"
"#;
    let mut p = boot(source);
    let height = |p: &Presenter<()>, name: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(name)[0])
            .unwrap()
            .frame
            .height
    };
    let original = (height(&p, "body"), height(&p, "field"), height(&p, "fixed"));
    assert!(p.set_text_scale(2.0).is_none());
    assert!(height(&p, "body") > original.0 * 1.5);
    assert!(
        height(&p, "field") > original.1,
        "field: {} -> {}",
        original.1,
        height(&p, "field")
    );
    assert_eq!(height(&p, "fixed"), original.2);
    assert_eq!(width(&p, "rem"), 320.0);
    let k = p.host().kernel();
    let field = k.node_by_key(k.find_by_test_id("field")[0]).unwrap();
    assert_eq!(field.props.str(PropId::Value), Some("Keep this draft"));
    for (name, size) in [("field", 32.0), ("fixed", 12.0), ("button", 32.0)] {
        let node = k.node_by_key(k.find_by_test_id(name)[0]).unwrap();
        assert_eq!(node.computed_row(StyleId::FontSize, |s| s.font_size), size);
    }
    let epoch = k.epoch();
    assert!(p.set_text_scale(2.0).is_none());
    assert_eq!(p.host().kernel().epoch(), epoch);
    let mut replacement = boot(source);
    p.restore_time(&mut replacement.host).unwrap();
    assert_eq!(height(&replacement, "field"), height(&p, "field"));
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(p.set_text_scale(invalid).is_none());
        assert_eq!(width(&p, "rem"), 320.0);
    }
    assert!(p.set_text_scale(1.0).is_none());
    assert_eq!(height(&p, "body"), original.0);
    assert_eq!(height(&p, "field"), original.1);
    assert_eq!(width(&p, "rem"), 160.0);
}

#[test]
fn a_phones_settings_reach_the_app() {
    let mut p = boot(READS);
    assert!(p.set_device(&phone()).is_none());
    // A finger is a coarse pointer that cannot hover (D3.2).
    assert_eq!(facts(&p), "light false no-preference coarse none");
    assert_eq!(width(&p, "rem"), 160.0);

    let night = Device {
        night: true,
        font_scale: 1.3,
        reduced_motion: true,
        high_contrast: true,
        ..phone()
    };
    assert!(p.set_device(&night).is_none());
    assert_eq!(facts(&p), "dark true more coarse none");
    assert!(p.dark(), "the system's scheme is dark");
    // The root font size is 16 times the font scale: 20.8, so 10rem is 208.
    assert!((p.host().runner().root_font_size() - 20.8).abs() < 1e-4);
    assert!((width(&p, "rem") - 208.0).abs() < 1e-3);
    assert_eq!(p.device(), Some(&night));

    // The same settings again commit nothing.
    let epoch = p.host().kernel().epoch();
    assert!(p.set_device(&night).is_none());
    assert_eq!(p.host().kernel().epoch(), epoch);

    // Back to day, in place.
    assert!(p.set_device(&phone()).is_none());
    assert_eq!(facts(&p), "light false no-preference coarse none");
    assert!(!p.dark());
    assert_eq!(width(&p, "rem"), 160.0);
}

#[test]
fn the_apps_own_scheme_stands_over_the_devices() {
    let mut p = boot(READS);
    p.app_scheme(Some(false));
    let night = Device {
        night: true,
        ..phone()
    };
    assert!(p.set_device(&night).is_none());
    // Author > platform: `setScheme("light")` keeps the page light, and the
    // app can still read that the system is dark.
    assert!(!p.dark());
    assert!(facts(&p).starts_with("dark "));
    p.app_scheme(None);
    assert!(p.dark(), "`setScheme(\"system\")` follows the device");
}

#[test]
fn the_devices_locale_is_the_viewers_place() {
    let mut p = boot(READS);
    let seed = p.host().runner().place().seed;
    let zone = p.host().runner().place().time_zone.clone();
    let french = Device {
        locale: "fr-CA".into(),
        rtl: false,
        hour24: true,
        ..phone()
    };
    assert!(p.set_device(&french).is_none());
    let place = p.host().runner().place();
    assert_eq!(place.locale, "fr-CA");
    assert_eq!(
        (place.seed, place.time_zone.as_str()),
        (seed, zone.as_str())
    );
    // Carried, with no door yet: the direction, the clock.
    let device = p.device().unwrap();
    assert!(device.hour24 && !device.rtl && !device.bold_text);
}

/// How an en-US phone on a 12-hour clock writes dates, as its shell tells it.
const US_DATES: &str = "MMM d, y\nh:mm a\nh:mm:ss a\nMMM d, y, h:mm a\nMMM d, y, h:mm:ss a\nAM\nPM\n0\nJan\nFeb\nMar\nApr\nMay\nJun\nJul\nAug\nSep\nOct\nNov\nDec\nJanuary\nFebruary\nMarch\nApril\nMay\nJune\nJuly\nAugust\nSeptember\nOctober\nNovember\nDecember\nJan\nFeb\nMar\nApr\nMay\nJun\nJul\nAug\nSep\nOct\nNov\nDec\nJanuary\nFebruary\nMarch\nApril\nMay\nJune\nJuly\nAugust\nSeptember\nOctober\nNovember\nDecember\nMon\nTue\nWed\nThu\nFri\nSat\nSun\nMonday\nTuesday\nWednesday\nThursday\nFriday\nSaturday\nSunday\nBC\nAD";

#[test]
fn a_date_control_is_materials_field_and_fits_the_value_as_the_device_writes_it() {
    let source = "component App\n  view\n    row\n      input type=\"date\" value=\"2026-10-16\" testId=\"day\"\n";
    let mut p = boot(source);
    let size = |p: &Presenter<()>| {
        let kernel = p.host().kernel();
        let frame = kernel
            .node_by_key(kernel.find_by_test_id("day")[0])
            .unwrap()
            .frame;
        (frame.width, frame.height)
    };
    let text = |p: &Presenter<()>, s: &str| {
        let style = exact_kernel::StyleProps::default();
        let spec = crate::paint::text_spec(&style, s);
        p.text.borrow_mut().paragraph(&spec, None).width
    };
    // Told nothing, the field fits HTML's value; 56 points tall either way.
    assert!(p.set_device(&phone()).is_none());
    let iso = size(&p);
    assert_eq!(iso.1, crate::paint::material::height());
    let us = Device {
        dates: US_DATES.into(),
        ..phone()
    };
    assert!(p.set_device(&us).is_none());
    let written = size(&p);
    assert_eq!(written.1, crate::paint::material::height());
    // The widest month's date and the trailing icon.
    let widest = ["May 28, 2000", "Sep 28, 2000", "Dec 28, 2000"]
        .map(|s| text(&p, s))
        .into_iter()
        .fold(0f32, f32::max);
    assert!(written.0 >= (widest + crate::paint::material::with_icon()).floor());
    assert_ne!(written, iso);
    // A telling of nothing returns to HTML's value.
    assert!(p.set_device(&phone()).is_none());
    assert_eq!(size(&p), iso);
}

#[test]
fn a_fields_placeholder_is_its_label_and_floats_as_it_fills() {
    let source = "component App\n  state day = \"\"\n  action setDay(value)\n    day = value\n  view\n    row\n      input type=\"date\" value=day change=setDay placeholder=\"Due\" testId=\"day\"\n";
    let mut p = boot(source);
    let day = {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id("day")[0]).unwrap().id
    };
    p.clock(1000.);
    let resting = p.frame().data().to_vec();
    assert!(!p.needs_animation_frame(), "an empty field's label rests");
    // A value floats it, over 150 ms, and then asks for no frame.
    p.set_control_value(day, "2026-10-23").unwrap();
    let _ = p.frame();
    assert!(p.needs_animation_frame(), "the label is floating");
    p.clock(1200.);
    let floated = p.frame().data().to_vec();
    assert!(!p.needs_animation_frame(), "and has floated");
    assert_ne!(resting, floated);
}

/// Android's bold text (its font weight adjustment) is Bold Text (LLP
/// 1115.000 D12.3): the system font one step heavier, and back.
#[test]
fn bold_text_weights_the_system_font() {
    let mut p = boot(READS);
    let weight = |p: &Presenter<()>| {
        let kernel = p.host().kernel();
        let key = kernel.find_by_test_id("facts")[0];
        kernel.node_by_key(key).unwrap().text_style().font_weight
    };
    assert_eq!(weight(&p), 400);
    assert!(p
        .set_device(&Device {
            bold_text: true,
            ..phone()
        })
        .is_none());
    assert_eq!(weight(&p), 600);
    assert!(p.set_device(&phone()).is_none());
    assert_eq!(weight(&p), 400);
}

#[test]
fn bold_text_weights_material_glyphs_and_symbols() {
    for (family, steps) in [("system-ui", true), ("Georgia, serif", false)] {
        for (element, field, stroked) in [
            ("input type=\"time\"", true, true),
            ("input type=\"date\"", true, true),
            ("select", true, false),
            ("image \"symbol:calendar\"", false, true),
        ] {
            let source = format!(
                "component App\n  view\n    column font-family=\"{family}\" font-weight=500\n      {element} width=200 height=56 testId=\"icon\"\n"
            );
            let mut p = boot(&source);
            let mut pictures = Vec::new();
            for bold_text in [false, true, false] {
                assert!(p
                    .set_device(&Device {
                        bold_text,
                        ..phone()
                    })
                    .is_none());
                let k = p.host().kernel();
                let frame = k.node_by_key(k.find_by_test_id("icon")[0]).unwrap().frame;
                let shot = p.frame();
                // Crop only the 24 dp glyph, excluding the value and indicator.
                let (x, y, w, h) = if field {
                    (
                        frame.x + frame.width - 36.,
                        frame.y + (frame.height - 24.) / 2.,
                        24,
                        24,
                    )
                } else {
                    (frame.x, frame.y, frame.width as u32, frame.height as u32)
                };
                let mut pixels = Vec::new();
                for row in 0..h {
                    for col in 0..w {
                        pixels.push(shot.pixel(x as u32 + col, y as u32 + row).unwrap());
                    }
                }
                pictures.push(pixels);
            }
            assert_eq!(pictures[0], pictures[2], "restored {element}, {family}");
            assert_eq!(
                pictures[0] != pictures[1],
                steps && stroked,
                "{element}, {family}"
            );
        }
    }
}

#[test]
fn a_touch_screen_alone_is_the_primary_pointer() {
    let touch = Pointing {
        absolute: true,
        touch: true,
        ..Default::default()
    };
    let touchpad = Pointing {
        absolute: true,
        touch: true,
        left: true,
        ..Default::default()
    };
    let mouse = Pointing {
        relative: true,
        left: true,
        ..Default::default()
    };
    let kvm = Pointing {
        absolute: true,
        left: true,
        ..Default::default()
    };
    assert!(touch_only(&[touch]));
    assert!(!touch_only(&[touch, mouse]), "a laptop's mouse is primary");
    assert!(!touch_only(&[touchpad]));
    assert!(!touch_only(&[kvm]));
    assert!(!touch_only(&[]));
}

/// Android's shell tells its locale and no regional format (LLP
/// 1027.000.000): `formatLocale` answers the device's locale, not the one
/// the process launched with.
#[test]
fn a_phones_locale_is_its_format_locale_too() {
    let mut p = boot(READS);
    let launched = exact_runner::time::Place {
        format_locale: "en-US".into(),
        seed: 1.0,
        ..Default::default()
    };
    assert!(p.set_place(&launched).is_none());
    let korean = Device {
        locale: "ko-KR".into(),
        ..phone()
    };
    assert!(p.set_device(&korean).is_none());
    let place = p.host().runner().place();
    assert_eq!(
        (place.locale.as_str(), place.format_locale.as_str()),
        ("ko-KR", "")
    );
}

/// A page symbol is drawn in its `color`, inherited here from a custom button
/// or a box, as Apple and the web draw one (LLP 1081 §4), not in the accent.
#[test]
fn a_page_symbol_takes_its_color() {
    let source = "component App\n  view\n    column\n      button appearance=\"none\" color=\"#ff0000\" testId=\"b\"\n        image \"symbol:emoji\" width=48 height=48\n      view color=\"#ff0000\" testId=\"v\"\n        image \"symbol:emoji\" width=48 height=48\n";
    let mut p = boot(source);
    for id in ["b", "v"] {
        let frame = {
            let k = p.host().kernel();
            k.node_by_key(k.find_by_test_id(id)[0]).unwrap().frame
        };
        let shot = p.frame();
        let (mut red, mut other) = (0, 0);
        for y in 0..frame.height as u32 {
            for x in 0..frame.width as u32 {
                let c = shot.pixel(frame.x as u32 + x, frame.y as u32 + y).unwrap();
                if c.red() > 150 && c.green() < 100 && c.blue() < 100 {
                    red += 1;
                } else if c.blue() > 150 && c.red() < 100 {
                    other += 1;
                }
            }
        }
        assert!(red > 100 && other == 0, "{id}: {red} red, {other} accent");
    }
}

/// A grouped row's leading symbol and chevron keep the tints the platform's
/// sheet gives them (the accent, a tertiary grey), as UIKit tints a list
/// cell's image; only an author's `color` is the symbol's own.
#[test]
fn a_grouped_rows_symbols_keep_the_platforms_tints() {
    let source = "component App\n  view\n    list appearance=\"auto\" listStyle=\"inset-grouped\" width=300 height=200 color=\"#000000\"\n      section\n        row\n          image \"symbol:star\" testId=\"lead\"\n          text \"Starred\"\n          image \"symbol:forward-chevron\" testId=\"tail\"\n";
    let mut p = boot(source);
    let shot = p.frame();
    let k = p.host().kernel();
    let ink = |id: &str| {
        let f = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().frame;
        (0..f.height as u32)
            .flat_map(|y| (0..f.width as u32).map(move |x| (x, y)))
            .filter_map(|(x, y)| shot.pixel(f.x as u32 + x, f.y as u32 + y))
            .map(|c| {
                let c = c.demultiply();
                [c.red(), c.green(), c.blue()]
            })
            .min_by_key(|c| u16::from(c[0]) + u16::from(c[1]) + u16::from(c[2]))
            .unwrap()
    };
    let (lead, tail) = (ink("lead"), ink("tail"));
    assert!(
        lead[2] > 150 && lead[0] < 100,
        "the star is the accent: {lead:?}"
    );
    assert!(
        tail[0] > 100 && tail[0].abs_diff(tail[2]) < 30,
        "the chevron is a grey: {tail:?}"
    );
}
