//! @ref LLP 1043.000 §3 D1–D4 — authoring and state-to-layout commits.
use exact_kernel::{FlowShape, Kernel, Offer, WrapFlow};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
}

fn source(attrs: &str) -> String {
    format!("component App\n  view\n    view width=600 height=400\n      text \"Words\" width=600 height=400 testId=\"p\"\n      view {attrs}\n")
}
#[test]
fn auto_values_follow_each_rows_codec_and_vocabulary() {
    // @ref LLP 1017.000 P1a — enum keywords use the kernel's vocabulary.
    for (property, reason) in [
        ("display", "one of \"block\", \"flex\", \"grid\", \"none\""),
        ("font-size", "number"),
        ("opacity", "number"),
    ] {
        let error = contract::compile(&source(&format!("{property}=\"auto\""))).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{property}: {error}");
        assert_eq!(
            error.message,
            format!("`{property}=\"auto\"` is not a valid `{property}`: expected {reason}"),
            "{property}"
        );
    }
    contract::compile(&source(
        "overflow=\"auto\" width=\"auto\" height=\"auto\" caret-color=\"auto\" wrap-flow=\"auto\" touch-action=\"auto\"",
    ))
    .unwrap();
}

#[test]
fn supported_css_forms_and_precise_refusals() {
    for shape in [
        "none",
        "circle()",
        "circle(30% at 50% 60%)",
        "ellipse(20px 30px)",
        "inset(10px round 4px)",
        "polygon(0 0, 120px 0, 0 120px)",
    ] {
        contract::compile(&source(&format!("position=\"absolute\" wrap-flow=\"both\" shape-outside=\"{shape}\" shape-margin=\"8px\""))).unwrap();
    }
    contract::compile(&source("wrap-flow=\"auto\" shape-margin=0")).unwrap();
    for attrs in [
        "wrap-flow=\"both\"",
        "position=\"relative\" wrap-flow=\"both\"",
    ] {
        let error = contract::compile(&source(attrs)).unwrap_err().to_string();
        assert!(
            error.contains("`wrap-flow: both` requires `position: absolute` in exact2 v1"),
            "{error}"
        );
    }
    for wrap in ["start", "end", "minimum", "maximum", "clear", "bogus"] {
        let error = contract::compile(&source(&format!("wrap-flow=\"{wrap}\"")))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("CSS Exclusions defines it; exact2 v1 implements `both`"),
            "{error}"
        );
    }
    for value in ["url(dancer.png)", "path('M 0 0')", "circle(NaN)"] {
        let error = contract::compile(&source(&format!("shape-outside=\"{value}\"")))
            .unwrap_err()
            .to_string();
        assert!(error.contains("expected none, circle(), ellipse(), inset() with one round radius, or polygon() with at most 64 vertices; lengths are points/px or percentages"),"{error}");
    }
    let error = contract::compile(&source("shape-margin=\"10%\""))
        .unwrap_err()
        .to_string();
    assert!(error.contains("percentage `shape-margin` is not implemented in exact2 v1; use a nonnegative length in points/px"),"{error}");
    let error = contract::compile(&source("shape-margin=-1"))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("nonnegative finite length in points/px"),
        "{error}"
    );
}
#[test]
fn every_new_row_and_offsets_update_through_one_commit() {
    let source = r#"component App
  state x = 100
  state y = 80
  state wrap = "auto"
  state outline = "none"
  state margin = 0
  action move()
    x = 200
    y = 90
    wrap = "both"
    outline = "circle()"
    margin = 8
  view
    view width=600 height=400
      text "Words" width=600 height=400 testId="p"
      view position="absolute" left=x top=y width=120 height=120 wrap-flow=wrap shape-outside=outline shape-margin=margin testId="ball"
"#;
    let p = contract::compile(source).unwrap();
    let p = exact_plan::Plan::decode(&p.encode()).unwrap();
    let mut r = Runner::boot(p, NoData, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let root = r.roots()[0];
    let leaf = r.kernel().find_by_test_id("p")[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(600., 400.))
        .unwrap();
    assert!(r
        .kernel()
        .node_by_key(leaf)
        .unwrap()
        .flow_shapes()
        .is_empty());
    let commit = r.act("move", Vec::<Value>::new()).unwrap();
    assert!(commit.layout_invalidated);
    let layout = r
        .kernel_mut()
        .compute_layout(root, Offer::definite(600., 400.))
        .unwrap();
    assert_eq!(layout.flow_changed, vec![leaf]);
    assert!(!layout.changed.contains(&leaf));
    assert_eq!(
        r.kernel().node_by_key(leaf).unwrap().flow_shapes(),
        &[FlowShape::Circle {
            cx: 260.,
            cy: 150.,
            r: 68.
        }]
    );
    let ball = r.kernel().find_by_test_id("ball")[0];
    assert_eq!(
        r.kernel().node_by_key(ball).unwrap().style.wrap_flow,
        WrapFlow::Both
    );
    let json = exact_runner::agent::node(&r, r.kernel().node_by_key(leaf).unwrap().id);
    assert!(
        json.contains("\"flow_shapes\":[{\"kind\":\"Circle\",\"cx\":260,\"cy\":150,\"r\":68}]"),
        "{json}"
    );
}

#[test]
fn exclusions_require_their_parent_to_contain_them() {
    for parent in ["", "position=\"static\""] {
        let source = format!("component App\n  view\n    box\n      box {parent}\n        when true\n          box position=\"absolute\" wrap-flow=\"both\"\n");
        let error = contract::compile(&source).unwrap_err();
        assert!(
            error
                .message
                .contains("give the parent `position: relative`"),
            "{error}"
        );
    }
    for parent in [
        "position=\"relative\"",
        "overflow=\"hidden\"",
        "translate=\"0px 0px\"",
    ] {
        contract::compile(&format!("component App\n  view\n    box\n      box {parent}\n        box position=\"absolute\" wrap-flow=\"both\"\n")).unwrap();
    }
    contract::compile("component App\n  view\n    box position=\"absolute\" wrap-flow=\"both\"\n")
        .unwrap();
}
