//! What of an app's structure a host projects into its platform's bars
//! (LLP 1115.001 D11; `kernel/src/bars.rs`): the header-shaped route, read
//! from the kernel's tree and from a host's own mirror of it as words, what
//! a route shows of the bar, and a navigation root's tabs.

use exact_kernel::bars::{self, fact, Bar, HeaderShape};
use exact_kernel::{Kernel, NodeType, Op, PropId, PropValue, StyleId, StyleProps, StyleValue};

/// A tree written as (id, type, props, children), applied whole.
struct Node(u32, NodeType, &'static [(PropId, &'static str)], Vec<Node>);

fn ops(node: &Node, out: &mut Vec<Op>) {
    out.push(Op::CreateView {
        id: node.0,
        node_type: node.1,
    });
    for (prop, value) in node.2 {
        let value = match (*prop, value.parse::<i64>()) {
            (PropId::AccessibilityHeadingLevel, Ok(level)) => PropValue::Int(level),
            _ => PropValue::Str((*value).into()),
        };
        out.push(Op::SetProp {
            id: node.0,
            prop: *prop,
            value,
        });
    }
    for child in &node.3 {
        ops(child, out);
    }
    out.push(Op::SetChildren {
        id: node.0,
        children: node.3.iter().map(|c| c.0).collect(),
    });
}

fn kernel(root: Node) -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let mut out = Vec::new();
    ops(&root, &mut out);
    out.push(Op::AttachRoot { id: root.0 });
    kernel.apply(0, 1, &out).unwrap();
    kernel
}

const HEADER: &[(PropId, &str)] = &[(PropId::SemanticTag, "header")];
const H1: &[(PropId, &str)] = &[
    (PropId::AccessibilityRole, "heading"),
    (PropId::AccessibilityHeadingLevel, "1"),
];
const HEADING: &[(PropId, &str)] = &[(PropId::AccessibilityRole, "heading")];
const BACK: &[(PropId, &str)] = &[(PropId::Id, "back")];
const SEARCH: &[(PropId, &str)] = &[(PropId::Type, "search")];
const TABLIST: &[(PropId, &str)] = &[(PropId::AccessibilityRole, "tablist")];

fn view(id: u32, children: Vec<Node>) -> Node {
    Node(id, NodeType::View, &[], children)
}
fn button(id: u32, props: &'static [(PropId, &'static str)]) -> Node {
    Node(id, NodeType::Pressable, props, vec![])
}
fn text(id: u32, props: &'static [(PropId, &'static str)]) -> Node {
    Node(id, NodeType::Text, props, vec![])
}

/// Shelf's library: a level-1 heading, a button after it, a search field.
/// Its add form: the Back control, a heading, a button.
fn shelf() -> Kernel {
    kernel(view(
        1,
        vec![
            view(
                10,
                vec![
                    Node(
                        11,
                        NodeType::View,
                        HEADER,
                        vec![
                            text(12, H1),
                            button(13, &[]),
                            Node(14, NodeType::TextInput, SEARCH, vec![]),
                        ],
                    ),
                    view(15, vec![]),
                ],
            ),
            view(
                20,
                vec![Node(
                    21,
                    NodeType::View,
                    HEADER,
                    vec![button(22, BACK), text(23, HEADING), button(24, &[])],
                )],
            ),
        ],
    ))
}

fn pressed(id: u32) -> bool {
    matches!(id, 13 | 22 | 24 | 32 | 41 | 42)
}

#[test]
fn a_header_is_a_bar_with_its_heading_its_buttons_and_its_search_field() {
    let kernel = shelf();
    let shape = kernel
        .header_shape(10, Some("back"), false, &pressed)
        .unwrap();
    assert_eq!(
        shape,
        HeaderShape {
            header: 11,
            heading: 12,
            level: 1,
            leading: vec![],
            trailing: vec![13],
            items: vec![13],
            search: Some(14),
            segments: None,
            tap: None,
        }
    );
}

#[test]
fn the_back_control_is_the_platforms_own_back_on_a_pushed_route_and_an_item_elsewhere() {
    let kernel = shelf();
    // Pushed: the platform's Back stands for it, and it is no item.
    let pushed = kernel
        .header_shape(20, Some("back"), true, &pressed)
        .unwrap();
    assert_eq!(
        (pushed.leading, pushed.trailing, pushed.items),
        (vec![], vec![24], vec![22, 24])
    );
    assert_eq!(pushed.level, 2, "a heading that says no level is ARIA's 2");
    // The root of a presented stack has no such Back: a leading item.
    let presented = kernel
        .header_shape(20, Some("back"), false, &pressed)
        .unwrap();
    assert_eq!(
        (presented.leading, presented.trailing),
        (vec![22], vec![24])
    );
    // No root names a Back control: every button is an item.
    let unnamed = kernel.header_shape(20, None, true, &pressed).unwrap();
    assert_eq!(unnamed.leading, vec![22]);
}

#[test]
fn what_is_not_header_shaped_is_no_bar() {
    // No header first; a header with no heading; one with two; a level with
    // no heading role (LLP 1115.000 D12.5); a text in a button is no title.
    let level: &[(PropId, &str)] = &[(PropId::AccessibilityHeadingLevel, "1")];
    for (name, route) in [
        (
            "the header is not first",
            view(
                10,
                vec![
                    view(15, vec![]),
                    Node(11, NodeType::View, HEADER, vec![text(12, H1)]),
                ],
            ),
        ),
        (
            "no heading",
            view(
                10,
                vec![Node(11, NodeType::View, HEADER, vec![button(13, &[])])],
            ),
        ),
        (
            "two headings",
            view(
                10,
                vec![Node(
                    11,
                    NodeType::View,
                    HEADER,
                    vec![text(12, H1), text(16, HEADING)],
                )],
            ),
        ),
        (
            "a level alone",
            view(
                10,
                vec![Node(11, NodeType::View, HEADER, vec![text(12, level)])],
            ),
        ),
        (
            "a first child that is no header",
            view(10, vec![view(11, vec![text(12, H1)])]),
        ),
    ] {
        let kernel = kernel(view(1, vec![route]));
        assert_eq!(
            kernel.header_shape(10, None, true, &pressed),
            None,
            "{name}"
        );
    }
}

#[test]
fn a_pressable_around_the_heading_is_the_title_and_a_tablist_its_segments() {
    let kernel = kernel(view(
        10,
        vec![Node(
            11,
            NodeType::View,
            HEADER,
            vec![
                button(41, &[]),
                // Pressable, and it holds the heading: the title's group.
                Node(
                    42,
                    NodeType::Pressable,
                    &[],
                    vec![view(43, vec![text(12, HEADING)]), text(44, &[])],
                ),
                Node(30, NodeType::View, TABLIST, vec![button(32, &[])]),
                // A button that opens a menu is an item with no `press`.
                button(50, &[(PropId::Popovertarget, "more")]),
                // One that only hides a popover is not.
                button(
                    51,
                    &[
                        (PropId::Popovertarget, "more"),
                        (PropId::Popovertargetaction, "hide"),
                    ],
                ),
            ],
        )],
    ));
    let shape = kernel.header_shape(10, None, true, &pressed).unwrap();
    assert_eq!(shape.tap, Some(42));
    assert_eq!(shape.heading, 12);
    assert_eq!(shape.segments, Some(30));
    assert_eq!(shape.leading, vec![41]);
    assert_eq!(
        shape.trailing,
        vec![50],
        "the tabs are segments, never items"
    );
    assert_eq!(shape.items, vec![41, 50]);
}

#[test]
fn a_route_that_says_no_bar_or_hides_its_header_has_none() {
    let mut kernel = shelf();
    assert!(kernel.header_shape(10, None, false, &pressed).is_some());
    let none = PropValue::Str("none".into());
    kernel
        .apply(
            0,
            2,
            &[Op::SetProp {
                id: 10,
                prop: PropId::NavigationBar,
                value: none,
            }],
        )
        .unwrap();
    assert_eq!(kernel.header_shape(10, None, false, &pressed), None);
    let mut hidden = StyleProps::default();
    hidden
        .set_dynamic(StyleId::Display, &StyleValue::Text("none".into()))
        .unwrap();
    kernel
        .apply(
            0,
            3,
            &[Op::SetStyle {
                id: 21,
                patch: Box::new(hidden),
            }],
        )
        .unwrap();
    assert_eq!(
        kernel.header_shape(20, None, false, &pressed),
        None,
        "a header the web does not show is no bar's"
    );
}

/// A header whose author wrote `appearance: none` keeps its authored bar on
/// every host (LLP 1075.003 §9.10, #393); one the compiler only implied it
/// for is still the bar's.
#[test]
fn a_header_that_writes_appearance_none_is_no_bar() {
    let mut kernel = shelf();
    let id = 11; // route 10's header
    let mut implied = StyleProps::default();
    implied
        .set_dynamic(StyleId::Appearance, &StyleValue::Text("none".into()))
        .unwrap();
    let mut written = implied.clone();
    written.written = written.mask;
    kernel
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id,
                patch: Box::new(implied),
            }],
        )
        .unwrap();
    assert!(
        kernel.header_shape(10, None, false, &pressed).is_some(),
        "implied is not written"
    );
    kernel
        .apply(
            0,
            3,
            &[Op::SetStyle {
                id,
                patch: Box::new(written),
            }],
        )
        .unwrap();
    assert_eq!(kernel.header_shape(10, None, false, &pressed), None);
}

/// The same route as a host's mirror hands it: the route, then its first
/// child's subtree in preorder, two words a node.
#[test]
fn a_hosts_own_mirror_of_the_tree_reads_the_same_rule() {
    let h1 = fact::HEADING | 1 << fact::LEVEL;
    // route > header > [back button, heading, button, search]
    let words = [
        0,
        1,
        fact::HEADER,
        4,
        fact::PRESS | fact::BACK,
        0,
        h1,
        0,
        fact::PRESS,
        1,
        0,
        0,
        fact::SEARCH,
        0,
    ];
    // header 1, heading 3, level 1, no tap, search 6 (+1), no tablist;
    // one leading, one trailing, two items.
    assert_eq!(
        bars::header_shape_words(&words, 0),
        Some(vec![1, 3, 1, 0, 7, 0, 1, 1, 2, 2, 4, 2, 4])
    );
    // Pushed: the Back control is the platform's, in neither list.
    assert_eq!(
        bars::header_shape_words(&words, bars::BACK_IS_PLATFORMS),
        Some(vec![1, 3, 1, 0, 7, 0, 0, 1, 2, 4, 2, 4])
    );
    // The projection: a hidden header, or a route that says no bar, has none,
    // and the shape alone is still read.
    let mut hidden = words;
    hidden[2] |= fact::HIDDEN;
    assert_eq!(bars::header_shape_words(&hidden, bars::PROJECTED), None);
    assert!(bars::header_shape_words(&hidden, 0).is_some());
    let mut authored = words;
    authored[2] |= fact::AUTHORED;
    assert_eq!(bars::header_shape_words(&authored, bars::PROJECTED), None);
    assert!(bars::header_shape_words(&authored, 0).is_some());
    let mut refused = words;
    refused[0] |= fact::NO_BAR;
    assert_eq!(bars::header_shape_words(&refused, bars::PROJECTED), None);
    assert!(bars::header_shape_words(&words, bars::PROJECTED).is_some());
    // Words that are not one tree answer nothing.
    assert_eq!(bars::header_shape_words(&[], 0), None);
    assert_eq!(bars::header_shape_words(&[0, 1], 0), None, "a child owed");
    assert_eq!(
        bars::header_shape_words(&[0, 0, 0, 0], 0),
        None,
        "two roots"
    );
    assert_eq!(bars::header_shape_words(&[0], 0), None);
}

#[test]
fn what_a_route_shows_of_the_bar() {
    assert_eq!(
        Bar::of(true, true, true, true),
        Bar::Hidden,
        "the author says none"
    );
    assert_eq!(Bar::of(false, true, false, true), Bar::Header);
    assert_eq!(Bar::of(false, false, true, true), Bar::Back);
    assert_eq!(
        Bar::of(false, false, true, false),
        Bar::Hidden,
        "a platform whose Back is not a bar's button"
    );
    assert_eq!(Bar::of(false, false, false, true), Bar::Hidden);
}

#[test]
fn a_roots_tabs_are_the_panels_its_own_tablist_names() {
    let tab = |id: u32, controls: &'static [(PropId, &'static str)]| {
        Node(id, NodeType::Pressable, controls, vec![])
    };
    const A: &[(PropId, &str)] = &[
        (PropId::AccessibilityRole, "tab"),
        (PropId::AccessibilityControls, "panel-a"),
    ];
    const B: &[(PropId, &str)] = &[
        (PropId::AccessibilityRole, "tab"),
        (PropId::AccessibilityControls, "panel-b"),
    ];
    const NOWHERE: &[(PropId, &str)] = &[
        (PropId::AccessibilityRole, "tab"),
        (PropId::AccessibilityControls, "panel-z"),
    ];
    const PANEL_A: &[(PropId, &str)] = &[
        (PropId::AccessibilityRole, "tabpanel"),
        (PropId::Id, "panel-a"),
    ];
    const PANEL_B: &[(PropId, &str)] = &[
        (PropId::AccessibilityRole, "tabpanel"),
        (PropId::Id, "panel-b"),
    ];
    const ROUTE: &[(PropId, &str)] = &[(PropId::NavigationKey, "1")];
    let kernel = kernel(view(
        1,
        vec![
            view(
                2,
                vec![
                    Node(
                        3,
                        NodeType::View,
                        PANEL_A,
                        // A route's own tablist (a segmented filter) is the route's.
                        vec![Node(
                            5,
                            NodeType::View,
                            ROUTE,
                            vec![Node(6, NodeType::View, TABLIST, vec![tab(7, B)])],
                        )],
                    ),
                    Node(4, NodeType::View, PANEL_B, vec![]),
                ],
            ),
            // The tabs in the order written; one names no panel.
            Node(
                8,
                NodeType::View,
                TABLIST,
                vec![tab(9, B), tab(10, NOWHERE), tab(11, A)],
            ),
        ],
    ));
    let tabs = kernel.root_tabs(1).unwrap();
    assert_eq!(tabs.tablist, 8);
    assert_eq!(tabs.tabs, vec![9, 11]);
    assert_eq!(tabs.panels, vec![4, 3]);
    // A root with no tablist of its own is one stack.
    assert_eq!(kernel.root_tabs(5), None);
    assert_eq!(kernel.root_tabs(99), None);
}
