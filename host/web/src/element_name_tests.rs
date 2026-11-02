//! `host_css_of`'s control sizing against the page's reset, and prop names.

use super::host_css_of;
use exact_kernel::{NodeFacts, NodeType, PropId, PropList, PropValue, StyleProps};

#[test]
fn prop_names_are_ascii() {
    for prop in exact_kernel::PropId::ALL {
        assert!(prop.name().is_ascii(), "{}", prop.name());
    }
}

#[test]
fn page_reset_and_lowered_css_leave_native_control_sizing_to_chrome() {
    let page = include_str!("../index.html");
    let stylesheet = page
        .split_once("<style>")
        .and_then(|(_, rest)| rest.split_once("</style>"))
        .map(|(css, _)| css)
        .expect("page stylesheet");
    assert!(page.contains("select { display: block; }"));
    assert!(page
        .contains("input[type=\"checkbox\"], input[type=\"radio\"] { box-sizing: border-box; }"));
    let mut file_appearance_restored = false;
    for rule in stylesheet.split('}') {
        let Some((selectors, declarations)) = rule.rsplit_once('{') else {
            continue;
        };
        if selectors.contains("input[type=\"file\"]")
            && declarations
                .split(';')
                .any(|declaration| declaration.trim() == "appearance: auto")
        {
            file_appearance_restored = true;
        }
        if selectors.split(',').any(|selector| {
            let selector = selector.trim();
            if selector.contains("::") || selector.contains(" > ") {
                return false;
            }
            selector.contains("input")
                || selector.contains("textarea")
                || selector.contains("select")
                || selector.contains("button")
        }) {
            assert!(
                !declarations
                    .split(';')
                    .any(|declaration| declaration.trim_start().starts_with("width:")),
                "control reset must not author a width: {selectors} {{{declarations}}}"
            );
        }
    }
    assert!(file_appearance_restored);
    for (tag, ty) in [("input", "checkbox"), ("select", "select")] {
        let style = StyleProps::default();
        let mut props = PropList::default();
        props.set(PropId::Type, PropValue::Str(ty.into()));
        let facts = NodeFacts {
            id: 1,
            node_type: NodeType::Control,
            style: &style,
            props: &props,
            is_root: false,
            inline_run: false,
            symbol_color: false,
        };
        let css = host_css_of(&facts, None, String::new(), tag);
        assert!(!css.contains("width:"), "{tag}: {css}");
        assert!(!css.contains("box-sizing:"), "{tag}: {css}");
    }
}
