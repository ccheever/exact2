//! Which positioning rows cross to the presenter, which otherwise reads a
//! box's place from its frame: a dialog's insets, a placed popover's
//! margins, a sticky box's vertical insets. A width or height does not
//! cross, but that its author wrote one does (`"written"`): a presenter tells
//! an author's box from one a sheet gave (a grouped row's leading symbol,
//! `GroupedCustomRowsIOS`). So does an `animation`, which the engine runs: a
//! glass its author animates in does not also materialize.

use super::*;
use exact_runner::{DataError, DataSource, Runner};

#[test]
fn only_dialogs_keep_viewport_positioning_rows() {
    use exact_kernel::{Kernel, MonospaceMeasurer, Op, PropId};
    let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let mut style = StyleProps::default();
    style
        .set_dynamic(StyleId::Left, &StyleValue::Number(12.0))
        .unwrap();
    style
        .set_dynamic(StyleId::Bottom, &StyleValue::Text("calc(10% + 8px)".into()))
        .unwrap();
    kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(style),
                },
            ],
        )
        .unwrap();
    let ordinary: serde_json::Value =
        serde_json::from_str(&style_json_for(&kernel.node(1).unwrap(), &Env::default()).0).unwrap();
    assert!(ordinary.get("left").is_none());
    assert!(ordinary.get("bottom").is_none());
    kernel
        .apply(
            0,
            2,
            &[Op::SetProp {
                id: 1,
                prop: PropId::SemanticTag,
                value: "dialog".into(),
            }],
        )
        .unwrap();
    let dialog: serde_json::Value =
        serde_json::from_str(&style_json_for(&kernel.node(1).unwrap(), &Env::default()).0).unwrap();
    assert_eq!(dialog["left"], 12);
    assert_eq!(dialog["bottom"], serde_json::json!({ "pct": 10, "px": 8 }));
}

/// A sticky box carries its vertical insets, which say whether it
/// sticks as its scroller scrolls (an iOS header's search field), and
/// not its horizontal ones.
#[test]
fn a_sticky_box_carries_its_vertical_insets() {
    use exact_kernel::{Kernel, MonospaceMeasurer, Op};
    let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let mut style = StyleProps::default();
    style
        .set_dynamic(StyleId::PositionType, &StyleValue::Text("sticky".into()))
        .unwrap();
    style
        .set_dynamic(StyleId::Left, &StyleValue::Number(4.0))
        .unwrap();
    let create = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        },
    ];
    kernel.apply(0, 1, &create).unwrap();
    let json = |kernel: &Kernel| -> serde_json::Value {
        serde_json::from_str(&style_json_for(&kernel.node(1).unwrap(), &Env::default()).0).unwrap()
    };
    let sideways = json(&kernel);
    assert!(sideways.get("left").is_none(), "{sideways}");
    assert!(sideways.get("top").is_none(), "{sideways}");
    let mut top = StyleProps::default();
    top.set_dynamic(StyleId::Top, &StyleValue::Number(0.0))
        .unwrap();
    kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(top),
            }],
        )
        .unwrap();
    assert_eq!(json(&kernel)["top"], 0);
}

/// LLP 1021 "Placement": a popover placed by `position-area` carries its
/// margins to the host, which aligns its margin box; without the row
/// they stay the kernel's, as every other box's.
#[test]
fn a_placed_popover_carries_its_margins() {
    use exact_kernel::{Kernel, MonospaceMeasurer, Op};
    let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let mut style = StyleProps::default();
    style
        .set_dynamic(StyleId::MarginBottom, &StyleValue::Number(12.0))
        .unwrap();
    style
        .set_dynamic(StyleId::MarginLeft, &StyleValue::Auto)
        .unwrap();
    let create = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        },
    ];
    kernel.apply(0, 1, &create).unwrap();
    let json = |kernel: &Kernel| -> serde_json::Value {
        serde_json::from_str(&style_json_for(&kernel.node(1).unwrap(), &Env::default()).0).unwrap()
    };
    assert!(json(&kernel).get("margin_bottom").is_none());
    let mut area = StyleProps::default();
    area.set_dynamic(StyleId::PositionArea, &StyleValue::Text("top".into()))
        .unwrap();
    area.set_dynamic(
        StyleId::PositionTryFallbacks,
        &StyleValue::Text("flip-block, flip-inline, flip-block flip-inline".into()),
    )
    .unwrap();
    kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(area),
            }],
        )
        .unwrap();
    let placed = json(&kernel);
    assert_eq!(placed["position_area"], "top", "{placed}");
    assert_eq!(
        placed["position_try_fallbacks"],
        "flip-block, flip-inline, flip-block flip-inline"
    );
    assert_eq!(placed["margin_bottom"], 12, "{placed}");
    assert_eq!(placed["margin_left"], "auto", "{placed}");
}

struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn an_authored_size_is_named_written_and_a_sheets_is_not() {
    let source = r#"component App
  view
    column testId="root" width=402 height=874
      list appearance="auto" testId="list" flex=1
        section
          row testId="sheet"
            image "symbol:star" testId="sheet-star"
            text "Favorites"
          row testId="sized"
            image "symbol:star" width=32 height=32 testId="sized-star"
            text "Favorites"
"#;
    let plan = contract::bake(contract::compile(source).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::new(Box::new(exact_kernel::MonospaceMeasurer::default())),
        Default::default(),
        "/",
    )
    .unwrap();
    let written = |t: &str| -> serde_json::Value {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&style_json_for(&node, &Env::default()).0).unwrap();
        assert!(json.get("width").is_none(), "the size itself is layout's");
        json.get("written")
            .cloned()
            .unwrap_or(serde_json::json!([]))
    };
    assert_eq!(
        written("sized-star"),
        serde_json::json!(["width", "height"])
    );
    assert_eq!(written("sheet-star"), serde_json::json!([]));
}

#[test]
fn an_authored_animation_is_named_written_and_does_not_cross() {
    let source = r#"keyframes rise
  from opacity=0

component App
  view
    column testId="root"
      text "Badge" testId="moving" animation="rise 200ms ease-out"
      text "Still" testId="still"
"#;
    let plan = contract::bake(contract::compile(source).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::new(Box::new(exact_kernel::MonospaceMeasurer::default())),
        Default::default(),
        "/",
    )
    .unwrap();
    let json = |t: &str| -> serde_json::Value {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
        serde_json::from_str(&style_json_for(&node, &Env::default()).0).unwrap()
    };
    let moving = json("moving");
    assert!(moving.get("animation").is_none(), "{moving}");
    assert_eq!(
        moving["written"],
        serde_json::json!(["animation"]),
        "{moving}"
    );
    assert!(json("still").get("written").is_none());
}
