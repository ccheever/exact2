//! A `path` on the web (LLP 1065 D5): an inline `<svg>` whose stroke is one
//! `<path>` per subpath, each dashed from the node's registered
//! `--exact-stroke-start`/`--exact-stroke-end` and its place in the whole —
//! so the browser draws the subpaths one after another, in pen order, as
//! it transitions or keyframes the two numbers.

use exact_kernel::vector::PathData;
use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

const D: &str = "M12 70c10-40 30-55 42-40s-6 45-20 35 14-40 38-38c14 2 6 30 18 28s10-24 22-20 M110 40q20-30 40 0t40 0 M20 88h160";

fn boot() -> (Host<NoData>, String) {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/path.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap()
}

/// The `create` op of the node with this test id, as batch JSON text.
fn create<'a>(batch: &'a str, test_id: &str) -> &'a str {
    let at = batch
        .find(&format!("\"data-testid\":\"{test_id}\""))
        .unwrap_or_else(|| panic!("{test_id}: {batch}"));
    let start = batch[..at].rfind("{\"op\":\"create\"").unwrap();
    let end = batch[at..].find("{\"op\":").map_or(batch.len(), |e| at + e);
    &batch[start..end]
}

#[test]
fn the_stroke_is_split_per_subpath_at_its_place_in_the_whole() {
    let (_, first) = boot();
    let mark = create(&first, "mark");
    assert!(mark.contains("\"tag\":\"div\""), "{mark}");
    // The kernel's data replaces the authored attributes.
    assert!(
        !mark.contains("data-pathdata") && !mark.contains("data-viewbox"),
        "{mark}"
    );
    assert!(
        mark.contains(r#"<svg data-exact-path viewBox=\"0 0 200 100\""#),
        "{mark}"
    );
    let data = PathData::parse(D);
    let lengths: Vec<f64> = data.lengths().collect();
    assert_eq!(lengths.len(), 3);
    let mut at = 0.0f64;
    for length in &lengths {
        let piece = format!(
            r#"pathLength=\"{}\" fill=\"none\" style=\"--a:{};--l:{}\""#,
            exact_num::Shortest32(*length as f32),
            exact_num::Shortest32(at as f32),
            exact_num::Shortest32(*length as f32)
        );
        assert!(mark.contains(&piece), "{piece}\n{mark}");
        at += length;
    }
    assert!(
        mark.contains(&format!("--L:{}", exact_num::Shortest32(at as f32))),
        "{mark}"
    );
    // The fill is the whole path under the strokes (fill:none inherits here).
    assert!(mark.contains(r#"stroke=\"none\"/>"#), "{mark}");
    assert!(mark.contains("@property --exact-stroke-end"), "{mark}");
    // Decorative without a label; an image with one.
    assert!(mark.contains("\"aria-hidden\":\"true\""), "{mark}");
    let stamp = create(&first, "stamp");
    assert!(stamp.contains("\"role\":\"img\"") && stamp.contains("\"aria-label\":\"Signed\""));
    assert!(!stamp.contains("\"aria-hidden\""), "{stamp}");
}

#[test]
fn painting_and_the_stroke_fractions_are_css_the_browser_animates() {
    let (mut host, first) = boot();
    let mark = create(&first, "mark");
    for declaration in [
        "position:relative;",
        "stroke:light-dark(",
        "fill:none;",
        "stroke-width:9px;",
        "stroke-linecap:round;",
        "stroke-linejoin:round;",
        "--exact-stroke-end:0;",
        "transition:--exact-stroke-end 1.6s cubic-bezier(0.45,0,0.55,1) 0s",
    ] {
        assert!(mark.contains(declaration), "{declaration}\n{mark}");
    }
    // The keyframes rule animates the registered number.
    assert!(
        first.contains("{0%{--exact-stroke-end:0;}100%{--exact-stroke-end:1;}}"),
        "{first}"
    );
    let k = host.runner().kernel();
    let sign = k.node_by_key(k.find_by_test_id("sign")[0]).unwrap().id;
    let signed = host.dispatch(sign, Event::Press);
    assert!(signed.contains("--exact-stroke-end:1;"), "{signed}");
    // Only the style changed: the markup is not rebuilt.
    assert!(!signed.contains("pathMarkup"), "{signed}");
}

fn boot_source(src: &str) -> (Host<NoData>, String) {
    let plan = contract::compile(src).unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap()
}

/// A `spring()` on a stroke fraction plays as its curve from rest, a CSS
/// `linear()` easing of the registered number (LLP 1062 D3, LLP 1065): the
/// value no longer jumps on the web.
#[test]
fn a_spring_on_a_stroke_fraction_is_a_linear_easing() {
    let (mut host, _) = boot_source(
        "component App\n  state on = false\n  action go writes on\n    on = true\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      path testId=\"p\" d=\"M0 0 H10\" stroke=\"#000\" width=10 height=10 stroke-end=(on ? 1 : 0) transition=\"stroke-end spring(170, 26, 1)\"\n",
    );
    let k = host.runner().kernel();
    let go = k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id;
    let batch = host.dispatch(go, Event::Press);
    assert!(
        batch.contains("transition:--exact-stroke-end 0.9") && batch.contains("s linear(0 0%,"),
        "{batch}"
    );
}

/// SVG's painting vocabulary as CSS the browser applies, `fill` and
/// `stroke` transitioning as colours (LLP 1065).
#[test]
fn the_rest_of_svg_painting_is_css() {
    let (_, first) = boot_source(
        "component App\n  view\n    path testId=\"p\" d=\"M0 0 H10\" width=10 height=10 fill=\"currentColor\" fill-rule=\"evenodd\" stroke=\"#000\" stroke-miterlimit=8 stroke-dasharray=\"4, 2\" stroke-dashoffset=1 transition=\"fill 1s, stroke 200ms\"\n",
    );
    let p = create(&first, "p");
    for declaration in [
        "fill:currentcolor;",
        "fill-rule:evenodd;",
        "stroke-miterlimit:8;",
        "stroke-dasharray:4 2;",
        "stroke-dashoffset:1px;",
        "transition:fill 1s ease 0s,stroke 0.2s ease 0s",
    ] {
        assert!(p.contains(declaration), "{declaration}\n{p}");
    }
}

/// The view box's fit, a zero-length subpath's dot, a dashed stroke masked
/// by the trim, and a non-scaling stroke's pixels (LLP 1065).
#[test]
fn aspect_dots_dashes_and_non_scaling_strokes_in_the_markup() {
    let (_, first) = boot_source(
        "component App\n  view\n    column\n      path testId=\"plain\" d=\"M5 5 Z M0 0 H10 M20 20\" viewBox=\"0 0 10 20\" preserveAspectRatio=\"xMinYMax slice\" stroke=\"#000\" width=10 height=10\n      path testId=\"dashed\" d=\"M0 0 H10\" stroke=\"#000\" stroke-dasharray=\"2 1\" width=10 height=10\n      path testId=\"fixed\" d=\"M0 0 H10\" viewBox=\"0 0 10 20\" vector-effect=\"non-scaling-stroke\" stroke=\"#000\" width=10 height=10\n",
    );
    let plain = create(&first, "plain");
    assert!(
        plain.contains(r#"viewBox=\"0 0 10 20\" preserveAspectRatio=\"xMinYMax slice\""#),
        "{plain}"
    );
    // The dot sits at its place in the whole; the lone moveto draws nothing.
    assert!(
        plain.contains(r#"<path data-dot d=\"M 5 5 L 5 5\" fill=\"none\" style=\"--a:0\"/>"#),
        "{plain}"
    );
    assert!(!plain.contains("M 20 20\\\" pathLength"), "{plain}");
    assert!(
        plain.contains("--k:1") && !plain.contains("<mask"),
        "{plain}"
    );
    let dashed = create(&first, "dashed");
    assert!(dashed.contains("<mask id=\\\"exact-path-"), "{dashed}");
    assert!(
        dashed.contains(r##"pathLength=\"10\" fill=\"none\" stroke=\"#fff\""##),
        "{dashed}"
    );
    assert!(
        dashed.contains("fill=\\\"none\\\" mask=\\\"url(#exact-path-"),
        "{dashed}"
    );
    let fixed = create(&first, "fixed");
    assert!(
        fixed.contains("--k:calc(min(100cqw / 10, 100cqh / 20) / 1px)"),
        "{fixed}"
    );
    assert!(!fixed.contains("pathLength"), "{fixed}");
    assert!(
        fixed.contains("container-type:size;")
            && fixed.contains("vector-effect:non-scaling-stroke;"),
        "{fixed}"
    );
}
