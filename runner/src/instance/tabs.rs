//! Tabs built on first selection (LLP 1075.003 §3.7). A panel route, a node
//! with a `navigationKey` whose enclosing node is a literal
//! `role="tabpanel"` under a navigation root, has no children until its panel
//! holds the selected route: one whose `navigationKey` equals the root's.
//! From then on the panel stays built. A route that is or sits in a node with
//! a `virtualized` binding is built at once: a collection builds its own rows.

use super::*;
use exact_kernel::PropId;
use exact_plan::{Opcode, StrId};

/// The plan's panel routes, each with its panel and navigation root.
#[derive(Debug)]
pub struct Tabs {
    routes: Vec<Option<(NodesId, NodesId)>>,
    panel: Vec<bool>,
    /// A panel or an ancestor of one: the nodes the opening pass enters.
    path: Vec<bool>,
}

impl Tabs {
    /// The plan's tabs, or `None` when it has no panel route.
    pub fn of(plan: &Plan) -> Option<Tabs> {
        let n = plan.nodes.len();
        let mut tabs = Tabs {
            routes: vec![None; n],
            panel: vec![false; n],
            path: vec![false; n],
        };
        for i in 0..n {
            let node = NodesId(i as u32);
            if !has_prop(plan, node, PropId::NavigationKey) {
                continue;
            }
            let Some(panel) = enclosing(plan, node).filter(|p| is_panel(plan, *p)) else {
                continue;
            };
            let mut root = enclosing(plan, panel);
            while let Some(at) = root.filter(|at| !has_prop(plan, *at, PropId::NavigationKey)) {
                root = enclosing(plan, at);
            }
            let Some(root) = root else { continue };
            let mut at = Some(node);
            while let Some(a) = at.filter(|a| !has_prop(plan, *a, PropId::Virtualized)) {
                at = enclosing(plan, a);
            }
            if at.is_some() {
                continue;
            }
            tabs.routes[i] = Some((panel, root));
            tabs.panel[panel.0 as usize] = true;
            let mut at = Some(panel);
            while let Some(p) = at.filter(|p| !tabs.path[p.0 as usize]) {
                tabs.path[p.0 as usize] = true;
                at = enclosing(plan, p);
            }
        }
        tabs.routes.iter().any(Option::is_some).then_some(tabs)
    }

    /// A panel route's panel and navigation root.
    pub fn route(&self, node: NodesId) -> Option<(NodesId, NodesId)> {
        self.routes[node.0 as usize]
    }
}

/// The nearest plan node enclosing `node`, through the regions between.
fn enclosing(plan: &Plan, node: NodesId) -> Option<NodesId> {
    let row = plan.node(node);
    if row.parent.is_some() {
        return row.parent;
    }
    let mut arm = row.arm;
    while let Some(a) = arm {
        let region = plan.region(plan.arms[a.0 as usize].region);
        if region.parent.is_some() {
            return region.parent;
        }
        arm = region.arm;
    }
    None
}

fn has_prop(plan: &Plan, node: NodesId, prop: PropId) -> bool {
    plan.node(node).bindings.iter().any(|b| {
        let b = plan.binding(b);
        b.kind == BindingKind::Prop && b.id == prop as u16
    })
}

/// Whether `node` has a literal `role="tabpanel"`.
pub fn is_panel(plan: &Plan, node: NodesId) -> bool {
    plan.node(node).bindings.iter().any(|b| {
        let b = plan.binding(b);
        b.kind == BindingKind::Prop
            && b.id == PropId::AccessibilityRole as u16
            && match plan.code(b.expr) {
                [op, a, b, c, d, ret]
                    if *op == Opcode::Str as u8 && *ret == Opcode::Return as u8 =>
                {
                    let id = u32::from_le_bytes([*a, *b, *c, *d]);
                    (id as usize) < plan.strings.len() && plan.str(StrId(id)) == "tabpanel"
                }
                _ => false,
            }
    })
}

/// Open the panels that hold their root's selected route and build their
/// routes. Runs at the end of every create and update of the tree, so a
/// selection and its panel's first build are one batch.
pub(super) fn open(children: &mut [Child], u: &mut Update<'_>) -> Result<(), InstanceError> {
    if u.sites.tabs.is_some() {
        open_all(children, u, &[], None, false)?;
    }
    Ok(())
}

/// `key` is the nearest enclosing navigation root's `navigationKey`, and
/// `opened` whether the enclosing panel, if any, is open.
fn open_all(
    children: &mut [Child],
    u: &mut Update<'_>,
    frames: &[Frame],
    key: Option<&Value>,
    opened: bool,
) -> Result<(), InstanceError> {
    for c in children {
        match c {
            Child::Node(n) => n.open(u, frames, key, opened)?,
            Child::Region(r) => match &mut r.active {
                Active::Arm { roots, frame, .. } => {
                    open_all(roots, u, &with_frame(frames, frame.clone()), key, opened)?
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let inner = with_frame(frames, row.frame.clone());
                        open_all(&mut row.roots, u, &inner, key, opened)?;
                    }
                }
            },
        }
    }
    Ok(())
}

/// The nodes among `children`, through regions but not into nodes.
fn nodes_in<'a>(children: &'a [Child], out: &mut Vec<&'a NodeInst>) {
    for c in children {
        match c {
            Child::Node(n) => out.push(n),
            Child::Region(r) => match &r.active {
                Active::Arm { roots, .. } => nodes_in(roots, out),
                Active::Rows { rows } => rows.iter().for_each(|row| nodes_in(&row.roots, out)),
            },
        }
    }
}

impl NodeInst {
    fn open(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        key: Option<&Value>,
        opened: bool,
    ) -> Result<(), InstanceError> {
        let sites = u.sites;
        let tabs = sites.tabs.as_ref().expect("checked by open");
        let plan = u.env.plan;
        let i = self.node.0 as usize;
        if self.deferred && opened {
            self.deferred = false;
            self.children = realize(u, Some(self.node), plan.node(self.node).arm, frames)?;
            self.emit_children(u);
        }
        if !tabs.path[i] || self.collection.is_some() {
            return Ok(());
        }
        if tabs.panel[i] && !self.opened {
            let mut routes = Vec::new();
            nodes_in(&self.children, &mut routes);
            self.opened = key.is_some_and(|key| {
                routes.iter().any(|r| {
                    r.bound_prop(plan, PropId::NavigationKey)
                        .is_some_and(|k| crate::compare::equal(k, key) == Some(true))
                })
            });
        }
        let own = self.bound_prop(plan, PropId::NavigationKey).cloned();
        let key = own.as_ref().or(key);
        let opened = tabs.panel[i] && self.opened;
        open_all(&mut self.children, u, frames, key, opened)
    }
}
