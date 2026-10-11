//! `props_of`, `tag_of` and `host_css_of` on single nodes: what the DOM is told.

use exact_kernel::{NodeFacts, NodeType, PropId, PropKind, PropList, PropValue, StyleProps};

fn written(
    node_type: NodeType,
    prop: PropId,
    value: PropValue,
) -> super::SortedMap<String, String> {
    let style = StyleProps::default();
    let mut props = PropList::new();
    props.set(prop, value);
    super::props_of(&NodeFacts {
        id: 1,
        node_type,
        style: &style,
        props: &props,
        is_root: false,
        inline_run: false,
    })
}

/// An SVG filter primitive's own prop: the compiler sets it only on an
/// `fe*` or `filter` element, which writes it under its own name.
fn svg_only(prop: PropId) -> bool {
    let name = prop.name();
    name.starts_with("pointsAt")
        || name.starts_with("specular")
        || name.ends_with("ChannelSelector")
        || matches!(
            name,
            "amplitude"
                | "azimuth"
                | "baseFrequency"
                | "bias"
                | "diffuseConstant"
                | "divisor"
                | "edgeMode"
                | "elevation"
                | "exponent"
                | "filterUnits"
                | "in"
                | "in2"
                | "intercept"
                | "k1"
                | "k2"
                | "k3"
                | "k4"
                | "kernelMatrix"
                | "limitingConeAngle"
                | "mode"
                | "numOctaves"
                | "operator"
                | "feOrder"
                | "preserveAlpha"
                | "primitiveUnits"
                | "result"
                | "seed"
                | "slope"
                | "stdDeviation"
                | "stitchTiles"
                | "surfaceScale"
                | "tableValues"
                | "targetX"
                | "targetY"
                | "values"
        )
}

/// LLP 1069.001, amended 2026-10-07: an indeterminate progress is a box
/// the base sheet draws a ring in (HTML's own draws a bar), ARIA's role
/// and busy state its attributes, sized as the kernel's 20 × 20 leaf.
#[test]
fn a_progress_is_a_busy_progressbar_box_the_page_draws_a_ring_in() {
    let style = StyleProps::default();
    let mut props = PropList::default();
    props.set(PropId::Type, PropValue::Str("progress".into()));
    props.set(
        PropId::AccessibilityRole,
        PropValue::Str("progressbar".into()),
    );
    props.set(PropId::AccessibilityBusy, PropValue::Bool(true));
    let facts = NodeFacts {
        id: 1,
        node_type: NodeType::Control,
        style: &style,
        props: &props,
        is_root: false,
        inline_run: false,
    };
    assert_eq!(super::tag_of(&facts, false), "div");
    assert_eq!(super::tag_of(&facts, true), "span", "in a button");
    let out = super::props_of(&facts);
    let get = |k: &str| out.get(k).map(String::as_str);
    assert_eq!(get("role"), Some("progressbar"));
    assert_eq!(get("aria-busy"), Some("true"));
    assert_eq!(get("data-exact-progress"), Some(""));
    assert_eq!(get("type"), None, "{out:?}");
    let css = super::host_css_of(&facts, None, String::new(), "div");
    assert!(
        css.contains("container-type:size;contain-intrinsic-size:20px 20px;display:grid;"),
        "{css}"
    );
    assert!(css.contains("justify-self:start;"), "a block's: {css}");
    // A hidden one stays hidden: its grid is not written over `none`.
    let hidden_style = StyleProps {
        display: exact_kernel::Display::None,
        ..StyleProps::default()
    };
    let hidden = NodeFacts {
        style: &hidden_style,
        ..facts
    };
    let css = super::host_css_of(&hidden, None, String::new(), "div");
    assert!(!css.contains("display:grid"), "{css}");
    // In a grid or a flex container it stretches as the kernel's does.
    for display in [exact_kernel::Display::Grid, exact_kernel::Display::Flex] {
        let grid = StyleProps {
            display,
            ..StyleProps::default()
        };
        let parent = NodeFacts {
            id: 2,
            node_type: NodeType::View,
            style: &grid,
            props: &PropList::default(),
            is_root: false,
            inline_run: false,
        };
        let css = super::host_css_of(&facts, Some(&parent), String::new(), "div");
        assert!(!css.contains("justify-self"), "{display:?}: {css}");
    }
    assert!(include_str!("../index.html").contains("[data-exact-progress]::before"));
}

/// A link to an absolute URL opens outside the app, as natively, unless
/// its `target` is authored; a path stays (chat F11, hn-reader F3).
#[test]
fn a_link_out_of_the_app_opens_a_new_browsing_context() {
    let link = |href: &str, target: Option<&str>| {
        let style = StyleProps::default();
        let mut props = PropList::new();
        props.set(PropId::Href, PropValue::Str(href.into()));
        if let Some(t) = target {
            props.set(PropId::Target, PropValue::Str(t.into()));
        }
        let out = super::props_of(&NodeFacts {
            id: 1,
            node_type: NodeType::Pressable,
            style: &style,
            props: &props,
            is_root: false,
            inline_run: false,
        });
        let get = |k: &str| out.get(k).cloned();
        (get("target"), get("rel"))
    };
    let out = (Some("_blank".into()), Some("external noopener".into()));
    assert_eq!(link("https://example.com/a", None), out);
    assert_eq!(link("//example.com/a", None), out);
    assert_eq!(link("/c/42", None), (None, None));
    assert_eq!(
        link("https://example.com/", Some("_self")),
        (Some("_self".into()), None)
    );
    let blank = (Some("_blank".into()), Some("noopener".into()));
    assert_eq!(link("/c/42", Some("_blank")), blank);
    let app = |t: &str| {
        format!("component A\n  view\n    link href=\"/x\" target=\"{t}\"\n      text \"x\"\n")
    };
    assert!(contract::compile(&app("_blank")).is_ok());
    let e = contract::compile(&app("_top")).unwrap_err().to_string();
    assert!(e.contains("`target` takes"), "{e}");
}

/// @ref LLP 1075.003 §3.3 — every `data-` name this host writes on an
/// HTML element for a prop of its own is a word Contract refuses, so an
/// app's `data-*` never lands on one.
#[test]
fn every_data_name_the_host_writes_is_a_reserved_word() {
    let kinds = [
        NodeType::View,
        NodeType::Text,
        NodeType::Pressable,
        NodeType::TextInput,
        NodeType::Image,
        NodeType::NativeView,
        NodeType::ScrollView,
        NodeType::List,
        NodeType::Video,
        NodeType::WebView,
        NodeType::Canvas,
        NodeType::Svg,
        NodeType::Control,
    ];
    let mut unreserved = std::collections::BTreeSet::new();
    for prop in PropId::ALL {
        if prop == PropId::Dataset || svg_only(prop) {
            continue;
        }
        let value = match prop.kind() {
            PropKind::Str => PropValue::Str("x".into()),
            PropKind::Bool => PropValue::Bool(true),
            PropKind::Int => PropValue::Int(1),
            PropKind::Float => PropValue::Float(1.0),
        };
        for kind in kinds {
            for name in written(kind, prop, value.clone()).keys() {
                if let Some(word) = name.strip_prefix("data-") {
                    if !contract_lower::dataset::reserved(word) {
                        unreserved.insert(format!("`{name}` (from {})", prop.name()));
                    }
                }
            }
        }
    }
    assert!(
        unreserved.is_empty(),
        "not reserved in contract/lower/src/dataset.rs: {unreserved:?}"
    );
}

#[test]
fn a_dataset_is_one_attribute_per_word() {
    crate::link::link(crate::Linked {
        dataset: Some(crate::document::dataset),
        ..crate::link::linked()
    });
    let out = written(
        NodeType::View,
        PropId::Dataset,
        PropValue::Str(r#"{"large-title":"Inbox","trailing":"compose"}"#.into()),
    );
    assert_eq!(
        out.get("data-large-title").map(String::as_str),
        Some("Inbox")
    );
    assert_eq!(
        out.get("data-trailing").map(String::as_str),
        Some("compose")
    );
    assert!(out.get("data-dataset").is_none());
    assert_eq!(
        crate::document::dataset(r#"{"a":"q\"\\\t\u0001","b":""}"#),
        vec![
            ("a".into(), "q\"\\\t\u{1}".into()),
            ("b".into(), String::new())
        ]
    );
}
