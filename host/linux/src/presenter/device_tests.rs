//! The device's settings reach the app through the presenter's doors
//! (LLP 1115.001 D3.1, D3.2): what a shell tells with [`Presenter::set_device`].
use super::*;

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
    // Carried, with no door yet: the direction, the clock, bold text.
    let device = p.device().unwrap();
    assert!(device.hour24 && !device.rtl && !device.bold_text);
}

/// How an en-US phone on a 12-hour clock writes dates, as its shell tells it.
const US_DATES: &str = "MMM d, y\nh:mm a\nh:mm:ss a\nMMM d, y, h:mm a\nMMM d, y, h:mm:ss a\nAM\nPM\n0\nJan\nFeb\nMar\nApr\nMay\nJun\nJul\nAug\nSep\nOct\nNov\nDec\nJanuary\nFebruary\nMarch\nApril\nMay\nJune\nJuly\nAugust\nSeptember\nOctober\nNovember\nDecember\nMon\nTue\nWed\nThu\nFri\nSat\nSun\nMonday\nTuesday\nWednesday\nThursday\nFriday\nSaturday\nSunday\nBC\nAD";

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
    assert_eq!(iso.1, crate::paint::material::HEIGHT);
    let us = Device {
        dates: US_DATES.into(),
        ..phone()
    };
    assert!(p.set_device(&us).is_none());
    let written = size(&p);
    assert_eq!(written.1, crate::paint::material::HEIGHT);
    // The widest month's date and the trailing icon.
    let widest = ["May 28, 2000", "Sep 28, 2000", "Dec 28, 2000"]
        .map(|s| text(&p, s))
        .into_iter()
        .fold(0f32, f32::max);
    assert!(written.0 >= (widest + crate::paint::material::WITH_ICON).floor());
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
