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
const BRAND: &str = "platform-color(ios colorsTestBrandColor, macos colorsTestBrandColor, light-dark(#102030, #405060))";

fn app() -> String {
    format!(
        "component A\n  view\n    column\n      box testId=\"wash\" width=20 height=20 background-image=\"linear-gradient({BRAND}, #ffffff)\"\n      svg width=10 height=10 viewBox=\"0 0 10 10\"\n        circle cx=5 cy=5 r=5 fill=\"{BRAND}\"\n      text \"hi\" color=\"{BRAND}\" transition=\"color 300ms linear\"\n"
    )
}

fn ops(batch: &str) -> Vec<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"].as_array().cloned().unwrap_or_default()
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
    assert!(ops_of(&again).is_empty(), "{again}");
    // No report: the fallback pair again.
    host.set_colors(Vec::new());
    assert_eq!(c.resolve(false), Color(0x1020_30ff));
}

fn ops_of(batch: &str) -> Vec<serde_json::Value> {
    ops(batch)
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
        "component A\n  state on = false\n  view\n    column\n      box width=20 height=20 background-image=(on ? \"linear-gradient(platform-color(ios colorsTestLateSeedColor, macos colorsTestLateSeedColor, #010203), #ffffff)\" : \"none\")\n",
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
