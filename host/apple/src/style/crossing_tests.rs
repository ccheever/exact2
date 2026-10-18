//! Which positioning rows cross to the presenter, which otherwise reads a
//! box's place from its frame: a dialog's insets, a placed popover's
//! margins, a sticky box's vertical insets.

use super::*;

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
