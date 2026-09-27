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
