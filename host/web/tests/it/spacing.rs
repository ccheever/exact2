//! LLP 1115.000 D10 on the web host: a control's spacing and size are the
//! plan's, the same on every host, so the page is told nothing a native host
//! is not.

use exact_runner::{DataError, DataSource, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot(src: &str) -> (Host<NoData>, String) {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap()
}

/// The `cssText` the first batch creates the node whose testId is `id` with.
fn css_of(host: &Host<NoData>, batch: &str, id: &str) -> String {
    let k = host.runner().kernel();
    let view = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id;
    let at = batch
        .find(&format!("{{\"op\":\"create\",\"id\":{view},"))
        .unwrap_or_else(|| panic!("no create for `{id}` in {batch}"));
    let op = &batch[at..];
    let css = &op[op.find("\"css\":\"").expect("its css") + 7..];
    css[..css.find('"').expect("the css ends")].to_owned()
}

/// D10.1: a checkbox, a radio and a range reach the page with no margin of
/// their own (lowering wrote Chrome's UA margins inline until 2026-10-11),
/// and one the author writes is the control's.
#[test]
fn a_checkbox_a_radio_and_a_range_carry_no_margin() {
    let (host, batch) = boot(
        r#"component App
  view
    row
      input type="checkbox" testId="check"
      input type="checkbox" switch testId="switch"
      input type="radio" testId="radio"
      input type="range" testId="range"
      input type="checkbox" margin-left=6 testId="spaced"
"#,
    );
    for id in ["check", "switch", "radio", "range"] {
        let css = css_of(&host, &batch, id);
        assert!(!css.contains("margin"), "{id}: {css}");
    }
    let spaced = css_of(&host, &batch, "spaced");
    assert!(spaced.contains("margin-left:6px"), "{spaced}");
}

/// D10.2: a range and a determinate `progress` reach the page `width: 100%`,
/// the row lowering implies, so the browser makes them as wide as what holds
/// them by CSS and the host fills nothing itself. An authored `width` is the
/// only one sent; an indeterminate `progress` gets none.
#[test]
fn a_range_and_a_determinate_progress_are_sent_full_width() {
    let (host, batch) = boot(
        r#"component App
  view
    column
      input type="range" testId="range"
      progress value=0.5 testId="bar"
      input type="range" width=90 testId="narrow"
      progress value=0.5 width="auto" testId="auto"
      progress testId="spinner"
"#,
    );
    for id in ["range", "bar"] {
        let css = css_of(&host, &batch, id);
        assert!(css.contains("width:100%"), "{id}: {css}");
    }
    let narrow = css_of(&host, &batch, "narrow");
    assert!(
        narrow.contains("width:90px") && !narrow.contains("100%"),
        "{narrow}"
    );
    let auto = css_of(&host, &batch, "auto");
    assert!(
        auto.contains("width:auto") && !auto.contains("100%"),
        "{auto}"
    );
    let spinner = css_of(&host, &batch, "spinner");
    assert!(!spinner.contains("width:100%"), "{spinner}");
}

/// D10.3: an `hr` reaches the page as one hairline with no margin, a solid
/// top edge one device pixel thick. The wasm page's kernel is told the
/// page's `devicePixelRatio` (`exact_set_display_scale`) and writes the
/// pixels: 1px until it is, half a pixel on a 2x display, and the batch that
/// tells it restyles every hairline. The JS target, which keeps no kernel,
/// is written the sheet's custom property, and the shell's sheet resets the
/// UA's `hr` and holds the property at one device pixel.
#[test]
fn an_hr_is_one_hairline_the_displays_scale_resolves() {
    let (mut host, batch) = boot(
        r#"component App
  view
    column
      hr testId="rule"
      box testId="edged" border-bottom-width="-exact-hairline" border-bottom-style="solid"
      box testId="half" border-bottom-width=0.5 border-bottom-style="solid"
"#,
    );
    let rule = css_of(&host, &batch, "rule");
    assert!(!rule.contains("margin"), "{rule}");
    assert!(
        rule.contains("border-top-width:1px") && rule.contains("border-top-style:solid"),
        "{rule}"
    );
    let k = host.runner().kernel();
    let id = |test_id: &str| k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    let (rule_id, edged_id, half_id) = (id("rule"), id("edged"), id("half"));
    // A 2x display: both hairlines are restyled to half a pixel.
    let told = host.set_display_scale(2.0);
    for view in [rule_id, edged_id] {
        let at = told
            .find(&format!("{{\"op\":\"style\",\"id\":{view},"))
            .unwrap_or_else(|| panic!("no restyle of {view} in {told}"));
        let op = &told[at..told[at..].find('}').map_or(told.len(), |end| at + end)];
        assert!(op.contains("-width:0.5px"), "{op}");
    }
    assert!(
        !told.contains(&format!("{{\"op\":\"style\",\"id\":{half_id},")),
        "an authored 0.5 does not move: {told}"
    );
    assert!(
        !host.set_display_scale(2.0).contains("\"op\":\"style\""),
        "the same scale again restyles nothing"
    );
    // The JS target: the property, for the browser to resolve.
    let node = host.runner().kernel().node(rule_id).unwrap();
    let (css, _) = exact_web::css::css_text_relative(node.style, &[]);
    assert!(
        css.contains("border-top-width:var(--exact-hairline, 1px);"),
        "{css}"
    );
    let sheet = include_str!("../../index.html");
    for rule in [
        "hr { margin: 0; border: 0 none; }",
        ":root { --exact-hairline: 1px; }",
        "@media (min-resolution: 2dppx) { :root { --exact-hairline: .5px; } }",
        "@media (min-resolution: 3dppx) { :root { --exact-hairline: calc(1px / 3); } }",
    ] {
        assert!(sheet.contains(rule), "the shell's sheet says `{rule}`");
    }
}

#[test]
fn svg_hairline_attributes_follow_the_display_scale() {
    let (mut host, first) = boot(
        r##"component App
  state thin = true
  view
    svg width=200 height=100
      defs
        pattern testId="tile" id="tile" patternUnits="userSpaceOnUse" width=8 height="-exact-hairline"
          rect width=4 height=1 fill="#e11d48"
      svg testId="nested" height=(thin ? "-exact-hairline" : "10px")
        rect width=100 height=1 fill="red"
"##,
    );
    let first: serde_json::Value = serde_json::from_str(&first).unwrap();
    let views: Vec<_> = ["tile", "nested"]
        .map(|name| {
            let op = first["ops"]
                .as_array()
                .unwrap()
                .iter()
                .find(|op| op["props"]["data-testid"] == name)
                .unwrap();
            assert_eq!(op["props"]["height"], "1");
            op["id"].clone()
        })
        .into();
    for scale in [1.25, 2.0, 3.0, 1.0] {
        let batch: serde_json::Value =
            serde_json::from_str(&host.set_display_scale(scale)).unwrap();
        let document = host.document().unwrap().root;
        for view in &views {
            let op = batch["ops"]
                .as_array()
                .unwrap()
                .iter()
                .find(|op| op["op"] == "props" && &op["id"] == view)
                .unwrap_or_else(|| panic!("no geometry update for {view}: {batch}"));
            let height = op["set"]["height"].as_str().unwrap();
            assert!(
                (height.parse::<f64>().unwrap() - 1.0 / scale).abs() < 0.000001,
                "{height}"
            );
            assert!(
                document.contains(&format!("height=\"{height}\"")),
                "{document}"
            );
        }
        let unchanged: serde_json::Value =
            serde_json::from_str(&host.set_display_scale(scale)).unwrap();
        assert!(unchanged["ops"].as_array().unwrap().is_empty());
    }
}
