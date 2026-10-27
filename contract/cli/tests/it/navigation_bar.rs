//! LLP 1115.000 D5.2: no navigation bar is something a route says.
//! `navigationBar` is a route's prop, `"none"` or `"auto"`; a literal with
//! any other word is refused, and so is the attribute anywhere but on a
//! route, where no host would read it.

use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A stack of two routes; `root`, `route` and `inner` are attributes of the
/// navigation root, of each route and of a box inside each.
fn stack(root: &str, route: &str, inner: &str) -> String {
    format!(
        "routes nav\n  home \"/\"\n    viewer \"/viewer\"\ncomponent App\n  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\" {root}\n      for e in stack(nav) key=e.id\n        column navigationKey=`${{e.id}}` testId=`route-${{e.name}}` position=\"absolute\" inset=0 {route}\n          column {inner}\n            text e.name\n"
    )
}

#[test]
fn a_route_says_it_has_no_bar_and_the_word_reaches_the_host() {
    let src = stack(
        "",
        "navigationBar=(e.name == \"viewer\" ? \"none\" : \"auto\")",
        "",
    );
    let plan = contract::bake(contract::compile(&src).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/viewer",
    )
    .unwrap();
    let bar = |id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .str(PropId::NavigationBar)
            .map(str::to_string)
    };
    assert_eq!(bar("route-home").as_deref(), Some("auto"));
    assert_eq!(bar("route-viewer").as_deref(), Some("none"));
    // A literal on every route, and none at all (the default).
    for route in ["navigationBar=\"none\"", "navigationBar=\"auto\"", ""] {
        contract::compile(&stack("", route, "")).unwrap_or_else(|e| panic!("{route}: {e}"));
    }
}

#[test]
fn a_literal_outside_none_and_auto_is_refused() {
    for value in ["hidden", "false", "", "None", "none auto"] {
        let error =
            contract::compile(&stack("", &format!("navigationBar=\"{value}\""), "")).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{value:?}: {error}");
        assert!(
            error.message.contains("`navigationBar` takes \"none\"")
                && error.message.contains("\"auto\""),
            "{value:?}: {error}"
        );
    }
}

#[test]
fn off_a_route_the_attribute_is_refused_naming_where_it_belongs() {
    // On the navigation root (a guess at "no bars in this app") and on a box
    // inside a screen, no host reads it: the screen would keep its bar.
    for (root, inner, on) in [
        ("navigationBar=\"none\"", "", "the navigation root"),
        (
            "",
            "navigationBar=\"none\"",
            "an element with no `navigationKey`",
        ),
    ] {
        let error = contract::compile(&stack(root, "", inner)).unwrap_err();
        assert_eq!(error.id, "lower-route-bar", "{error}");
        assert!(
            error.message.contains(on) && error.message.contains("belongs to a route"),
            "{error}"
        );
        assert_eq!(
            error.span.line,
            if inner.is_empty() { 6 } else { 9 },
            "{error}"
        );
    }
    // With no navigation root at all it is as misplaced.
    let error = contract::compile("component App\n  view\n    column navigationBar=\"none\"\n")
        .unwrap_err();
    assert_eq!(error.id, "lower-route-bar", "{error}");
}

/// A button that presses nothing and opens no popover is no item of the iOS
/// bar and is hidden with the header (euxy port 14's "● Clock" readout):
/// the warning says so and names a readout's `role="status"`.
#[test]
fn a_header_button_that_presses_nothing_is_named() {
    let header = "header\n            text \"Log\" role=\"heading\" aria-level=2\n            button aria-label=\"Clock\"\n              text \"● Clock\"";
    let src = stack("", "", "").replace("column \n", &format!("{header}\n          column\n"));
    let (result, warnings) = contract::compile_warned(&src);
    result.unwrap();
    let ids: Vec<_> = warnings
        .iter()
        .map(|w| (w.id.as_str(), w.span.line))
        .collect();
    assert_eq!(ids, [("lower-header-unplaced", 11)], "{src}");
    let says = &warnings[0].message;
    assert!(
        says.starts_with(
            "a `button` that presses nothing in this route's `header` is not shown on iOS"
        ),
        "{says}"
    );
    assert!(says.contains("give it `role=\"status\"`"), "{says}");
    // A disabled one is UIKit's disabled bar item, shown dimmed: no warning.
    let disabled = src.replace("button aria-label", "button disabled=true aria-label");
    let (result, warnings) = contract::compile_warned(&disabled);
    result.unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    // One that says it is not disabled is still no item.
    let enabled = src.replace("button aria-label", "button disabled=false aria-label");
    let (result, warnings) = contract::compile_warned(&enabled);
    result.unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn safe_area_opt_out_is_local_to_a_routes_authored_root() {
    for value in [
        "auto",
        "none",
        "top",
        "bottom",
        "top bottom",
        "left right",
        "top right bottom left",
    ] {
        let source = stack("", &format!("-exact-safe-area=\"{value}\""), "");
        let plan = contract::bake(contract::compile(&source).unwrap(), NoData).unwrap();
        let runner = Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/viewer",
        )
        .unwrap();
        let kernel = runner.kernel();
        let ids = kernel.find_by_test_id("route-viewer");
        assert_eq!(ids.len(), 1);
        for key in ids {
            assert_eq!(
                kernel.node_by_key(key).unwrap().props.str(PropId::SafeArea),
                Some(value)
            );
        }
    }
    for value in ["", "Top", "top top", "auto top", "none bottom", "vertical"] {
        let error = contract::compile(&stack("", &format!("-exact-safe-area=\"{value}\""), ""))
            .unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{value}: {error}");
    }
    // Every literal a value can be is checked: a conditional's branches and a template with no `${}`.
    for expression in [
        r#"e.name == "viewer" ? "top" : "none bottom""#,
        r#"e.name == "viewer" ? "auto top" : "bottom""#,
        r#"e.name == "viewer" ? "top" : (e.name == "other" ? "Top" : "auto")"#,
        "`tpo`",
        r#"e.name == "viewer" ? `top` : `left left`"#,
    ] {
        let source = stack("", &format!("-exact-safe-area=({expression})"), "");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{expression}: {error}");
    }
    // A template that computes its value is the host's to read.
    contract::compile(&stack("", "-exact-safe-area=`${e.name}`", "")).unwrap();
    for (root, inner) in [
        ("-exact-safe-area=\"none\"", ""),
        ("", "-exact-safe-area=\"top\""),
    ] {
        let error = contract::compile(&stack(root, "", inner)).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{error}");
        assert!(error.message.contains("owner's authored root"), "{error}");
    }
    let error = contract::compile("component App\n  view\n    column -exact-safe-area=\"none\"\n")
        .unwrap_err();
    assert_eq!(error.id, "lower-attr-value", "{error}");
    // State bindings update the same non-inherited prop; ordinary descendants get none.
    let source = stack(
        "",
        "-exact-safe-area=(e.name == \"viewer\" ? \"top bottom\" : \"auto\")",
        "",
    );
    let plan = contract::bake(contract::compile(&source).unwrap(), NoData).unwrap();
    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/viewer",
    )
    .unwrap();
    let kernel = runner.kernel();
    let viewer = kernel
        .node_by_key(kernel.find_by_test_id("route-viewer")[0])
        .unwrap();
    assert_eq!(viewer.props.str(PropId::SafeArea), Some("top bottom"));
    for child in viewer.children() {
        assert_eq!(
            kernel.node(child).unwrap().props.str(PropId::SafeArea),
            None
        );
    }
}
