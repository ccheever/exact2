//! Structure projected into a shell's bars (LLP 1115.001 D11): where a
//! shell has bars of the platform's (Android's reader: Material's top app
//! bar and its navigation bar, `presenter/bars.rs`), a route's `header` is
//! the top bar and the navigation root's `tablist` the navigation bar, by
//! the rule every such host reads (`exact_kernel::bars`).
//!
//! This is the host's half: after each commit it reads the first navigation
//! root's routes and tabs by that rule, and takes what a bar stands for out
//! of the page, as iOS does (`HostCover`, LLP 1075.003 §3.5):
//!
//! - the header a top bar replaces takes no room and is neither painted nor
//!   hit ([`crate::navigation::Navigation::lifted`]). A header that holds a
//!   search field or a tablist keeps those in the page under the bar: then
//!   only what the bar shows of it (the heading, its buttons, whatever else
//!   holds neither) is taken out;
//! - the tablist the navigation bar replaces, likewise;
//! - each route is inset by the room its bars take: the top bar's at its
//!   top edge, the navigation bar's at its bottom.
//!
//! The room is the shell's to say ([`Host::set_bar_room`]: it depends on
//! the window, the system's insets and the keyboard, which the presenter
//! holds). With no shell that has bars it is `None`, nothing is projected
//! and nothing is covered: desktop Linux and a headless run paint the
//! authored header and tablist as they always did.
use super::Host;
use crate::navigation::RouteBar;
use exact_kernel::{
    Appearance, Bar, Display, HeaderShape, HostCover, Kernel, PropId, StyleId, ViewId,
};
use exact_plan::EventKind;
use exact_runner::DataSource;
use std::collections::BTreeMap;

/// Whether `node` is `ancestor` or inside it.
fn within(kernel: &Kernel, node: ViewId, ancestor: ViewId) -> bool {
    let mut at = Some(node);
    while let Some(id) = at {
        if id == ancestor {
            return true;
        }
        at = kernel.node(id).and_then(|n| n.parent);
    }
    false
}

/// The nodes of a header its bar stands for: the header itself, or, where
/// it holds a search field or a tablist (which stay in the page under the
/// bar), every part of it that holds neither.
fn lifted(kernel: &Kernel, shape: &HeaderShape<ViewId>) -> Vec<ViewId> {
    let kept: Vec<ViewId> = shape.search.into_iter().chain(shape.segments).collect();
    if kept.is_empty() {
        return vec![shape.header];
    }
    fn walk(kernel: &Kernel, node: ViewId, kept: &[ViewId], out: &mut Vec<ViewId>) {
        for child in kernel.node(node).map(|n| n.children()).unwrap_or_default() {
            if kept.contains(&child) {
                continue;
            }
            match kept.iter().any(|k| within(kernel, *k, child)) {
                true => walk(kernel, child, kept, out),
                false => out.push(child),
            }
        }
    }
    let mut out = Vec::new();
    walk(kernel, shape.header, &kept, &mut out);
    out
}

impl<D: DataSource> Host<D> {
    /// What the shell's bars are made from, as of the last commit.
    pub fn bars(&self) -> &crate::navigation::Projection {
        &self.navigation.bars
    }

    /// Hidden and inert, as [`Host::route_visibility`] has them, but for what
    /// a shell's bar stands for: a node of a header the bar replaces is
    /// out of the page, and still its route's own to press.
    pub(crate) fn authored_visibility(&self, id: ViewId) -> (bool, bool) {
        self.navigation
            .visibility_of(self.runner.kernel(), id, false)
    }

    /// The room a shell's bars take of a route (the top bar from its top
    /// edge, the navigation bar from its bottom; points), or `None` for a
    /// shell with no bars. The tree is projected again and, where what the
    /// bars cover changed, laid out: whether it was.
    pub fn set_bar_room(&mut self, room: Option<(f32, f32)>) -> Result<bool, String> {
        if self.navigation.bars.room == room {
            return Ok(false);
        }
        self.navigation.bars.room = room;
        match self.project_bars() {
            true => self.layout().map(|_| true),
            false => Ok(false),
        }
    }

    /// Read the navigation root's bars from the tree as it now stands and
    /// tell the kernel what they cover; whether that changed (a layout is
    /// then owed, which a commit's own follows with).
    pub(super) fn project_bars(&mut self) -> bool {
        let kernel = self.runner.kernel();
        let room = self.navigation.bars.room;
        let root = room
            .filter(|_| kernel.has_prop(PropId::NavigationBack))
            .and_then(|_| {
                kernel
                    .preorder_where(&self.runner.roots(), |_, props| {
                        props.str(PropId::NavigationBack).is_some()
                    })
                    .into_iter()
                    .next()
            });
        let mut wanted: BTreeMap<ViewId, HostCover> = BTreeMap::new();
        // Where a shell shows the top layer in windows of its own (D10), a
        // popover is never the page's: it takes no room there, as the web
        // gives a closed one none. In the page's flow it took its rows'
        // height, which Shelf's row menu took of the list above it.
        if self.navigation.top_layer && kernel.has_prop(PropId::Popover) {
            let popovers = kernel.preorder_where(&self.runner.roots(), |_, props| {
                props.str(PropId::Popover).is_some()
            });
            wanted.extend(popovers.into_iter().map(|id| (id, HostCover::Whole)));
        }
        let (mut routes, mut tabs, mut stacks) = (Vec::new(), None, Vec::new());
        if let (Some(root), Some((above, below))) = (root, room) {
            let back = kernel
                .node(root)
                .and_then(|n| n.props.str(PropId::NavigationBack))
                .filter(|name| !name.is_empty());
            let pressed = |id: ViewId| self.runner.handlers_of(id).contains(&EventKind::Press);
            let (found, all) = crate::navigation::stacks(kernel, root);
            // A tablist hidden as authored is no bar, as the web shows none;
            // nor is one whose author wrote `appearance: none`, which keeps
            // its authored bar (LLP 1075.003 §9.10).
            let found = found.filter(|tabs| {
                kernel.node(tabs.tablist).is_some_and(|n| {
                    n.style.display != Display::None
                        && !(n.style.appearance == Appearance::None
                            && n.style.written.has(StyleId::Appearance))
                })
            });
            let below = if found.is_some() { below } else { 0.0 };
            if let Some(tabs) = &found {
                wanted.insert(tabs.tablist, HostCover::Whole);
            }
            for stack in &all {
                for (index, route) in stack.iter().enumerate() {
                    let Some(node) = kernel.node(*route) else {
                        continue;
                    };
                    // A presented route starts a stack of its own (LLP
                    // 1035.001 D4): it is not pushed, and its Back control
                    // is an item of its bar.
                    let presented = matches!(
                        node.props.str(PropId::NavigationPresentation),
                        Some("modal" | "fullscreen")
                    );
                    let pushed = index > 0 && !presented;
                    let shape = kernel.header_shape(*route, back, pushed, &pressed);
                    let asked_none = node.props.str(PropId::NavigationBar) == Some("none");
                    // Android's Back is the system's, not a bar's button: a
                    // pushed route with no header of the bar's has no bar.
                    let shows = Bar::of(asked_none, shape.is_some(), pushed, false) == Bar::Header;
                    let mut top = 0.0;
                    if let Some(shape) = shape.filter(|_| shows) {
                        top = above;
                        for id in lifted(kernel, &shape) {
                            wanted.insert(id, HostCover::Whole);
                        }
                        routes.push(RouteBar {
                            route: *route,
                            pushed,
                            shape,
                        });
                    }
                    if top > 0.0 || below > 0.0 {
                        wanted.insert(*route, HostCover::Edges([top, 0.0, below, 0.0]));
                    }
                }
            }
            stacks = all;
            tabs = found;
        }
        let bars = &mut self.navigation.bars;
        (bars.root, bars.routes, bars.tabs, bars.stacks) = (root, routes, tabs, stacks);
        let before = std::mem::replace(&mut bars.covers, wanted);
        self.navigation.lifted = (self.navigation.bars.covers.iter())
            .filter(|(_, cover)| **cover == HostCover::Whole)
            .map(|(id, _)| *id)
            .collect();
        // Against what the kernel holds, not what it was last told: a node
        // that went took its cover with it, and its id may be another's now.
        let kernel = self.runner.kernel_mut();
        let held = |kernel: &Kernel, id: ViewId| {
            let arena = kernel.arena();
            arena.slot_of(id).and_then(|slot| arena.cover(slot))
        };
        let mut changed = false;
        for id in before.keys() {
            if !self.navigation.bars.covers.contains_key(id) && held(kernel, *id).is_some() {
                changed |= kernel.set_host_cover(*id, None).is_ok();
            }
        }
        for (id, cover) in &self.navigation.bars.covers {
            if held(kernel, *id) != Some(*cover) {
                match kernel.set_host_cover(*id, Some(*cover)) {
                    Ok(()) => changed = true,
                    Err(e) => eprintln!("exact: bars: {e:?}"),
                }
            }
        }
        changed
    }
}
