//! LLP 1115.000 D4.15 — a page built ahead of time marks the header that is
//! its route's bar `data-exact-bar`, by the kernel's rule
//! (`exact_kernel::bars::header_bar`), as navigation.js marks it (`barItems`).
//! The mark sizes the bar's symbols (neutral.css), so they are the bar's size
//! before the page adopts.

use super::Source;
use exact_kernel::{PropId, SortedMap, ViewId};
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

/// The header iOS makes `route`'s bar of, if any.
pub(super) fn bar_header<S: Source>(
    src: &S,
    handlers: &SortedMap<ViewId, Vec<EventKind>>,
    route: ViewId,
) -> Option<ViewId> {
    exact_kernel::bars::header_bar(&BarTree { src, handlers }, route, false)
        .map(|shape| shape.header)
}
