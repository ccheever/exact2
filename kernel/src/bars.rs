//! What of an app's structure a host projects into its platform's bars
//! (LLP 1115.001 D11): a route's `header` as its navigation bar or top app
//! bar, and a navigation root's `tablist` as its tab bar or navigation bar.
//! One rule for every host that has such bars, beside the grouped list's
//! (`grouped.rs`): what each bar looks like is the platform's (UIKit's on
//! iOS, Material's on Android), and what counts as one is decided here.
//!
//! **A header-shaped route** (LLP 1035.001 D9) is one whose first child is a
//! `header` holding exactly one heading. The heading is the bar's title, at
//! its level; the header's buttons are the bar's items, those before the
//! heading leading and those after it trailing; its `input type="search"`
//! is the bar's search field and its `tablist` the title's segments. A
//! pressable around the heading is the title itself, tapped. The stack's
//! Back control is the platform's own Back where the platform has one for
//! that route, and an item like the others where it has not.
//!
//! **Two trees.** The rule is written once, over [`Tree`]: a node's facts
//! ([`fact`]) and its children. A host that reads the kernel's tree calls
//! [`Kernel::header_shape`]. A host that keeps a mirror of the tree (the
//! Apple hosts' views, which UIKit also rearranges) hands the same facts
//! about its own nodes as words ([`header_shape_words`]), so the two cannot
//! disagree about the rule, only about a fact.
//!
//! **The root's tabs** (LLP 1075.003 §3.7) are the tabpanels the root's own
//! tablist's tabs name with `aria-controls`, in tab order
//! ([`Kernel::root_tabs`]). A tablist or a panel inside a route is that
//! route's, never the root's.

use crate::control::ControlKind;
use crate::generated::{Appearance, Display, NodeType, PropId, StyleId};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};
use crate::props::PropValue;

// Paint predicates are broader than the projection rules below.
impl NodeRef<'_> {
    /// The neutral sheet removes an ordinary bar button's edge anywhere
    /// below a direct route header, including nested authored containers.
    pub fn inside_neutral_header(&self) -> bool {
        self.arena.inside_neutral_header(self.slot)
    }

    /// A direct controlled tab of the neutral sheet, independently of
    /// whether a navigation host projects its tablist or resolves its target.
    pub fn is_neutral_tab(&self) -> bool {
        self.arena.is_neutral_tab(self.slot)
    }
}

/// What the rule asks of a node: bits, with a heading's level above them
/// ([`fact::LEVEL`]).
pub mod fact {
    /// A text whose role is `heading`.
    pub const HEADING: u32 = 1;
    /// An `input type="search"`.
    pub const SEARCH: u32 = 1 << 1;
    /// Its role is `tablist`.
    pub const TABLIST: u32 = 1 << 2;
    /// It hears `press`.
    pub const PRESS: u32 = 1 << 3;
    /// A button that opens a popover: it names one with `popovertarget`,
    /// and not to hide it.
    pub const MENU: u32 = 1 << 4;
    /// Its `id` is the one the navigation root names as the stack's Back
    /// control (`navigationBack`).
    pub const BACK: u32 = 1 << 5;
    /// A `header`.
    pub const HEADER: u32 = 1 << 6;
    /// `display: none`.
    pub const HIDDEN: u32 = 1 << 7;
    /// A route that says it has no bar (`navigationBar="none"`).
    pub const NO_BAR: u32 = 1 << 8;
    /// The author wrote `appearance: none` on it (LLP 1075.003 §9.10): a
    /// header that says so keeps its authored bar. Only a written one: a
    /// row the compiler implied is no opt-out.
    pub const AUTHORED: u32 = 1 << 9;
    /// A heading's `aria-level`, 1 to 6, shifted this far; 0 where it says
    /// none (ARIA's default is then 2).
    pub const LEVEL: u32 = 16;
}

/// A tree the rule can read.
pub trait Tree {
    /// One of its nodes.
    type Node: Copy + PartialEq;
    /// The node's facts ([`fact`]).
    fn facts(&self, node: Self::Node) -> u32;
    /// Its children, in order.
    fn children(&self, node: Self::Node) -> Vec<Self::Node>;
}

/// A header-shaped route's header, as a bar shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderShape<N> {
    /// The `header`.
    pub header: N,
    /// Its one heading: the bar's title.
    pub heading: N,
    /// The heading's level: 1 is the screen's own title (UIKit's large
    /// title), any other an inline one.
    pub level: u32,
    /// The buttons before the heading, in order.
    pub leading: Vec<N>,
    /// The buttons after it, in order.
    pub trailing: Vec<N>,
    /// Every button of the header, the stack's Back control among them
    /// (which is in neither list where the platform's own Back stands for
    /// it).
    pub items: Vec<N>,
    /// Its `input type="search"`, the first.
    pub search: Option<N>,
    /// Its `tablist`, the first: the title's segments.
    pub segments: Option<N>,
    /// The pressable around the heading, which a tap on the title presses.
    pub tap: Option<N>,
}

/// What a route shows of the platform's bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    /// The route's `header`, projected: its heading the title, its buttons
    /// the items, and the platform's Back when it is pushed.
    Header = 0,
    /// The platform's Back alone, with no title: a pushed route with no
    /// bar of its own.
    Back = 1,
    /// No bar.
    Hidden = 2,
}

impl Bar {
    /// LLP 1115.000 D5.2, D5.3: the bar is decided route by route. A route
    /// that says `navigationBar="none"` (`asked_none`) has none: no bar is
    /// something the author says, never inferred. A route whose `header` is
    /// the bar's (`header`) shows that. Any other pushed route shows the
    /// platform's Back alone, where the platform's bar has a Back button
    /// (`back_button`: tvOS's has none, its Back is the remote's Menu, and
    /// Android's Back is the system's). The first route of a stack with no
    /// such header has nothing to show.
    pub fn of(asked_none: bool, header: bool, pushed: bool, back_button: bool) -> Bar {
        if asked_none {
            return Bar::Hidden;
        }
        if header {
            return Bar::Header;
        }
        if pushed && back_button {
            Bar::Back
        } else {
            Bar::Hidden
        }
    }
}

/// Whether any node under `node` is a heading.
fn holds_heading<T: Tree>(tree: &T, node: T::Node) -> bool {
    tree.children(node)
        .into_iter()
        .any(|child| tree.facts(child) & fact::HEADING != 0 || holds_heading(tree, child))
}

struct Walk<N> {
    headings: Vec<N>,
    leading: Vec<N>,
    trailing: Vec<N>,
    items: Vec<N>,
    search: Option<N>,
    segments: Option<N>,
    tap: Option<N>,
}

impl<N: Copy + PartialEq> Walk<N> {
    fn walk<T: Tree<Node = N>>(&mut self, tree: &T, node: N, back_is_platforms: bool) {
        for child in tree.children(node) {
            let facts = tree.facts(child);
            if facts & fact::HEADING != 0 {
                self.headings.push(child);
            } else if facts & fact::SEARCH != 0 {
                self.search.get_or_insert(child);
            } else if facts & fact::TABLIST != 0 {
                // Its tabs press, but they are the title's segments, never
                // bar items.
                self.segments.get_or_insert(child);
            } else if facts & (fact::PRESS | fact::MENU) != 0 {
                // A pressable heading's group is the title, tapped.
                if self.tap.is_none() && facts & fact::PRESS != 0 && holds_heading(tree, child) {
                    self.tap = Some(child);
                    self.walk(tree, child, back_is_platforms);
                    continue;
                }
                self.items.push(child);
                if back_is_platforms && facts & fact::BACK != 0 {
                    continue;
                }
                match self.headings.is_empty() {
                    true => self.leading.push(child),
                    false => self.trailing.push(child),
                }
            } else {
                self.walk(tree, child, back_is_platforms);
            }
        }
    }
}

/// The header of `route` as a bar shows it, or `None` for a route that is
/// not header-shaped. `back_is_platforms`: the platform's own Back stands
/// for the stack's Back control on this route (a pushed one), so that
/// control is no item; the root of a presented stack has no such Back, and
/// there it is an item like the others.
pub fn header_shape<T: Tree>(
    tree: &T,
    route: T::Node,
    back_is_platforms: bool,
) -> Option<HeaderShape<T::Node>> {
    let header = tree.children(route).into_iter().next()?;
    if tree.facts(header) & fact::HEADER == 0 {
        return None;
    }
    let mut walk = Walk {
        headings: Vec::new(),
        leading: Vec::new(),
        trailing: Vec::new(),
        items: Vec::new(),
        search: None,
        segments: None,
        tap: None,
    };
    walk.walk(tree, header, back_is_platforms);
    let [heading] = walk.headings[..] else {
        return None;
    };
    let level = match tree.facts(heading) >> fact::LEVEL {
        level @ 1..=6 => level,
        _ => 2,
    };
    Some(HeaderShape {
        header,
        heading,
        level,
        leading: walk.leading,
        trailing: walk.trailing,
        items: walk.items,
        search: walk.search,
        segments: walk.segments,
        tap: walk.tap,
    })
}

/// The header a route's bar is projected from in the app: its shape
/// ([`header_shape`]) unless the route says it has no bar
/// (`navigationBar="none"`, LLP 1115.000 D5.2), where its header paints in
/// the page as written, or its header is hidden as authored
/// (`display: none`): a header the web does not show is no bar's (LLP
/// 1075.003 §9.10). A header whose author wrote `appearance: none` keeps
/// its authored bar on every host (§9.10, Charlie's ruling for #393).
pub fn header_bar<T: Tree>(
    tree: &T,
    route: T::Node,
    back_is_platforms: bool,
) -> Option<HeaderShape<T::Node>> {
    if tree.facts(route) & fact::NO_BAR != 0 {
        return None;
    }
    header_shape(tree, route, back_is_platforms)
        .filter(|shape| tree.facts(shape.header) & (fact::HIDDEN | fact::AUTHORED) == 0)
}

/// A host's own mirror of a route, as words: the route, then the subtree of
/// its first child, in preorder, two words a node (its facts, the count of
/// its children). The route's other children are left out: the rule reads
/// none of them.
struct Words {
    facts: Vec<u32>,
    children: Vec<Vec<u32>>,
}

impl Words {
    fn read(words: &[u32]) -> Option<Words> {
        if words.is_empty() || !words.len().is_multiple_of(2) {
            return None;
        }
        let count = words.len() / 2;
        let mut out = Words {
            facts: Vec::with_capacity(count),
            children: vec![Vec::new(); count],
        };
        // The nodes still owed children, innermost last.
        let mut open: Vec<(u32, u32)> = Vec::new();
        for (index, node) in words.chunks_exact(2).enumerate() {
            let index = index as u32;
            match open.last_mut() {
                Some((parent, owed)) => {
                    out.children[*parent as usize].push(index);
                    *owed -= 1;
                }
                // A second root: not one tree.
                None if index > 0 => return None,
                None => {}
            }
            while open.last().is_some_and(|(_, owed)| *owed == 0) {
                open.pop();
            }
            out.facts.push(node[0]);
            if node[1] > 0 {
                open.push((index, node[1]));
            }
        }
        open.is_empty().then_some(out)
    }
}

impl Tree for Words {
    type Node = u32;
    fn facts(&self, node: u32) -> u32 {
        self.facts[node as usize]
    }
    fn children(&self, node: u32) -> Vec<u32> {
        self.children[node as usize].clone()
    }
}

/// [`header_shape_words`]'s flags: the platform's own Back stands for the
/// stack's Back control on this route.
pub const BACK_IS_PLATFORMS: u32 = 1;
/// The header as the route's bar is projected from it ([`header_bar`]),
/// not its shape alone ([`header_shape`]).
pub const PROJECTED: u32 = 2;

/// [`header_shape`] (or, with [`PROJECTED`], [`header_bar`]) for a host
/// that keeps its own mirror of the tree: `words` is the route and its
/// first child's subtree in preorder, two words a node (facts, child
/// count). The answer names nodes by their place in that order: the
/// header, the heading, its level, then one more than the pressable
/// around the heading, the search field and the tablist (0: none), the
/// counts of leading items, trailing items and all items, and those three
/// lists. `None` for a route that is not header-shaped, or words that are
/// not one tree.
pub fn header_shape_words(words: &[u32], flags: u32) -> Option<Vec<u32>> {
    let tree = Words::read(words)?;
    let back_is_platforms = flags & BACK_IS_PLATFORMS != 0;
    let shape = match flags & PROJECTED != 0 {
        true => header_bar(&tree, 0, back_is_platforms),
        false => header_shape(&tree, 0, back_is_platforms),
    }?;
    let optional = |node: Option<u32>| node.map_or(0, |node| node + 1);
    let mut out = vec![
        shape.header,
        shape.heading,
        shape.level,
        optional(shape.tap),
        optional(shape.search),
        optional(shape.segments),
        shape.leading.len() as u32,
        shape.trailing.len() as u32,
        shape.items.len() as u32,
    ];
    out.extend(shape.leading);
    out.extend(shape.trailing);
    out.extend(shape.items);
    Some(out)
}

/// The kernel's own tree, with what the kernel does not hold: which nodes
/// hear `press` (the runner's), and the stack's Back control's `id`.
struct Live<'a> {
    kernel: &'a Kernel,
    back: Option<&'a str>,
    pressed: &'a dyn Fn(ViewId) -> bool,
}

fn is_button(node: &NodeRef<'_>) -> bool {
    node.node_type == NodeType::Pressable
        || ControlKind::of(node.node_type, node.props) == Some(ControlKind::Button)
}

impl Tree for Live<'_> {
    type Node = ViewId;
    fn facts(&self, id: ViewId) -> u32 {
        let Some(node) = self.kernel.node(id) else {
            return 0;
        };
        let props = node.props;
        let role = props.str(PropId::AccessibilityRole);
        let mut facts = 0;
        let mut set = |on: bool, bit: u32| {
            if on {
                facts |= bit;
            }
        };
        let heading = node.node_type == NodeType::Text && role == Some("heading");
        set(heading, fact::HEADING);
        set(
            node.node_type == NodeType::TextInput && props.str(PropId::Type) == Some("search"),
            fact::SEARCH,
        );
        set(role == Some("tablist"), fact::TABLIST);
        set((self.pressed)(id), fact::PRESS);
        set(
            is_button(&node)
                && props.str(PropId::Popovertarget).is_some()
                && props.str(PropId::Popovertargetaction) != Some("hide"),
            fact::MENU,
        );
        set(
            self.back.is_some() && props.str(PropId::Id) == self.back,
            fact::BACK,
        );
        set(
            props.str(PropId::SemanticTag) == Some("header"),
            fact::HEADER,
        );
        set(node.style.display == Display::None, fact::HIDDEN);
        set(
            node.style.appearance == Appearance::None
                && node.style.written.has(StyleId::Appearance),
            fact::AUTHORED,
        );
        set(
            props.str(PropId::NavigationBar) == Some("none"),
            fact::NO_BAR,
        );
        if heading {
            if let Some(PropValue::Int(level @ 1..=6)) =
                props.get(PropId::AccessibilityHeadingLevel)
            {
                facts |= (*level as u32) << fact::LEVEL;
            }
        }
        facts
    }
    fn children(&self, id: ViewId) -> Vec<ViewId> {
        self.kernel
            .node(id)
            .map(|node| node.children())
            .unwrap_or_default()
    }
}

/// A navigation root's tabs: its own tablist, the tabs of it that name a
/// panel, and those panels, in tab order.
#[derive(Debug, Clone, PartialEq)]
pub struct RootTabs {
    /// The `tablist`.
    pub tablist: ViewId,
    /// Its tabs that name one of the root's tabpanels with `aria-controls`.
    pub tabs: Vec<ViewId>,
    /// The panel each names: one tab's stack.
    pub panels: Vec<ViewId>,
}

impl Kernel {
    /// The header of `route` as its bar is projected from it
    /// ([`header_bar`]), read from the tree as it now stands. `back` is the
    /// `id` the navigation root names as its Back control; `pressed` says
    /// whether a node hears `press`.
    pub fn header_shape(
        &self,
        route: ViewId,
        back: Option<&str>,
        back_is_platforms: bool,
        pressed: &dyn Fn(ViewId) -> bool,
    ) -> Option<HeaderShape<ViewId>> {
        let tree = Live {
            kernel: self,
            back,
            pressed,
        };
        header_bar(&tree, route, back_is_platforms)
    }

    /// The tabs of the navigation root `root` (LLP 1075.003 §3.7), or
    /// `None` when it has none (one stack): the first of its own tablists,
    /// in tree order, with a tab that names one of its own tabpanels. What
    /// is inside a route is that route's, and a tablist's own subtree holds
    /// no panel of the root's.
    pub fn root_tabs(&self, root: ViewId) -> Option<RootTabs> {
        fn walk(
            kernel: &Kernel,
            ids: &[ViewId],
            lists: &mut Vec<ViewId>,
            panels: &mut Vec<ViewId>,
        ) {
            for id in ids {
                let Some(node) = kernel.node(*id) else {
                    continue;
                };
                let role = node.props.str(PropId::AccessibilityRole);
                if role == Some("tabpanel") {
                    panels.push(*id);
                }
                if role == Some("tablist") {
                    lists.push(*id);
                }
                if node.props.str(PropId::NavigationKey).is_none() && role != Some("tablist") {
                    walk(kernel, &node.children(), lists, panels);
                }
            }
        }
        let (mut lists, mut panels) = (Vec::new(), Vec::new());
        walk(self, &self.node(root)?.children(), &mut lists, &mut panels);
        lists.into_iter().find_map(|tablist| {
            let (tabs, named): (Vec<ViewId>, Vec<ViewId>) = self
                .node(tablist)?
                .children()
                .into_iter()
                .filter_map(|tab| {
                    let node = self.node(tab)?;
                    if node.props.str(PropId::AccessibilityRole) != Some("tab") {
                        return None;
                    }
                    let name = node.props.str(PropId::AccessibilityControls)?;
                    let panel = panels.iter().copied().find(|panel| {
                        self.node(*panel)
                            .is_some_and(|n| n.props.str(PropId::Id) == Some(name))
                    })?;
                    Some((tab, panel))
                })
                .unzip();
            (!named.is_empty()).then_some(RootTabs {
                tablist,
                tabs,
                panels: named,
            })
        })
    }

    /// A text node's words as a bar sets them (a title, a label): its
    /// runs, white space collapsed as its `white-space` has the web
    /// collapse it; `None` when blank or no text.
    pub fn bar_text(&self, view: ViewId) -> Option<String> {
        crate::grouped::text_of(&self.node(view)?)
    }
}
