//! @ref LLP 1047 D2 — what a plan uses is read from its rows, never declared.

use exact_runner::{uses, Capability, Uses};

fn used(source: &str) -> Uses {
    uses(&contract::compile(source).unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn markdown_is_a_markup_prop_that_can_be_markdown() {
    let plain = used("component A\n  view\n    text \"**b**\"\n");
    assert_eq!(plain, Uses::NONE);
    let constant = used("component A\n  view\n    text \"**b**\" markup=\"markdown\"\n");
    assert!(constant.has(Capability::Markdown));
    assert_eq!(constant.to_string(), "markdown");
    // Another constant selects nothing; a computed one might be Markdown.
    let other = used("component A\n  view\n    text \"**b**\" markup=\"plain\"\n");
    assert_eq!(other, Uses::NONE);
    let computed = used(
        "component A\n  state rich = true\n  view\n    text \"**b**\" markup=(rich ? \"markdown\" : \"plain\")\n",
    );
    assert!(computed.has(Capability::Markdown));
}

#[test]
fn motion_is_a_spring_or_a_gesture_that_holds_a_value() {
    let css =
        used("component A\n  view\n    text \"a\" opacity=0.5 transition=\"opacity 200ms ease\"\n");
    assert_eq!(css, Uses::NONE, "CSS plays an easing transition");
    let spring = used(
        "component A\n  view\n    text \"a\" scale=1.5 transition=\"scale spring(180, 12, 1)\"\n",
    );
    assert!(spring.has(Capability::Motion));
    let swipe = used(
        "component A\n  state n = 0\n  action swipe writes n\n    n = n + 1\n  view\n    text \"a\" swiperight=swipe\n",
    );
    assert!(swipe.has(Capability::Motion));
    assert!(
        !swipe.has(Capability::Drag),
        "a swipe holds a value but tracks no handle"
    );
    let drag = used(
        "component A\n  view\n    column id=\"sheet\" height=100\n      column heightDragFor=\"sheet\"\n",
    );
    assert!(drag.has(Capability::Motion) && drag.has(Capability::Drag));
    assert_eq!(
        spring.with(Capability::Markdown).to_string(),
        "markdown, motion"
    );
}

#[test]
fn collections_are_lists_the_host_windows() {
    let list = |attrs: &str| {
        used(&format!(
            "component A\n  resource rows = rows() as shape list<string>\n  view\n    list {attrs}\n      each x in rows key=x\n        column\n          text x\n"
        ))
    };
    assert_eq!(list("height=100"), Uses::NONE, "a plain list is the core's");
    assert!(list("virtualized=true height=100").has(Capability::Collections));
    assert_eq!(list("virtualized=false height=100"), Uses::NONE);
    assert!(list("item-height=20 height=100").has(Capability::Collections));
}

#[test]
fn a_set_names_what_it_holds_beyond_another() {
    let markdown = Uses::NONE.with(Capability::Markdown);
    assert!(markdown.beyond(markdown).is_empty());
    assert_eq!(markdown.beyond(Uses::NONE), markdown);
    assert!(Uses::NONE.beyond(markdown).is_empty());
    assert_eq!(Uses::NONE.to_string(), "");
}
