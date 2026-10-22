//! `props_of`, `tag_of` and `host_css_of` on single nodes: what the DOM is told.

use exact_kernel::{NodeFacts, NodeType, PropId, PropKind, PropList, PropValue, StyleProps};

fn written(
    node_type: NodeType,
    prop: PropId,
    value: PropValue,
) -> super::SortedMap<String, String> {
    written_props(node_type, &[(prop, value)])
}

fn written_props(
    node_type: NodeType,
    values: &[(PropId, PropValue)],
) -> super::SortedMap<String, String> {
    let style = StyleProps::default();
    let mut props = PropList::new();
    for (prop, value) in values {
        props.set(*prop, value.clone());
    }
    super::props_of(&NodeFacts {
        id: 1,
        node_type,
        style: &style,
        props: &props,
        is_root: false,
        inline_run: false,
        symbol_color: false,
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

/// LLP 1116 D8: a determinate progress is HTML's own `<progress>`, which
/// the browser draws and sizes: its `value` and `max` the attributes, its
/// role implicit, no `type`, and none of the ring's box.
#[test]
fn a_progress_with_a_value_is_htmls_progress_element() {
    let style = StyleProps::default();
    let mut props = PropList::default();
    for (prop, value) in [
        (PropId::Type, "progress"),
        (PropId::AccessibilityRole, "progressbar"),
        (PropId::Value, "0.25"),
        (PropId::Max, "1"),
        (PropId::AccessibilityLabel, "Water"),
    ] {
        props.set(prop, PropValue::Str(value.into()));
    }
    let facts = NodeFacts {
        id: 1,
        node_type: NodeType::Control,
        style: &style,
        props: &props,
        is_root: false,
        inline_run: false,
        symbol_color: false,
    };
    assert_eq!(super::tag_of(&facts, false), "progress");
    assert_eq!(
        super::tag_of(&facts, true),
        "progress",
        "phrasing in a button"
    );
    let out = super::props_of(&facts);
    let get = |k: &str| out.get(k).map(String::as_str);
    assert_eq!(get("value"), Some("0.25"));
    assert_eq!(get("max"), Some("1"));
    assert_eq!(get("aria-label"), Some("Water"));
    for gone in ["role", "type", "aria-busy", "data-exact-progress"] {
        assert_eq!(get(gone), None, "{gone}: {out:?}");
    }
    let css = super::host_css_of(&facts, None, String::new(), "progress");
    assert!(!css.contains("contain"), "the browser's box: {css}");
    // The page draws its bar (LLP 1116.003; tests/document.test.mjs), and
    // gives the element the intrinsic size the kernel measures, `10em` by
    // `1em`, and no automatic minimum (LLP 1115.000 D10.2).
    assert!(include_str!("../index.html").contains(
        "progress { display: block; appearance: none; border: none; background: none; contain: size; contain-intrinsic-size: 10em 1em; min-width: 0; }"
    ));
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
        symbol_color: false,
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
            symbol_color: false,
        };
        let css = super::host_css_of(&facts, Some(&parent), String::new(), "div");
        assert!(!css.contains("justify-self"), "{display:?}: {css}");
    }
    assert!(include_str!("../index.html").contains("[data-exact-progress]::before"));
}

/// LLP 1116.001 D1: a date, time or local date-time control is marked for
/// the page's field look while its appearance is the platform's; the
/// author's `appearance: none`, and every other control, is not.
#[test]
fn a_date_or_time_control_takes_the_field_look_unless_the_author_has_it() {
    let marked = |ty: &str, appearance| {
        let style = StyleProps {
            appearance,
            ..StyleProps::default()
        };
        let mut props = PropList::default();
        props.set(PropId::Type, PropValue::Str(ty.into()));
        super::props_of(&NodeFacts {
            id: 1,
            node_type: NodeType::Control,
            style: &style,
            props: &props,
            is_root: false,
            inline_run: false,
            symbol_color: false,
        })
        .contains_key("data-exact-field")
    };
    use exact_kernel::Appearance::{Auto, None};
    for ty in ["date", "time", "datetime-local"] {
        assert!(marked(ty, Auto), "{ty}");
        assert!(!marked(ty, None), "{ty}, appearance none");
    }
    for ty in ["checkbox", "range", "file", "select"] {
        assert!(!marked(ty, Auto), "{ty}");
    }
    assert!(include_str!("../index.html").contains("input[data-exact-field]"));
}

/// LLP 1116.007 D4: a text field whose appearance is `textfield` is still the
/// platform's field (`data-native`) and is marked for the page to take a
/// number input's spin buttons off; `auto` and `none` are not.
#[test]
fn a_textfield_appearance_is_native_and_marked_for_the_page() {
    let marks = |appearance| {
        let style = StyleProps {
            appearance,
            ..StyleProps::default()
        };
        let mut props = PropList::default();
        props.set(PropId::Type, PropValue::Str("number".into()));
        let out = super::props_of(&NodeFacts {
            id: 1,
            node_type: NodeType::TextInput,
            style: &style,
            props: &props,
            is_root: false,
            inline_run: false,
            symbol_color: false,
        });
        (
            out.contains_key("data-native"),
            out.contains_key("data-exact-textfield"),
        )
    };
    use exact_kernel::Appearance::{Auto, None, Textfield};
    assert_eq!(marks(Auto), (true, false));
    assert_eq!(marks(Textfield), (true, true));
    assert_eq!(marks(None), (false, false));
    assert!(include_str!("../index.html")
        .contains("input[data-exact-textfield]::-webkit-inner-spin-button"));
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
            symbol_color: false,
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
    let time = written_props(
        NodeType::View,
        &[
            (PropId::SemanticTag, PropValue::Str("time".into())),
            (PropId::Text, PropValue::Str("now".into())),
        ],
    );
    assert!(time.contains_key("data-time-shown"));
    for name in time.keys() {
        if let Some(word) = name.strip_prefix("data-") {
            if !contract_lower::dataset::reserved(word) {
                unreserved.insert(format!("`{name}` (from a formatted time)"));
            }
        }
    }
    assert!(
        unreserved.is_empty(),
        "not reserved in contract/lower/src/dataset.rs: {unreserved:?}"
    );
}

#[test]
fn menu_hints_write_no_dom_attributes() {
    for (kind, prop, value) in [
        (NodeType::Pressable, PropId::MenuPlacement, "help"),
        (NodeType::View, PropId::MenuRemove, "close-all"),
    ] {
        let out = written(kind, prop, PropValue::Str(value.into()));
        assert!(out.get("data-menuplacement").is_none());
        assert!(out.get("data-menuremove").is_none());
    }
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

#[test]
fn svg_geometry_attributes_preserve_css_lengths() {
    use exact_kernel::{StyleId, StyleValue};
    let props = PropList::new();
    for (length, expected) in [
        ("0", "0"),
        ("12px", "12"),
        ("25%", "25%"),
        ("50vh", "50vh"),
        ("10vw", "10vw"),
        ("5vmin", "5vmin"),
        ("8vmax", "8vmax"),
    ] {
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::Height, &StyleValue::Text(length.into()))
            .unwrap();
        let facts = NodeFacts {
            id: 1,
            node_type: NodeType::SvgViewport,
            style: &style,
            props: &props,
            is_root: false,
            inline_run: false,
            symbol_color: false,
        };
        let mut out = super::SortedMap::new();
        super::svg_props_of(&|_, _| None, &facts, &mut out);
        assert_eq!(
            out.get("height").map(String::as_str),
            Some(expected),
            "{length}"
        );
    }
}

#[test]
fn complex_svg_geometry_attributes_take_the_initial_value() {
    use exact_kernel::{StyleId, StyleValue};
    exact_kernel::link_segments();
    for (kind, row, name) in [
        (NodeType::SvgViewport, StyleId::Height, "height"),
        (NodeType::SvgRadialGradient, StyleId::Cx, "cx"),
    ] {
        for length in [
            "calc(10vh + 10%)",
            "min(10vw, 50%)",
            "max(5px, 10vw)",
            "clamp(1px, 10vw, 100px)",
            "10svh",
            "10lvw",
            "10dvh",
            "env(viewport-segment-height 1 0)",
            "env(viewport-segment-height 1 0, 25%)",
            "ENV(viewport-segment-height 1 0, 25%)",
            "env(safe-area-inset-top)",
            "calc(env(viewport-segment-height 1 0) + 10%)",
            "min(env(viewport-segment-height 1 0), 50%)",
            "max(5px, env(viewport-segment-height 1 0))",
            "clamp(1px, env(viewport-segment-height 1 0), 100px)",
        ] {
            let mut style = StyleProps::default();
            style
                .set_dynamic(row, &StyleValue::Text(length.into()))
                .unwrap();
            let props = PropList::new();
            let facts = NodeFacts {
                id: 1,
                node_type: kind,
                style: &style,
                props: &props,
                is_root: false,
                inline_run: false,
                symbol_color: false,
            };
            let mut out = super::SortedMap::new();
            super::svg_props_of(&|_, _| None, &facts, &mut out);
            assert!(out.get(name).is_none(), "{kind:?}: {length}");
        }
    }
}

/// A browser has no layout regions (LLP 1123 D10): each is written as the
/// safe area it resolves to on every host that reports none, its offset
/// and any calculation around it kept.
#[test]
fn a_layout_region_is_the_browsers_safe_area() {
    use exact_kernel::{StyleId, StyleValue};
    for (text, css) in [
        (
            "env(-exact-layout-margins-inset-left)",
            "padding-left:env(safe-area-inset-left)",
        ),
        (
            "env(-exact-readable-content-corners-inset-right)",
            "padding-left:env(safe-area-inset-right)",
        ),
        (
            "calc(env(-exact-safe-area-corners-inset-top) + 4px)",
            "padding-left:calc(env(safe-area-inset-top) + 4px)",
        ),
        (
            "max(16px, env(-exact-layout-margins-corners-inset-bottom))",
            "padding-left:max(16px, env(safe-area-inset-bottom))",
        ),
        (
            "max(16px, env(-exact-readable-content-inset-left, 8px))",
            "padding-left:max(16px, env(safe-area-inset-left, 8px))",
        ),
    ] {
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::PaddingLeft, &StyleValue::Text(text.into()))
            .unwrap();
        let (out, _) = crate::css::css_text(&style, &[]);
        assert!(out.contains(css), "{text}: {out}");
        assert!(
            !out.contains("-exact-"),
            "no name a browser does not know: {out}"
        );
    }
}

/// LLP 1116.018 D3: a live region's `aria-atomic` reaches the page.
#[test]
fn a_live_region_says_whether_it_is_announced_whole() {
    let style = StyleProps::default();
    let mut props = PropList::default();
    props.set(PropId::AccessibilityLive, PropValue::Str("polite".into()));
    props.set(PropId::AccessibilityAtomic, PropValue::Bool(true));
    let facts = NodeFacts {
        id: 1,
        node_type: NodeType::Text,
        style: &style,
        props: &props,
        is_root: false,
        inline_run: false,
        symbol_color: false,
    };
    let out = super::props_of(&facts);
    assert_eq!(out.get("aria-live").map(String::as_str), Some("polite"));
    assert_eq!(out.get("aria-atomic").map(String::as_str), Some("true"));
}
