//! LLP 1095 D1, D6 on the Apple host: what the kernel resolves itself — a
//! gradient's stops, an SVG scene's paint, paint motion's endpoints —
//! follows the presenter's report, and a new report re-presents it.
use super::*;
use exact_kernel::style::{roles, Color, ColorValue};
use exact_kernel::MonospaceMeasurer;
use exact_plan::Value;
use exact_runner::DataError;

#[derive(Clone, Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// The table is process-wide: these tests take turns.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

// A name no other test reports.
const BRAND: &str = "-exact-platform-color(ios colorsTestBrandColor, macos colorsTestBrandColor, light-dark(#102030, #405060))";

fn app() -> String {
    format!(
        "component A\n  view\n    column\n      box testId=\"wash\" width=20 height=20 background-image=\"linear-gradient({BRAND}, #ffffff)\"\n      svg width=10 height=10 viewBox=\"0 0 10 10\"\n        circle cx=5 cy=5 r=5 fill=\"{BRAND}\"\n      text \"hi\" color=\"{BRAND}\" transition=\"color 300ms linear\"\n"
    )
}

fn ops(batch: &str) -> Vec<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"].as_array().cloned().unwrap_or_default()
}

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn brand() -> ColorValue {
    roles::references(cfg!(target_os = "macos"))
        .into_iter()
        .find(|(_, name)| &**name == "colorsTestBrandColor")
        .map(|(c, _)| c)
        .expect("the plan interned it")
}

fn boot(app: &str) -> (Host<NoData>, String) {
    let plan = contract::compile(app).unwrap().encode();
    Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap()
}

fn restyled<'a>(ops: &'a [serde_json::Value], row: &str) -> Option<&'a serde_json::Value> {
    ops.iter()
        .find(|op| op["op"] == "style" && op["style"].get(row).is_some())
}

#[test]
fn a_report_re_presents_what_the_kernel_resolved() {
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (mut host, boot) = boot(&app());
    // Before any report: the fallback pair, light.
    assert!(boot.contains("16,32,48"), "the gradient's fallback stop");
    host.set_scheme(false);
    let c = brand();
    assert_eq!(c.resolve(false), Color(0x1020_30ff));
    // The first report corrects boot's resolution without motion (LLP 1095
    // D6): here a blue brand, after the scheme was already reported.
    let blue = Color(0x0000_ffff);
    let first = host.set_colors(vec![(c, false, blue), (c, true, blue)]);
    let v: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(
        v["motion"], false,
        "a correction, not a transition: {first}"
    );
    assert!(
        restyled(&ops(&first), "background_image").is_some(),
        "{first}"
    );
    // The platform then says the brand is red in light and green in dark.
    let (red, green) = (Color(0xff00_00ff), Color(0x00ff_00ff));
    let batch = host.set_colors(vec![(c, false, red), (c, true, green)]);
    assert_eq!(c.resolve(false), red);
    assert_eq!(c.resolve(true), green);
    let ops = ops(&batch);
    let restyled = ops
        .iter()
        .find(|op| op["op"] == "style" && op["style"].get("background_image").is_some())
        .expect("the gradient's box is restyled");
    let stops = restyled["style"]["background_image"]["stops"].to_string();
    assert!(stops.starts_with("[0,255,0,0,255"), "{stops}");
    let dark = restyled["style"]["background_image"]["dark"].to_string();
    assert!(dark.starts_with("[0,0,255,0,255"), "{dark}");
    let scene = ops
        .iter()
        .find(|op| op["op"] == "svg")
        .expect("the scene is rebuilt");
    assert!(
        scene["scene"]
            .to_string()
            .contains("[[255,0,0,255],[0,255,0,255]]"),
        "{}",
        scene["scene"]
    );
    // The text's colour transitions toward the reported one (LLP 1095 D6).
    let v: serde_json::Value = serde_json::from_str(&batch).unwrap();
    assert_eq!(v["motion"], true, "a transition runs");
    // The same report again changes nothing and sends nothing.
    let again = host.set_colors(vec![(c, false, red), (c, true, green)]);
    assert!(self::ops(&again).is_empty(), "{again}");
    // No report: the fallback pair again.
    host.set_colors(Vec::new());
    assert_eq!(c.resolve(false), Color(0x1020_30ff));
}

/// LLP 1100 D2: a space Core Graphics has no name for crosses as extended
/// linear sRGB, unclipped.
#[test]
fn a_wide_colour_crosses_as_its_space_and_components() {
    let json = |text: &str| {
        let mut out = String::new();
        crate::style::push_color_value(&mut out, ColorValue::parse_light_dark(text).unwrap());
        serde_json::from_str::<serde_json::Value>(&out).unwrap()
    };
    let p3 = json("color(display-p3 1 0 0 / 0.5)");
    assert_eq!(p3["cs"][0]["s"], "display-p3");
    assert_eq!(p3["cs"][0]["v"].to_string(), "[1,0,0,0.5]");
    assert_eq!(p3["c"], serde_json::json!([255, 0, 0, 128]));
    let ok = json("oklch(0.7 0.3 145)");
    assert_eq!(ok["cs"][0]["s"], "srgb-linear");
    let v: Vec<f64> = ok["cs"][0]["v"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_f64().unwrap())
        .collect();
    assert!(v[0] < 0.0 && v[1] > 0.0, "outside sRGB, unclipped: {v:?}");
    let pair = json("light-dark(color(display-p3 0 1 0), #000)");
    assert_eq!(pair["cs"].as_array().unwrap().len(), 2);
    assert_eq!(pair["cs"][1]["s"], "srgb");
    assert_eq!(
        pair["c"],
        serde_json::json!([[0, 255, 0, 255], [0, 0, 0, 255]])
    );
}

/// LLP 1100 D2, D3 end to end: box, shadow, SVG, gradient and profile
/// colours reach the presenter in their own space.
#[test]
fn wide_colours_reach_the_presenter_in_their_space() {
    let (_host, boot) = boot(
        "color-profile --apple-test src=\"assets/t.icc\" rendering-intent=\"saturation\"\ncomponent A\n  view\n    column\n      box testId=\"p3\" width=20 height=20 background-color=\"color(display-p3 1 0 0)\" box-shadow=\"0 2px 4px color(display-p3 0 1 0 / 0.5)\"\n      svg width=10 height=10 viewBox=\"0 0 10 10\"\n        circle cx=5 cy=5 r=5 fill=\"oklch(0.7 0.3 145)\" fill-opacity=0.5\n      box testId=\"g\" width=20 height=20 background-image=\"linear-gradient(in oklch, color(display-p3 1 0 0), color(display-p3 0 0 1))\"\n      box testId=\"l\" width=20 height=20 background-image=\"linear-gradient(#ff0000, #0000ff)\"\n      box width=4 height=4 background-color=\"color(--dci-p3 1 0.5 0)\"\n      box width=4 height=4 border-width=1 border-color=\"color(--apple-test 0.1 0.2 0.3 0.4 / 0.5)\"\n",
    );
    let ops = ops(&boot);
    let style = ops
        .iter()
        .find_map(|op| op["style"].get("box_shadow").map(|_| &op["style"]))
        .expect("the box is styled");
    assert_eq!(style["background_color"]["cs"][0]["s"], "display-p3");
    assert_eq!(style["box_shadow"][0]["c"]["cs"][0]["s"], "display-p3");
    assert_eq!(style["box_shadow"][0]["c"]["cs"][0]["v"][3], 0.5);
    let scene = ops
        .iter()
        .find(|op| op["op"] == "svg")
        .expect("a scene")
        .to_string();
    assert!(scene.contains("\"cs\":[{\"s\":\"srgb-linear\""), "{scene}");

    let images: Vec<serde_json::Value> = ops
        .iter()
        .filter_map(|op| op["style"].get("background_image").cloned())
        .collect();
    let wide = images
        .iter()
        .find(|g| g["space"] == "srgb-linear")
        .expect("the oklch gradient is flagged");
    let stops: Vec<f64> = wide["stops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_f64().unwrap())
        .collect();
    assert_eq!(stops.len(), 17 * 5, "sixteen samples a stretch");
    assert!(stops[1] > 1.0, "P3 red, unclipped: {:?}", &stops[..5]);
    let legacy = images
        .iter()
        .find(|g| g.get("space").is_none())
        .expect("the legacy gradient");
    assert_eq!(legacy["stops"].as_array().unwrap()[1], 255, "8-bit sRGB");

    // A profile's colour: the space Core Graphics makes, no sRGB beside it.
    assert!(
        boot.contains(
            r#"{"cs":[{"s":"cg:kCGColorSpaceDCIP3","i":"relative-colorimetric","v":[1,0.5,0,1]}]}"#
        ),
        "{boot}"
    );
    assert!(
        boot.contains(r#"{"s":"icc:assets/t.icc","i":"saturation","v":[0.1,0.2,0.3,0.4,0.5]}"#),
        "{boot}"
    );
}

/// LLP 1100 D2: a transition to a wide colour moves in Oklab and ends in
/// the colour as written.
#[test]
fn a_transition_to_a_wide_colour_ends_in_its_space() {
    let (mut host, _) = boot(
        "component A\n  state on = false\n  action go()\n    on = not on\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      box testId=\"b\" width=20 height=20 background-color=(on ? \"color(display-p3 0 1 0)\" : \"#ff0000\") transition=\"background-color 300ms linear\"\n",
    );
    let (go, b) = (view(&host, "go"), view(&host, "b"));
    let mut seen = ops(&host.dispatch_at(go, exact_runner::Event::Press, 100.0));
    for t in [250.0, 400.0, 600.0, 1000.0] {
        seen.extend(ops(&host.tick(t)));
    }
    let restyled: Vec<&serde_json::Value> = seen
        .iter()
        .filter(|op| op["id"] == b && op["style"].get("background_color").is_some())
        .collect();
    let frames: Vec<&&serde_json::Value> = restyled
        .iter()
        .filter(|op| op["style"]["background_color"]["cs"][0]["s"] == "srgb-linear")
        .collect();
    assert!(!frames.is_empty(), "frames in Oklab: {seen:?}");
    // Oklab's midpoint from red to green is lighter than sRGB's olive
    // (linear 0.21 each).
    let lit = frames.iter().any(|f| {
        let v = &f["style"]["background_color"]["cs"][0]["v"];
        let (r, g) = (v[0].as_f64().unwrap(), v[1].as_f64().unwrap());
        r > 0.2 && g > 0.2 && r + g > 0.5
    });
    assert!(lit, "{frames:?}");
    let last = restyled.last().expect("the box is restyled");
    assert_eq!(
        last["style"]["background_color"]["cs"][0]["s"], "display-p3",
        "{last}"
    );
}

/// LLP 1100 D8: `dynamic-range-limit` is inherited, so a box below the node
/// that sets it gets it too.
#[test]
fn the_dynamic_range_limit_reaches_a_box_below_the_node_that_sets_it() {
    let (mut host, boot) = boot(
        "component A\n  state limit = \"constrained\"\n  action loosen\n    limit = \"standard\"\n  view\n    column dynamic-range-limit=limit\n      button \"Loosen\" press=loosen testId=\"loosen\"\n      column\n        box testId=\"leaf\" width=10 height=10 background-color=\"color(rec2100-linear 4 4 4)\"\n",
    );
    let limits: Vec<serde_json::Value> = ops(&boot)
        .iter()
        .filter(|op| op["style"].get("background_color").is_some())
        .map(|op| op["style"]["dynamic_range_limit"].clone())
        .collect();
    assert_eq!(limits, [serde_json::json!("constrained")], "the leaf box");
    let batch = host.dispatch(view(&host, "loosen"), exact_runner::Event::Press);
    let leaf = view(&host, "leaf");
    let restyled = ops(&batch)
        .into_iter()
        .find(|op| op["id"] == serde_json::json!(leaf) && op.get("style").is_some())
        .expect("the leaf is restyled");
    assert_eq!(
        restyled["style"]["dynamic_range_limit"], "standard",
        "{batch}"
    );
}

/// LLP 1034 §8: `color-scheme` reaches every native view below the node
/// that sets it (UIKit and AppKit inherit it too, but a popover or dialog the
/// host lifts out of its ancestor would not), a change restyles them, and a
/// box outside it carries nothing.
#[test]
fn the_color_scheme_reaches_every_view_below_the_node_that_sets_it() {
    let (mut host, boot) = boot(
        "component A\n  state dark = true\n  action flip\n    dark = not dark\n  view\n    column\n      column testId=\"sheet\" color-scheme=(dark ? \"dark\" : \"light\") background-color=\"light-dark(#ffffff, #000000)\"\n        button \"Flip\" press=flip testId=\"flip\"\n        box testId=\"leaf\" width=10 height=10 background-color=\"light-dark(#ffffff, #000000)\"\n      box testId=\"outside\" width=10 height=10 background-color=\"light-dark(#ffffff, #000000)\"\n",
    );
    let style = |batch: &str, id| {
        ops(batch)
            .into_iter()
            .find(|op| op["id"] == serde_json::json!(id) && op.get("style").is_some())
            .map(|op| op["style"].clone())
    };
    let (sheet, leaf, outside) = (
        view(&host, "sheet"),
        view(&host, "leaf"),
        view(&host, "outside"),
    );
    assert_eq!(style(&boot, sheet).unwrap()["color_scheme"], "dark");
    assert_eq!(style(&boot, leaf).unwrap()["color_scheme"], "dark");
    let plain = style(&boot, outside).unwrap();
    assert!(plain.get("color_scheme").is_none(), "{plain}");
    let batch = host.dispatch(view(&host, "flip"), exact_runner::Event::Press);
    assert_eq!(
        style(&batch, sheet).expect("the sheet restyled")["color_scheme"],
        "light",
        "{batch}"
    );
    assert_eq!(
        style(&batch, leaf).expect("the leaf restyled")["color_scheme"],
        "light",
        "{batch}"
    );
}

#[test]
fn every_session_re_presents_a_changed_report_not_only_the_first() {
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (mut a, _) = boot(&app());
    let (mut b, _) = boot(&app());
    let c = brand();
    let report = vec![
        (c, false, Color(0xff00_00ff)),
        (c, true, Color(0x00ff_00ff)),
    ];
    assert!(restyled(&ops(&a.set_colors(report.clone())), "background_image").is_some());
    // The table already holds this report; `b` has not presented it yet.
    let second = ops(&b.set_colors(report.clone()));
    let gradient = restyled(&second, "background_image").expect("the second session restyles");
    let stops = gradient["style"]["background_image"]["stops"].to_string();
    assert!(stops.starts_with("[0,255,0,0,255"), "{stops}");
    assert!(
        second.iter().any(|op| op["op"] == "svg"),
        "and rebuilds its scene"
    );
    assert!(ops(&b.set_colors(report)).is_empty(), "then nothing more");
    a.set_colors(Vec::new());
}

#[test]
fn a_box_filters_shadow_follows_the_report() {
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // One shadow is `currentcolor` under a referenced `color`, one names the
    // reference itself (LLP 1095 D1).
    let (mut host, _) = boot(&format!(
        "component A\n  view\n    column\n      box testId=\"inherits\" width=20 height=20 color=\"{BRAND}\" filter=\"drop-shadow(0px 2px 4px)\"\n      box testId=\"names\" width=20 height=20 filter=\"drop-shadow(0px 2px 4px {BRAND})\"\n"
    ));
    let c = brand();
    let batch = host.set_colors(vec![
        (c, false, Color(0xff00_00ff)),
        (c, true, Color(0x00ff_00ff)),
    ]);
    let shadows: Vec<(String, String)> = ops(&batch)
        .iter()
        .filter(|op| op["op"] == "style" && op["style"].get("filter").is_some())
        .map(|op| {
            let f = &op["style"]["filter"];
            (f["p"].to_string(), f["pd"].to_string())
        })
        .collect();
    assert_eq!(shadows.len(), 2, "{batch}");
    // Light in `p`, and the dark appearance's own chain in `pd` (LLP 1095 D5).
    for (p, pd) in shadows {
        assert!(p.contains("1,0,0,1"), "the shadow is the reported red: {p}");
        assert!(pd.contains("0,1,0,1"), "and green when dark: {pd}");
    }
    host.set_colors(Vec::new());
}

#[test]
fn a_platform_colour_in_a_branch_not_yet_taken_is_reported_from_boot() {
    let _turn = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // Compiling interns what it checks, in this process; renaming the
    // compiled literal gives a plan whose colour nothing has interned, as a
    // plan baked elsewhere is.
    let mut plan = contract::compile(
        "component A\n  state on = false\n  view\n    column\n      box width=20 height=20 background-image=(on ? \"linear-gradient(-exact-platform-color(ios colorsTestLateSeedColor, macos colorsTestLateSeedColor, #010203), #ffffff)\" : \"none\")\n",
    )
    .unwrap();
    for s in &mut plan.strings {
        *s = s.replace("colorsTestLateSeed", "colorsTestLateBoot");
    }
    let named = || {
        roles::references(cfg!(target_os = "macos"))
            .into_iter()
            .find(|(_, name)| &**name == "colorsTestLateBootColor")
            .map(|(c, _)| c)
    };
    assert!(named().is_none(), "nothing has interned it yet");
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    // Boot interned it, so the presenter's first report resolves it.
    let c = named().expect("the inactive branch's colour is a reference from boot");
    host.set_colors(vec![
        (c, false, Color(0xff00_00ff)),
        (c, true, Color(0xff00_00ff)),
    ]);
    assert_eq!(c.resolve(false), Color(0xff00_00ff));
    host.set_colors(Vec::new());
}

#[test]
fn wide_shadow_motion_reaches_the_presenter_unclipped() {
    let (mut host, _) = boot("component A\n  state on = false\n  action go\n    on = true\n  view\n    column\n      button testId=\"go\" press=go\n        text \"Go\"\n      box testId=\"b\" width=20 height=20 box-shadow=(on ? \"0 2px 4px color(rec2100-linear 4 4 4)\" : \"0 2px 4px #000000\") transition=\"box-shadow 1s linear\"\n");
    let (go, b) = (view(&host, "go"), view(&host, "b"));
    host.dispatch_at(go, exact_runner::Event::Press, 100.0);
    let frame = ops(&host.tick(900.0));
    let shadow = frame
        .iter()
        .find(|op| op["id"] == b && op["style"].get("box_shadow").is_some())
        .unwrap();
    let color = &shadow["style"]["box_shadow"][0]["c"]["cs"][0];
    assert_eq!(color["s"], "srgb-linear");
    assert!(color["v"][0].as_f64().unwrap() > 1.5, "{shadow}");
}

#[test]
fn mixed_appearance_gradient_keeps_each_schemes_wire_space() {
    let (_, batch) = boot("component A\n  view\n    box width=20 height=20 background-image=\"linear-gradient(light-dark(red, color(display-p3 1 0 0)), blue)\"\n");
    let parsed = ops(&batch);
    let gradient = parsed
        .iter()
        .find_map(|op| op["style"].get("background_image"))
        .unwrap();
    assert!(gradient["space"].is_null());
    assert_eq!(gradient["darkSpace"], "srgb-linear");
    assert_eq!(gradient["stops"][1], 255);
    assert_eq!(gradient["stops"][4], 255);
    assert!(gradient["dark"][1].as_f64().unwrap() > 1.1);
}

#[test]
fn mixed_appearance_translucent_gradient_keeps_legacy_premultiplication() {
    for (first, second, legacy_key, wide_key) in [
        (
            "light-dark(rgb(255 0 0 / 50%), color(display-p3 1 0 0))",
            "light-dark(rgb(0 0 255 / 25%), color(display-p3 0 0 1))",
            "stops",
            "dark",
        ),
        (
            "light-dark(color(display-p3 1 0 0), rgb(255 0 0 / 50%))",
            "light-dark(color(display-p3 0 0 1), rgb(0 0 255 / 25%))",
            "dark",
            "stops",
        ),
    ] {
        let (_, batch) = boot(&format!("component A\n  view\n    box background-image=\"linear-gradient({first}, {second})\"\n"));
        let parsed = ops(&batch);
        let gradient = parsed
            .iter()
            .find_map(|op| op["style"].get("background_image"))
            .unwrap();
        let expected = exact_kernel::gradient::BackgroundImage::parse(
            "linear-gradient(rgb(255 0 0 / 50%), rgb(0 0 255 / 25%))",
        )
        .unwrap();
        let ramp =
            exact_kernel::gradient::premultiplied_ramp(&expected.layers()[0].resolved(false));
        let stops = gradient[legacy_key].as_array().unwrap();
        assert_eq!(stops.len(), ramp.len() * 5);
        for (row, (at, c)) in stops.chunks_exact(5).zip(ramp) {
            assert!((row[0].as_f64().unwrap() - f64::from(at)).abs() < 0.00001);
            assert_eq!(
                &row[1..],
                &[c.r(), c.g(), c.b(), c.a()].map(serde_json::Value::from)
            );
        }
        assert!(gradient[wide_key][1].as_f64().unwrap() > 1.1);
    }
}

#[test]
fn hdr_colors_outside_motion_storage_change_discretely() {
    for value in [
        "color(rec2100-linear 32 32 32)",
        "color(srgb-linear 1000 1000 1000)",
    ] {
        let (mut host, _) = boot(&format!("component A\n  state on = false\n  action go\n    on = true\n  view\n    column\n      button testId=\"go\" press=go\n        text \"Go\"\n      box testId=\"b\" width=20 height=20 background-color=(on ? \"{value}\" : \"#000\") box-shadow=(on ? \"0 2px 4px {value}\" : \"0 2px 4px #000\") transition=\"background-color 1s linear, box-shadow 1s linear\"\n"));
        let go = view(&host, "go");
        let batch = host.dispatch_at(go, exact_runner::Event::Press, 100.0);
        let json: serde_json::Value = serde_json::from_str(&batch).unwrap();
        assert_eq!(
            json["motion"], false,
            "unsupported range must be discrete: {batch}"
        );
        let changes = ops(&batch);
        let style = &changes
            .iter()
            .find(|op| op["style"].get("background_color").is_some())
            .unwrap()["style"];
        assert!(style["background_color"]["cs"][0]["v"][0].as_f64().unwrap() > 31.0);
        assert!(
            style["box_shadow"][0]["c"]["cs"][0]["v"][0]
                .as_f64()
                .unwrap()
                > 31.0
        );
    }
}
