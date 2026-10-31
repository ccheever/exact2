//! LLP 1115.000 D4.15 — a page built ahead of time marks the header that is
//! its route's bar `data-exact-bar`, by the kernel's rule
//! (`exact_kernel::bars::header_bar`), as navigation.js marks it (`barItems`).
//! The mark sizes the bar's symbols (neutral.css), so they are the bar's size
//! before the page adopts. A site's header is marked `data-exact-site`
//! instead ([`site_header`]), so it is drawn as written before then too.

use super::Source;
use exact_kernel::{NodeFacts, PropId, SortedMap, SortedSet, ViewId};
use exact_plan::EventKind;

/// The page's nodes and which hear `press`, as the kernel's header rule reads
/// a tree ([`exact_kernel::bars::Tree`]).
struct BarTree<'t, S: Source> {
    src: &'t S,
    handlers: &'t SortedMap<ViewId, Vec<EventKind>>,
}

impl<S: Source> exact_kernel::bars::Tree for BarTree<'_, S> {
    type Node = ViewId;
    fn facts(&self, id: ViewId) -> u32 {
        let Some(node) = self.src.facts(id) else {
            return 0;
        };
        let props = node.props;
        let press = self
            .handlers
            .get(&id)
            .is_some_and(|kinds| kinds.contains(&EventKind::Press));
        // A button that shows or toggles a popover: `Invocation::opens_popover`
        // restated over the source's names, since `Invocation::of` resolves a
        // name through a kernel and a render's tree (`DocTree`) has none.
        let named = |prop| {
            let name = props.str(prop).filter(|name| !name.is_empty())?;
            self.src.facts(self.src.resolve(id, name)?)
        };
        let button = node.node_type == exact_kernel::NodeType::Pressable
            || exact_kernel::ControlKind::of(node.node_type, props)
                == Some(exact_kernel::ControlKind::Button);
        let is = |prop, words: &[&str]| {
            props
                .str(prop)
                .is_some_and(|w| words.iter().any(|v| w.eq_ignore_ascii_case(v)))
        };
        let menu = button
            && match named(PropId::Commandfor) {
                Some(commanded) => {
                    commanded.props.get(PropId::Popover).is_some()
                        && is(PropId::Command, &["show-popover", "toggle-popover"])
                }
                None => {
                    named(PropId::Popovertarget)
                        .is_some_and(|t| t.props.get(PropId::Popover).is_some())
                        && !is(PropId::Popovertargetaction, &["hide"])
                }
            };
        exact_kernel::bars::fact::of(&node, press, menu, None)
    }
    fn children(&self, id: ViewId) -> Vec<ViewId> {
        self.src.children(id).into_owned()
    }
}

/// The header iOS makes `route`'s bar of, if any, unless it is a site's
/// ([`site_header`]), which the web draws as written.
pub(super) fn bar_header<S: Source>(
    src: &S,
    handlers: &SortedMap<ViewId, Vec<EventKind>>,
    route: ViewId,
) -> Option<ViewId> {
    exact_kernel::bars::header_bar(&BarTree { src, handlers }, route, false)
        .map(|shape| shape.header)
        .filter(|header| !site_header(src, *header))
}

/// LLP 1153 D8 — whether `header` is a site's: it holds a `nav`, or more
/// than one link, outside any `popover` or `dialog` in it, whose contents
/// are the top layer's. The shell draws it as written, with no app bar
/// (`data-exact-site`), as navigation.js `siteHeader` decides on a live page.
pub(super) fn site_header<S: Source>(src: &S, header: ViewId) -> bool {
    fn holds<S: Source>(src: &S, id: ViewId, links: &mut u32) -> bool {
        src.children(id).iter().any(|&child| {
            let Some(node) = src.facts(child) else {
                return false;
            };
            let props = node.props;
            let tag = props.str(PropId::SemanticTag);
            if props.get(PropId::Popover).is_some() || tag == Some("dialog") {
                return false;
            }
            // A link: a node with an `href` (an `a`), or whose role is `link`.
            let link = props.str(PropId::Href).is_some()
                || props.str(PropId::AccessibilityRole) == Some("link");
            if link {
                *links += 1;
            }
            tag == Some("nav") || *links > 1 || holds(src, child, links)
        })
    }
    holds(src, header, &mut 0)
}

/// Mark the headers of `node` that are a site's ([`site_header`]) when it
/// is a route, as `navigation.project` marks them: any node with a
/// `navigationKey`, whether or not its root names a Back control.
pub(super) fn mark_sites<S: Source>(
    src: &S,
    node: &NodeFacts<'_>,
    children: &[ViewId],
    sites: &mut SortedSet<ViewId>,
) {
    if node.props.str(PropId::NavigationKey).is_none() {
        return;
    }
    for &child in children {
        let header = src
            .facts(child)
            .is_some_and(|c| c.props.str(PropId::SemanticTag) == Some("header"));
        if header && site_header(src, child) {
            sites.insert(child);
        }
    }
}
