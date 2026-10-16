//! A navigation tab built when it is first selected (LLP 1075.003 §3.7).
//!
//! A tab panel ([`panel_sites`]) is made empty. Its contents — their views,
//! rows and the state their components declare — are made in the update
//! whose tree first selects it, as `UITabBarController` loads a tab's view
//! controller when the tab is first selected and SwiftUI's `TabView` builds
//! a tab's content when it is first shown. Once made they stay, as a visited
//! tab's stack does. The inliner makes a panel's contents one `if true` arm
//! (`contract_syntax::inline`), which owns the state of a component used
//! directly in it, so that state too starts at the first selection.
//!
//! Which panel is selected is what every host's projection reads
//! (`head.rs` `unselected`): the panel whose routes hold the navigation
//! root's `navigationKey`. While no made panel holds it, the panel whose tab
//! is `aria-selected` is made, else the first waiting one in tab order,
//! until one does or none waits; a root that names no route of any panel
//! shows all of them, as the projection does.

use super::*;
use crate::head::{tab_panels, HeadNode};
use exact_kernel::{PropId, PropList, PropValue};
use std::collections::HashMap;

/// The props the selection reads.
const READ: [PropId; 6] = [
    PropId::AccessibilityRole,
    PropId::Id,
    PropId::NavigationKey,
    PropId::NavigationBack,
    PropId::AccessibilityControls,
    PropId::AccessibilitySelected,
];

/// The plan's tab panels, sorted: each node whose `role` is the literal
/// `"tabpanel"` and whose nearest ancestor with a `navigationKey` binding is
/// a navigation root (it binds `navigationBack` too), not a route — the
/// panels a root's tablist names (`head.rs` `tab_panels`) when the app
/// writes them as the recipe does. The web JS target defers the same sites.
pub fn panel_sites(plan: &Plan) -> Vec<NodesId> {
    let binds = |n: NodesId, prop: PropId| {
        plan.node(n)
            .bindings
            .iter()
            .any(|b| plan.binding(b).kind == BindingKind::Prop && plan.binding(b).id == prop as u16)
    };
    let tabpanel = |n: NodesId| {
        plan.node(n).bindings.iter().any(|b| {
            let b = plan.binding(b);
            b.kind == BindingKind::Prop
                && b.id == PropId::AccessibilityRole as u16
                && matches!(
                    crate::vm::instructions(plan.code(b.expr))
                        .collect::<Result<Vec<_>, _>>()
                        .as_deref(),
                    Ok([s, r]) if s.op == exact_plan::Opcode::Str
                        && r.op == exact_plan::Opcode::Return
                        && plan.str(exact_plan::StrId(s.args[0] as u32)) == "tabpanel"
                )
        })
    };
    // A site's enclosing node: its parent, else its arm's region's.
    let up = |parent: Option<NodesId>, mut arm: Option<ArmsId>| {
        if parent.is_some() {
            return parent;
        }
        while let Some(a) = arm {
            let region = plan.region(plan.arm(a).region);
            if region.parent.is_some() {
                return region.parent;
            }
            arm = region.arm;
        }
        None
    };
    (0..plan.nodes.len() as u32)
        .map(NodesId)
        .filter(|&n| tabpanel(n))
        .filter(|&n| {
            let mut at = up(plan.node(n).parent, plan.node(n).arm);
            while let Some(a) = at {
                if binds(a, PropId::NavigationKey) {
                    return binds(a, PropId::NavigationBack);
                }
                at = up(plan.node(a).parent, plan.node(a).arm);
            }
            false
        })
        .collect()
}

/// The instance tree as the selection reads it: each node's [`READ`] props
/// as its bindings last left them, its children as its kernel view has them,
/// and its parent.
#[derive(Default)]
struct Index {
    nodes: HashMap<ViewId, (NodeType, PropList, Vec<ViewId>)>,
    parent: HashMap<ViewId, ViewId>,
}

impl Index {
    fn of(plan: &Plan, children: &[Child]) -> Index {
        let mut index = Index::default();
        index.visit(plan, children, None);
        index
    }

    fn visit(&mut self, plan: &Plan, children: &[Child], parent: Option<ViewId>) {
        for c in children {
            match c {
                Child::Node(n) => {
                    let row = plan.node(n.node);
                    let mut props = PropList::new();
                    for (i, b) in row.bindings.iter().enumerate() {
                        let b = plan.binding(b);
                        let read = READ.iter().any(|p| *p as u16 == b.id);
                        if let (BindingKind::Prop, true, Some(v)) = (b.kind, read, &n.last[i]) {
                            if let Ok((prop, value)) = bridge::prop_value(b.id, v) {
                                props.set(prop, value);
                            }
                        }
                    }
                    let kids = match n.collection {
                        Some(_) => Vec::new(),
                        None => roots_of(&n.children),
                    };
                    let node_type = NodeType::from_wire(row.node_type).unwrap_or(NodeType::View);
                    self.nodes.insert(n.view, (node_type, props, kids));
                    if let Some(p) = parent {
                        self.parent.insert(n.view, p);
                    }
                    self.visit(plan, &n.children, Some(n.view));
                }
                Child::Region(r) => match &r.active {
                    Active::Arm { roots, .. } => self.visit(plan, roots, parent),
                    Active::Rows { rows } => {
                        for row in rows {
                            self.visit(plan, &row.roots, parent);
                        }
                    }
                },
            }
        }
    }

    fn node(&self, id: ViewId) -> Option<HeadNode<'_>> {
        self.nodes.get(&id).map(|(t, p, c)| (*t, p, c.clone()))
    }

    /// The navigation root whose tab panel `panel` may be: its nearest
    /// ancestor with a `navigationKey`, when that has a `navigationBack`.
    fn root_of(&self, panel: ViewId) -> Option<ViewId> {
        let mut at = self.parent.get(&panel).copied();
        while let Some(a) = at {
            let props = &self.nodes.get(&a)?.1;
            if props.str(PropId::NavigationKey).is_some() {
                return props.str(PropId::NavigationBack).is_some().then_some(a);
            }
            at = self.parent.get(&a).copied();
        }
        None
    }
}

/// The tab panels that wait, and the views their selection is read from.
#[derive(Debug, Default)]
pub(super) struct Waiting {
    panels: Vec<ViewId>,
    /// Each waiting panel's navigation root and what lies between it and its
    /// routes (its tablist and tabs, its panels), sorted: a child list one
    /// of them changes, or one of them destroyed, can change which tab is
    /// a navigation tab or selected.
    zone: Vec<ViewId>,
}

impl Tree {
    /// The tab panels whose contents wait for their first selection.
    pub(crate) fn waiting(&self) -> &[ViewId] {
        &self.tabs.panels
    }

    /// Make the contents of each waiting panel its root now selects, in
    /// this update's batch; a panel no root's tablist names is made at
    /// once. Asked when this update made a panel, set a prop the selection
    /// reads, or changed the children of or destroyed a view of a waiting
    /// panel's root above its routes.
    pub(super) fn open_tabs(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        let made = !u.waiting.is_empty();
        self.tabs.panels.append(&mut u.waiting);
        let zone = &self.tabs.zone;
        let read = |op: &Op| match op {
            Op::SetProp { prop, .. } | Op::ClearProp { prop, .. } => READ.contains(prop),
            Op::SetChildren { id, .. } | Op::DestroyView { id } => zone.binary_search(id).is_ok(),
            _ => false,
        };
        // A detached kernel's update writes no prop ops to read.
        if self.tabs.panels.is_empty() || !(made || u.discard || u.ops.iter().any(read)) {
            return Ok(());
        }
        loop {
            let index = Index::of(u.env.plan, &self.children);
            self.tabs.panels.retain(|w| index.nodes.contains_key(w));
            let open = self.to_open(&index);
            if open.is_empty() {
                self.tabs.zone = self.zone(&index);
                return Ok(());
            }
            self.tabs.panels.retain(|w| !open.contains(w));
            let mut left = open.len();
            open_in(u, &mut self.children, &[], &open, &mut left)?;
            self.tabs.panels.append(&mut u.waiting);
        }
    }

    /// [`Waiting::zone`], over `index`.
    fn zone(&self, index: &Index) -> Vec<ViewId> {
        let mut zone = Vec::new();
        let mut stack: Vec<ViewId> = self
            .tabs
            .panels
            .iter()
            .filter_map(|p| index.root_of(*p))
            .collect();
        while let Some(id) = stack.pop() {
            let Some((_, props, kids)) = index.nodes.get(&id) else {
                continue;
            };
            let route = props.str(PropId::NavigationKey).is_some()
                && props.str(PropId::NavigationBack).is_none();
            if route || zone.contains(&id) {
                continue;
            }
            zone.push(id);
            stack.extend(kids);
        }
        zone.sort_unstable();
        zone
    }

    /// The waiting panels to make now: at most one per navigation root.
    fn to_open(&self, index: &Index) -> Vec<ViewId> {
        let node = |id| index.node(id);
        let key_of = |id| {
            index
                .nodes
                .get(&id)
                .and_then(|n| n.1.str(PropId::NavigationKey))
        };
        let named: Vec<&str> = index
            .nodes
            .values()
            .filter(|(_, p, _)| {
                p.get(PropId::AccessibilitySelected) == Some(&PropValue::Bool(true))
            })
            .filter_map(|(_, p, _)| p.str(PropId::AccessibilityControls))
            .collect();
        let (mut open, mut roots) = (Vec::new(), Vec::new());
        for &w in &self.tabs.panels {
            let at = index.root_of(w).and_then(|r| Some((r, node(r)?)));
            let panels = at
                .as_ref()
                .map(|(_, at)| tab_panels(&node, &at.2))
                .unwrap_or_default();
            let Some((root, at)) = at.filter(|_| panels.contains(&w)) else {
                open.push(w);
                continue;
            };
            if roots.contains(&root) {
                continue;
            }
            roots.push(root);
            let key = at.1.str(PropId::NavigationKey);
            let held = panels.iter().any(|p| {
                index
                    .nodes
                    .get(p)
                    .is_some_and(|(_, _, kids)| kids.iter().any(|r| key_of(*r) == key))
            });
            if held {
                continue;
            }
            let waiting = || panels.iter().filter(|p| self.tabs.panels.contains(p));
            let chosen = waiting()
                .find(|p| {
                    let id = index.nodes.get(p).and_then(|n| n.1.str(PropId::Id));
                    id.is_some_and(|id| named.contains(&id))
                })
                .or_else(|| waiting().next());
            open.extend(chosen);
        }
        open
    }
}

/// Make the contents of the panels `open` names, found under `children`
/// in `frames`, as [`NodeInst::create`] would have made them.
fn open_in(
    u: &mut Update<'_>,
    children: &mut [Child],
    frames: &[Frame],
    open: &[ViewId],
    left: &mut usize,
) -> Result<(), InstanceError> {
    for c in children {
        if *left == 0 {
            return Ok(());
        }
        match c {
            Child::Node(n) => {
                let saved = n.enter(u, n.tuple);
                let done = if open.contains(&n.view) {
                    *left -= 1;
                    let arm = u.env.plan.node(n.node).arm;
                    realize(u, Some(n.node), arm, frames).map(|made| {
                        n.children = made;
                        n.emit_children(u);
                    })
                } else if n.collection.is_none() {
                    open_in(u, &mut n.children, frames, open, left)
                } else {
                    Ok(())
                };
                u.theme = saved;
                done?;
            }
            Child::Region(r) => match &mut r.active {
                Active::Arm { roots, frame, .. } => {
                    u.with_frame(frames, frame.scope(), |u, inner| {
                        open_in(u, roots, inner, open, left)
                    })?;
                }
                Active::Rows { rows } => {
                    for row in rows {
                        u.with_frame(frames, row.frame.clone(), |u, inner| {
                            open_in(u, &mut row.roots, inner, open, left)
                        })?;
                    }
                }
            },
        }
    }
    Ok(())
}
