//! What `style_json` and its presented forms send the presenter, row by row.
use super::*;
#[test]
fn backdrop_filters_cross_to_swift_in_order_and_clear_to_none() {
    exact_kernel::style::link_backdrop_filter();
    let mut style = StyleProps::default();
    for (css, expected) in [
        (
            "saturate(0) blur(4px)",
            serde_json::json!({"backdrop_filter": [{"saturate": 0}, {"blur": 4}]}),
        ),
        (
            "blur(4px) saturate(2)",
            serde_json::json!({"backdrop_filter": [{"blur": 4}, {"saturate": 2}]}),
        ),
        (
            "saturate(1)",
            serde_json::json!({"backdrop_filter": [{"saturate": 1}]}),
        ),
        ("none", serde_json::json!({})),
    ] {
        style
            .set_dynamic(StyleId::BackdropFilter, &StyleValue::Text(css.into()))
            .unwrap();
        let (json, skipped) = style_json(&style, &Env::default());
        assert!(skipped.is_empty());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap(),
            expected
        );
    }
}

#[test]
fn exclusion_rows_wait_for_resolved_shape_batches() {
    let mut s = StyleProps::default();
    for (id, value) in [
        (StyleId::WrapFlow, "both"),
        (StyleId::ShapeOutside, "circle()"),
        (StyleId::ShapeMargin, "8px"),
    ] {
        s.set_dynamic(id, &StyleValue::Text(value.into())).unwrap();
    }
    assert_eq!(style_json(&s, &Env::default()), ("{}".into(), vec![]));
}

/// CSS `visibility` is inherited. A box that does not set it still
/// receives the computed value, so the presenter can hide that box's
/// own paint without hiding a sibling that sets `visible`.
#[test]
fn a_box_inherits_visibility() {
    use exact_kernel::{Kernel, MonospaceMeasurer, Op};
    let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let mut hidden = StyleProps::default();
    hidden
        .set_dynamic(StyleId::Visibility, &StyleValue::Text("hidden".into()))
        .unwrap();
    let mut shown = StyleProps::default();
    shown
        .set_dynamic(StyleId::Visibility, &StyleValue::Text("visible".into()))
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
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::View,
                },
                Op::CreateView {
                    id: 3,
                    node_type: NodeType::View,
                },
                Op::CreateView {
                    id: 4,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(hidden),
                },
                Op::SetStyle {
                    id: 2,
                    patch: Box::new(shown),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
            ],
        )
        .unwrap();
    let json = |id: u32| -> serde_json::Value {
        serde_json::from_str(&style_json_for(&kernel.node(id).unwrap(), &Env::default()).0).unwrap()
    };
    assert_eq!(json(1)["visibility"], "hidden");
    assert_eq!(json(2)["visibility"], "visible");
    assert_eq!(json(3)["visibility"], "hidden");
    assert!(
        json(4).get("visibility").is_none(),
        "initial visible is not sent"
    );
}

/// LLP 1115.000 D2: the dictionary names the rows its author wrote on
/// the node. A colour the compiler implied or a child only inherits
/// crosses as a value and is not named; a platform colour and a colour
/// equal to the parent's are named like any other; a written row no
/// presenter is sent (a margin) is not.
#[test]
fn the_dictionary_names_the_rows_its_author_wrote() {
    use exact_kernel::{Kernel, MonospaceMeasurer, Op};
    let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let patch = |rows: &[(StyleId, &str, bool)]| {
        let mut p = StyleProps::default();
        for (id, value, written) in rows {
            p.set_dynamic(*id, &StyleValue::Text((*value).into()))
                .unwrap();
            if *written {
                p.written.set(*id);
            }
        }
        Box::new(p)
    };
    let (red, view) = ("-exact-system-red", NodeType::View);
    let mut ops: Vec<Op> = (1..=5)
        .map(|id| Op::CreateView {
            id,
            node_type: view,
        })
        .collect();
    let page = [
        (StyleId::TextColor, red, true),
        (StyleId::MarginTop, "8px", true),
        (StyleId::BackgroundColor, "#ffffff", false),
    ];
    let (same, implied) = (
        [(StyleId::TextColor, red, true)],
        [(StyleId::TextColor, "#111111", false)],
    );
    for (id, rows) in [(1, &page[..]), (3, &same[..]), (4, &implied[..])] {
        let patch = patch(rows);
        ops.push(Op::SetStyle { id, patch });
    }
    let children = vec![2, 3, 4];
    ops.push(Op::SetChildren { id: 1, children });
    kernel.apply(0, 1, &ops).unwrap();
    let json = |id: u32| -> serde_json::Value {
        serde_json::from_str(&style_json_for(&kernel.node(id).unwrap(), &Env::default()).0).unwrap()
    };
    assert_eq!(json(1)["written"], serde_json::json!(["text_color"]));
    assert_eq!(json(1)["text_color"]["sys"], "systemRedColor");
    assert_eq!(json(2)["text_color"], json(1)["text_color"], "inherited");
    assert!(json(2).get("written").is_none(), "{}", json(2));
    assert_eq!(
        json(3)["written"],
        serde_json::json!(["text_color"]),
        "the parent's colour, written again"
    );
    assert!(json(4).get("written").is_none(), "the compiler's row");
    assert!(json(5).get("written").is_none(), "no row");
}

/// LLP 1115 D4: a colour moving on a field is its own, so `"written"`
/// names it (a field reads its colour only there); an ancestor's moving
/// colour, which it inherits, is not named.
#[test]
fn an_own_moving_colour_is_written_and_an_inherited_one_is_not() {
    use exact_kernel::{Kernel, MonospaceMeasurer, Op};
    let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let node_type = NodeType::TextInput;
    kernel
        .apply(0, 1, &[Op::CreateView { id: 1, node_type }])
        .unwrap();
    let mut shown = Shown::default();
    shown.set(
        Property::Color,
        Some(exact_motion::Value::rgba8(0, 0, 255, 255)),
    );
    let node = kernel.node(1).unwrap();
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let own = json(style_json_presented(&node, &Env::default(), &shown).0);
    assert_eq!(own["written"], serde_json::json!(["text_color"]), "{own}");
    assert_eq!(own["text_color"], serde_json::json!([0, 0, 255, 255]));
    let inherited = json(style_json_inherited(&node, &Env::default(), &shown).0);
    assert!(inherited.get("written").is_none(), "{inherited}");
    let still = json(style_json_presented(&node, &Env::default(), &Shown::default()).0);
    assert!(still.get("written").is_none(), "{still}");
    // Off a field, a transition's colour is not the node's say (a
    // spinner under a changing parent keeps its own ink).
    let view = NodeType::View;
    kernel
        .apply(
            0,
            2,
            &[Op::CreateView {
                id: 2,
                node_type: view,
            }],
        )
        .unwrap();
    let other = json(style_json_presented(&kernel.node(2).unwrap(), &Env::default(), &shown).0);
    assert!(other.get("written").is_none(), "{other}");
    // A leaving view whose exit's keyframes name `color`.
    let last = r#"{"text_color":[0,0,0,255],"written":["background_color"]}"#;
    let left = json(restyle_presented(last, &Env::default(), &shown, true));
    assert_eq!(
        left["written"],
        serde_json::json!(["text_color", "background_color"])
    );
    let left = json(restyle_presented("{}", &Env::default(), &shown, true));
    assert_eq!(left["written"], serde_json::json!(["text_color"]));
    let left = json(restyle_presented(last, &Env::default(), &shown, false));
    assert_eq!(left["written"], serde_json::json!(["background_color"]));
}

/// Quarters are written as `{n}` writes them.
#[test]
fn quarters_spell_as_display_does() {
    for n in [
        0.25f32,
        0.5,
        0.75,
        1.5,
        -0.5,
        -2.25,
        123.75,
        9_999_999.5,
        0.1,
        1.0 / 3.0,
        -0.0,
        7.0,
    ] {
        let mut s = String::new();
        push_num(&mut s, n);
        let want = if n.fract() == 0.0 {
            format!("{}", n as i64)
        } else {
            format!("{n}")
        };
        assert_eq!(s, want, "{n}");
    }
}

/// LLP 1061 D2, D7: the press scale and paint are the presenter's to
/// show, so they cross (a number, colours as any colour row's), where
/// the engine's own `scale` never does.
#[test]
fn the_press_rows_cross_and_the_motion_scale_does_not() {
    let mut s = StyleProps::default();
    for (row, value) in [
        (StyleId::PressScale, StyleValue::Number(0.97)),
        (StyleId::Scale, StyleValue::Number(2.0)),
        (StyleId::PressOpacity, StyleValue::Number(0.5)),
        (
            StyleId::PressBackgroundColor,
            StyleValue::Text("light-dark(#fff, #000)".into()),
        ),
        (StyleId::PressColor, StyleValue::Text("#ff0000".into())),
    ] {
        s.set_dynamic(row, &value).unwrap();
    }
    let json = r#"{"press_scale":0.97,"press_opacity":0.5,"press_background_color":[[255,255,255,255],[0,0,0,255]],"press_color":[255,0,0,255]}"#;
    assert_eq!(style_json(&s, &Env::default()), (json.into(), vec![]));
}

/// LLP 1066: a gradient crosses as its shape and ready-to-mix stops; a
/// `light-dark()` one carries both appearances, and `none` nothing.
#[test]
fn a_gradient_crosses_as_shape_and_stops() {
    let json = |css: &str| {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::BackgroundImage, &StyleValue::Text(css.into()))
            .unwrap();
        style_json(&s, &Env::default()).0
    };
    assert_eq!(
        json("linear-gradient(to right, transparent, #fff 40%)"),
        r#"{"background_image":{"linear":90,"stops":[0,255,255,255,0,0.4,255,255,255,255,1,255,255,255,255]}}"#
    );
    assert_eq!(
        json("linear-gradient(to top left, light-dark(#000, #fff), #f00)"),
        r#"{"background_image":{"corner":[0,0],"stops":[0,0,0,0,255,1,255,0,0,255],"dark":[0,255,255,255,255,1,255,0,0,255]}}"#
    );
    assert_eq!(
        json("radial-gradient(circle closest-side at 10px 25%, #000, #fff)"),
        r#"{"background_image":{"radial":[1,0,0,10,25,0],"stops":[0,0,0,0,255,1,255,255,255,255]}}"#
    );
    assert_eq!(json("none"), "{}");
}

#[test]
fn a_moving_colour_leaves_system_colours_and_the_rest_stay() {
    // LLP 1077 D13: the row the motion paints is no longer a system
    // colour; an untouched one keeps its name.
    let last = r#"{"text_color":[[0,0,0,255],[255,255,255,255]],"background_color":[[120,120,128,51],[120,120,128,92]],"system_colors":{"text_color":"-apple-system-label","background_color":"-exact-fill"}}"#;
    let mut shown = Shown::default();
    shown.set(
        Property::BackgroundColor,
        Some(exact_motion::Value::rgba8(255, 0, 0, 128)),
    );
    let json: serde_json::Value =
        serde_json::from_str(&restyle_presented(last, &Env::default(), &shown, false)).unwrap();
    assert_eq!(
        json["system_colors"],
        serde_json::json!({"text_color": "-apple-system-label"})
    );
    assert_eq!(
        json["background_color"],
        serde_json::json!([255, 0, 0, 128])
    );
    shown.set(
        Property::Color,
        Some(exact_motion::Value::rgba8(0, 0, 255, 255)),
    );
    let json: serde_json::Value =
        serde_json::from_str(&restyle_presented(last, &Env::default(), &shown, false)).unwrap();
    assert!(json.get("system_colors").is_none(), "{json}");
}
