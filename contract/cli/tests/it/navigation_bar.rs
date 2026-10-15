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

#[test]
fn top_scroll_inset_is_an_explicit_route_policy() {
    for value in ["none", "auto"] {
        let plan = contract::compile(&stack(
            "",
            &format!("navigationScrollInsetTop=\"{value}\""),
            "",
        ))
        .unwrap();
        assert!(plan.nodes.iter().any(|n| n
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .any(|b| b.id == PropId::NavigationScrollInsetTop as u16)));
    }
    assert_eq!(
        contract::compile(&stack("", "navigationScrollInsetTop=\"hidden\"", ""))
            .unwrap_err()
            .id,
        "lower-attr-value"
    );
    assert_eq!(
        contract::compile(&stack("navigationScrollInsetTop=\"none\"", "", ""))
            .unwrap_err()
            .id,
        "lower-route-bar"
    );
}
