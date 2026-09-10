//! The instance tree, and the ops that keep the kernel equal to it.
//!
//! @ref LLP 0485 §8.2 (keyed instance management, never reconciliation of
//! trees; research)
//!
//! The plan's nodes and regions are *sites*. An instance is one realization
//! of a site: a kernel view for a node, an active arm for `when`/`match`, one
//! row per key for `each`. An update re-evaluates every site against the new
//! environment and emits exactly the kernel ops that make the kernel equal to
//! the result: a binding's op only when its value changed, `SetChildren` only
//! when a child list changed, create/destroy only when a key appeared or went
//! away. There is no tree diff: a keyed row keeps its views across reorders
//! because its key, not its position, is its identity.

use crate::bridge;
use crate::vm::{self, Env, Frame, RowSlots, Trap};
use exact_kernel::{NodeType, Op, StyleProps, ViewId};
use exact_plan::{ArmsId, BindingKind, NodesId, Plan, RegionKind, RegionsId, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Why an instance could not be realized.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceError {
    Trap(Trap),
    Bridge(bridge::BridgeError),
    UnknownNodeType(u8),
    SubjectKind { region: RegionsId },
    KeyKind { region: RegionsId },
    DuplicateKey { region: RegionsId },
    SlotType { slot: String },
}

impl From<Trap> for InstanceError {
    fn from(t: Trap) -> Self {
        InstanceError::Trap(t)
    }
}

/// One realized node.
#[derive(Debug)]
pub struct NodeInst {
    /// The site.
    pub node: NodesId,
    /// The kernel view.
    pub view: ViewId,
    /// Last emitted value per binding, in binding order.
    last: Vec<Option<Value>>,
    /// Last published surface inputs (a canvas node).
    last_surface: Option<Vec<Value>>,
    /// Ordered children: static nodes and regions interleaved by `order`.
    children: Vec<Child>,
    /// Last emitted child list.
    last_children: Vec<ViewId>,
}

#[derive(Debug)]
enum Child {
    Node(NodeInst),
    Region(RegionInst),
}

/// One realized region.
#[derive(Debug)]
pub struct RegionInst {
    region: RegionsId,
    active: Active,
}

#[derive(Debug)]
enum Active {
    /// `when` / `match`: the active arm and its roots.
    Arm {
        arm: Option<usize>,
        frame: Frame,
        roots: Vec<Child>,
    },
    /// `each`: rows by key, in current order.
    Rows { rows: Vec<Row> },
}

#[derive(Debug)]
struct Row {
    key: Value,
    frame: Frame,
    roots: Vec<Child>,
    /// The row's own slots (LLP 1017 P4c): the `state` a child component
    /// declared, one value per row, kept across reorders with the key,
    /// dropped with the row, initialized when the row is created.
    slots: RowSlots,
}

/// One step of the instance path from the plan's roots to a view: the
/// region crossed and, for an `each` row, its key; for a `when`/`match`
/// region, the active arm (LLP 1035.002 D6 — what identifies *this*
/// instance of a repeated site).
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceStep {
    /// A keyed row of an `each`.
    Row {
        /// The region.
        region: RegionsId,
        /// The row's key.
        key: Value,
    },
    /// The active arm of a `when`/`match`.
    Arm {
        /// The region.
        region: RegionsId,
        /// Which arm, when one is active.
        arm: Option<usize>,
    },
}

/// Allocates kernel view ids; never reuses one within a runner's life.
#[derive(Debug, Default)]
pub struct Ids {
    next: ViewId,
}

impl Ids {
    fn fresh(&mut self) -> ViewId {
        self.next += 1;
        self.next
    }
}

/// A canvas node's surface inputs, evaluated against state: the runner's
/// side-output for the host's GPU module (LLP 1009 D2). Published only
/// after the commit that produced it applied.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceUpdate {
    /// The canvas node's kernel view.
    pub view: ViewId,
    /// The surface's name in the app's GPU module.
    pub name: String,
    /// Its arguments, evaluated.
    pub values: Vec<Value>,
}

/// What one update needs: the environment and the id allocator, plus the op
/// batch under construction and the surface inputs that changed.
pub struct Update<'a> {
    /// The environment every expression sees.
    pub env: Env<'a>,
    /// The id allocator.
    pub ids: &'a mut Ids,
    /// Ops accumulated for one atomic `Kernel::apply`.
    pub ops: Vec<Op>,
    /// Surface inputs that changed, in tree order.
    pub surfaces: Vec<SurfaceUpdate>,
}

impl<'a> Update<'a> {
    fn eval(&self, code: exact_plan::Code, frames: &[Frame]) -> Result<Value, Trap> {
        let env = Env { frames, ..self.env };
        Ok(vm::eval(self.env.plan.code(code), &env, &[])?.value)
    }
}

/// The sites directly under `parent` within `arm` (or the root sites when
/// both are `None`), in `order`, as (order, node-or-region).
fn sites(plan: &Plan, parent: Option<NodesId>, arm: Option<ArmsId>) -> Vec<(u32, Site)> {
    let mut out = Vec::new();
    for (i, n) in plan.nodes.iter().enumerate() {
        if n.parent == parent && n.arm == arm {
            out.push((n.order, Site::Node(NodesId(i as u32))));
        }
    }
    for (i, r) in plan.regions.iter().enumerate() {
        if r.parent == parent && r.arm == arm {
            out.push((r.order, Site::Region(RegionsId(i as u32))));
        }
    }
    out.sort_by_key(|(order, site)| (*order, site.rank()));
    out
}

#[derive(Debug, Clone, Copy)]
enum Site {
    Node(NodesId),
    Region(RegionsId),
}

impl Site {
    fn rank(self) -> u32 {
        match self {
            Site::Node(n) => n.0,
            Site::Region(r) => r.0,
        }
    }
}

/// Realize the sites under (`parent`, `arm`) for the first time.
fn realize(
    u: &mut Update<'_>,
    parent: Option<NodesId>,
    arm: Option<ArmsId>,
    frames: &[Frame],
) -> Result<Vec<Child>, InstanceError> {
    let plan = u.env.plan;
    let mut out = Vec::new();
    for (_, site) in sites(plan, parent, arm) {
        out.push(match site {
            Site::Node(id) => Child::Node(NodeInst::create(u, id, frames)?),
            Site::Region(id) => Child::Region(RegionInst::create(u, id, frames)?),
        });
    }
    Ok(out)
}

fn roots_of(children: &[Child]) -> Vec<ViewId> {
    let mut out = Vec::new();
    for c in children {
        match c {
            Child::Node(n) => out.push(n.view),
            Child::Region(r) => r.collect_roots(&mut out),
        }
    }
    out
}

fn destroy_all(u: &mut Update<'_>, children: Vec<Child>) {
    for c in children {
        match c {
            Child::Node(n) => n.destroy(u),
            Child::Region(r) => r.destroy(u),
        }
    }
}

fn update_all(
    u: &mut Update<'_>,
    children: &mut [Child],
    frames: &[Frame],
) -> Result<(), InstanceError> {
    for c in children.iter_mut() {
        match c {
            Child::Node(n) => n.update(u, frames)?,
            Child::Region(r) => r.update(u, frames)?,
        }
    }
    Ok(())
}

impl NodeInst {
    fn create(
        u: &mut Update<'_>,
        node: NodesId,
        frames: &[Frame],
    ) -> Result<NodeInst, InstanceError> {
        let plan = u.env.plan;
        let row = plan.node(node);
        let node_type = NodeType::from_wire(row.node_type)
            .ok_or(InstanceError::UnknownNodeType(row.node_type))?;
        let view = u.ids.fresh();
        u.ops.push(Op::CreateView {
            id: view,
            node_type,
        });
        let mut inst = NodeInst {
            node,
            view,
            last: vec![None; row.bindings.len as usize],
            last_surface: None,
            children: Vec::new(),
            last_children: Vec::new(),
        };
        inst.emit_bindings(u, frames)?;
        inst.children = realize(u, Some(node), row.arm, frames)?;
        inst.emit_children(u);
        Ok(inst)
    }

    fn emit_bindings(&mut self, u: &mut Update<'_>, frames: &[Frame]) -> Result<(), InstanceError> {
        let plan = u.env.plan;
        let row = plan.node(self.node);
        let mut patch: Option<StyleProps> = None;
        for (i, b) in row.bindings.iter().enumerate() {
            let binding = plan.binding(b);
            let value = u.eval(binding.expr, frames)?;
            if self.last[i].as_ref() == Some(&value) {
                continue;
            }
            match binding.kind {
                BindingKind::Prop => {
                    let (prop, pv) =
                        bridge::prop_value(binding.id, &value).map_err(InstanceError::Bridge)?;
                    u.ops.push(Op::SetProp {
                        id: self.view,
                        prop,
                        value: pv,
                    });
                }
                BindingKind::Style => {
                    let p = patch.get_or_insert_with(StyleProps::default);
                    bridge::set_style(p, binding.id, &value, plan.stacks.len())
                        .map_err(InstanceError::Bridge)?;
                }
            }
            self.last[i] = Some(value);
        }
        if let Some(p) = patch {
            u.ops.push(Op::SetStyle {
                id: self.view,
                patch: Box::new(p),
            });
        }
        // A canvas's surface inputs: evaluated with the bindings (so a trap
        // refuses the commit whole), published only with a successful apply.
        if let Some(surface) = row.surface {
            let s = plan.surface(surface);
            let mut values = Vec::with_capacity(s.args.len as usize);
            for a in s.args.iter() {
                values.push(u.eval(plan.arg(a).expr, frames)?);
            }
            if self.last_surface.as_ref() != Some(&values) {
                u.surfaces.push(SurfaceUpdate {
                    view: self.view,
                    name: plan.str(s.name).to_string(),
                    values: values.clone(),
                });
                self.last_surface = Some(values);
            }
        }
        Ok(())
    }

    fn emit_children(&mut self, u: &mut Update<'_>) {
        let now = roots_of(&self.children);
        if now != self.last_children {
            u.ops.push(Op::SetChildren {
                id: self.view,
                children: now.clone(),
            });
            self.last_children = now;
        }
    }

    fn update(&mut self, u: &mut Update<'_>, frames: &[Frame]) -> Result<(), InstanceError> {
        self.emit_bindings(u, frames)?;
        update_all(u, &mut self.children, frames)?;
        self.emit_children(u);
        Ok(())
    }

    fn destroy(self, u: &mut Update<'_>) {
        // Destroying the view destroys its subtree in the kernel; the instance
        // tree just drops.
        u.ops.push(Op::DestroyView { id: self.view });
    }

    /// Find the instance owning `view`, with the frames in force there.
    pub fn find(&self, view: ViewId, frames: &mut Vec<Frame>) -> Option<NodesId> {
        if self.view == view {
            return Some(self.node);
        }
        for c in &self.children {
            match c {
                Child::Node(n) => {
                    if let Some(found) = n.find(view, frames) {
                        return Some(found);
                    }
                }
                Child::Region(r) => {
                    if let Some(found) = r.find(view, frames) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
}

impl RegionInst {
    fn create(
        u: &mut Update<'_>,
        region: RegionsId,
        frames: &[Frame],
    ) -> Result<RegionInst, InstanceError> {
        let mut inst = RegionInst {
            region,
            active: match u.env.plan.region(region).kind {
                RegionKind::Each => Active::Rows { rows: Vec::new() },
                _ => Active::Arm {
                    arm: None,
                    frame: Frame::default(),
                    roots: Vec::new(),
                },
            },
        };
        inst.update(u, frames)?;
        Ok(inst)
    }

    fn update(&mut self, u: &mut Update<'_>, frames: &[Frame]) -> Result<(), InstanceError> {
        let plan = u.env.plan;
        let row = plan.region(self.region);
        let subject = u.eval(row.subject, frames)?;
        match (&row.kind, &mut self.active) {
            (RegionKind::When, Active::Arm { arm, frame, roots }) => {
                let want = match subject {
                    Value::Bool(true) => Some(0),
                    Value::Bool(false) if row.arms.len > 1 => Some(1),
                    Value::Bool(false) => None,
                    _ => {
                        return Err(InstanceError::SubjectKind {
                            region: self.region,
                        })
                    }
                };
                Self::switch(
                    u,
                    self.region,
                    arm,
                    frame,
                    roots,
                    want,
                    Frame::default(),
                    frames,
                )
            }
            (RegionKind::Match, Active::Arm { arm, frame, roots }) => {
                let (want, new_frame) = match subject {
                    Value::Option(Some(v)) => (
                        Some(0),
                        Frame {
                            item: None,
                            bound: Some((*v).clone()),
                            ..Default::default()
                        },
                    ),
                    Value::Option(None) => (
                        if row.arms.len > 1 { Some(1) } else { None },
                        Frame::default(),
                    ),
                    _ => {
                        return Err(InstanceError::SubjectKind {
                            region: self.region,
                        })
                    }
                };
                Self::switch(u, self.region, arm, frame, roots, want, new_frame, frames)
            }
            (RegionKind::Each, Active::Rows { rows }) => {
                let items = match subject {
                    Value::List(items) => items,
                    _ => {
                        return Err(InstanceError::SubjectKind {
                            region: self.region,
                        })
                    }
                };
                let arm = row.arms.iter().next();
                // Key every item; refuse duplicates.
                let mut keyed: Vec<(String, Value, Frame)> = Vec::with_capacity(items.len());
                let mut seen: BTreeMap<String, ()> = BTreeMap::new();
                for item in items.iter() {
                    let frame = Frame {
                        item: Some(item.clone()),
                        bound: None,
                        ..Default::default()
                    };
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    let key = u.eval(row.key, &inner)?;
                    let key_text = key_text(&key).ok_or(InstanceError::KeyKind {
                        region: self.region,
                    })?;
                    if seen.insert(key_text.clone(), ()).is_some() {
                        return Err(InstanceError::DuplicateKey {
                            region: self.region,
                        });
                    }
                    keyed.push((key_text, key, frame));
                }
                // Reuse rows by key, create the new, destroy the gone; order follows the items.
                // A linear search per row made an unchanged 10,000-row list
                // quadratic. Reuse the same canonical keys as duplicate checking.
                // Keep old order for destruction, which is observable in receipts.
                let mut old: Vec<Option<Row>> =
                    std::mem::take(rows).into_iter().map(Some).collect();
                let by_key: BTreeMap<String, usize> = old
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            key_text(&r.as_ref().unwrap().key).expect("validated key"),
                            i,
                        )
                    })
                    .collect();
                let mut next = Vec::with_capacity(keyed.len());
                for (key_text, key, mut frame) in keyed {
                    let existing = by_key.get(&key_text).and_then(|i| old[*i].take());
                    frame.region = Some(self.region.0);
                    match existing {
                        Some(mut r) => {
                            frame.row = Some(r.slots.clone());
                            r.frame = frame.clone();
                            let mut inner = frames.to_vec();
                            inner.push(frame);
                            update_all(u, &mut r.roots, &inner)?;
                            next.push(r);
                        }
                        None => {
                            // A new row: its slots start from their initializers,
                            // evaluated here so an initializer may read the item.
                            let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
                            frame.row = Some(slots.clone());
                            let mut inner = frames.to_vec();
                            inner.push(frame.clone());
                            for (i, s) in plan.slots.iter().enumerate() {
                                if s.owner == Some(self.region) {
                                    let v = u.eval(s.init, &inner)?;
                                    if !v.conforms(plan, s.ty) {
                                        return Err(InstanceError::SlotType {
                                            slot: plan.str(s.name).to_string(),
                                        });
                                    }
                                    slots.borrow_mut().insert(i as u32, v);
                                }
                            }
                            let roots = realize(u, None, arm, &inner)?;
                            next.push(Row {
                                key,
                                frame,
                                roots,
                                slots,
                            });
                        }
                    }
                }
                for gone in old.into_iter().flatten() {
                    destroy_all(u, gone.roots);
                }
                *rows = next;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn switch(
        u: &mut Update<'_>,
        region: RegionsId,
        arm: &mut Option<usize>,
        frame: &mut Frame,
        roots: &mut Vec<Child>,
        want: Option<usize>,
        new_frame: Frame,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        let plan = u.env.plan;
        let mut inner = frames.to_vec();
        inner.push(new_frame.clone());
        if *arm == want {
            *frame = new_frame;
            return update_all(u, roots, &inner);
        }
        let old = std::mem::take(roots);
        destroy_all(u, old);
        *arm = want;
        *frame = new_frame;
        if let Some(i) = want {
            let arm_id = plan.region(region).arms.iter().nth(i);
            *roots = realize(u, None, arm_id, &inner)?;
        }
        Ok(())
    }

    fn collect_roots(&self, out: &mut Vec<ViewId>) {
        match &self.active {
            Active::Arm { roots, .. } => out.extend(roots_of(roots)),
            Active::Rows { rows } => {
                for r in rows {
                    out.extend(roots_of(&r.roots));
                }
            }
        }
    }

    fn destroy(self, u: &mut Update<'_>) {
        match self.active {
            Active::Arm { roots, .. } => destroy_all(u, roots),
            Active::Rows { rows } => {
                for r in rows {
                    destroy_all(u, r.roots);
                }
            }
        }
    }

    fn find(&self, view: ViewId, frames: &mut Vec<Frame>) -> Option<NodesId> {
        match &self.active {
            Active::Arm { roots, frame, .. } => {
                frames.push(frame.clone());
                for c in roots {
                    let found = match c {
                        Child::Node(n) => n.find(view, frames),
                        Child::Region(r) => r.find(view, frames),
                    };
                    if found.is_some() {
                        return found;
                    }
                }
                frames.pop();
                None
            }
            Active::Rows { rows } => {
                for r in rows {
                    frames.push(r.frame.clone());
                    for c in &r.roots {
                        let found = match c {
                            Child::Node(n) => n.find(view, frames),
                            Child::Region(rr) => rr.find(view, frames),
                        };
                        if found.is_some() {
                            return found;
                        }
                    }
                    frames.pop();
                }
                None
            }
        }
    }
}

/// One canonical key text: strings, finite numbers (`-0` is `0`, matching the
/// VM's equality), bools. NaN is not a key.
fn key_text(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(format!("s:{s}")),
        Value::Number(n) if n.is_finite() => {
            Some(format!("n:{}", if *n == 0.0 { 0.0 } else { *n }))
        }
        Value::Bool(b) => Some(format!("b:{b}")),
        _ => None,
    }
}

/// The root of the instance tree: the plan's top-level sites.
#[derive(Debug)]
pub struct Tree {
    children: Vec<Child>,
    last_roots: Vec<ViewId>,
}

impl Tree {
    /// Realize the plan's root sites.
    pub fn create(u: &mut Update<'_>) -> Result<Tree, InstanceError> {
        let children = realize(u, None, None, &[])?;
        let mut tree = Tree {
            children,
            last_roots: Vec::new(),
        };
        tree.emit_roots(u);
        Ok(tree)
    }

    /// Re-evaluate everything.
    pub fn update(&mut self, u: &mut Update<'_>) -> Result<(), InstanceError> {
        update_all(u, &mut self.children, &[])?;
        self.emit_roots(u);
        // Views are never reused within a runner. Detach removed children in
        // the final child lists before destroying them, so a removed list does
        // not rebuild its parent's siblings once per row. Still one atomic
        // batch; preserve relative destroy order for receipts and host effects.
        let (mut live, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut u.ops)
            .into_iter()
            .partition(|op| !matches!(op, Op::DestroyView { .. }));
        live.extend(gone);
        u.ops = live;
        Ok(())
    }

    /// Listener declarations for every live view in one walk, without
    /// reconstructing event argument frames for each created view.
    pub fn handlers(&self, plan: &Plan) -> BTreeMap<ViewId, Vec<exact_plan::EventKind>> {
        let mut out = BTreeMap::new();
        let mut stack: Vec<_> = self.children.iter().collect();
        while let Some(child) = stack.pop() {
            match child {
                Child::Node(node) => {
                    let handlers = plan.node(node.node).handlers;
                    if handlers.len > 0 {
                        out.insert(
                            node.view,
                            handlers.iter().map(|h| plan.handler(h).event).collect(),
                        );
                    }
                    stack.extend(node.children.iter());
                }
                Child::Region(region) => match &region.active {
                    Active::Arm { roots, .. } => stack.extend(roots.iter()),
                    Active::Rows { rows } => {
                        for row in rows {
                            stack.extend(row.roots.iter());
                        }
                    }
                },
            }
        }
        out
    }

    fn emit_roots(&mut self, u: &mut Update<'_>) {
        let roots = roots_of(&self.children);
        for r in &roots {
            if !self.last_roots.contains(r) {
                u.ops.push(Op::AttachRoot { id: *r });
            }
        }
        self.last_roots = roots;
    }

    /// The current kernel roots.
    pub fn roots(&self) -> Vec<ViewId> {
        roots_of(&self.children)
    }

    /// The site owning `view` and the frames in force there.
    pub fn find(&self, view: ViewId) -> Option<(NodesId, Vec<Frame>)> {
        let mut frames = Vec::new();
        for c in &self.children {
            let found = match c {
                Child::Node(n) => n.find(view, &mut frames),
                Child::Region(r) => r.find(view, &mut frames),
            };
            if let Some(node) = found {
                return Some((node, frames));
            }
        }
        None
    }

    /// The site owning `view` and the instance path to it: the regions
    /// crossed, with the row key or the active arm at each (LLP 1035.002).
    pub fn site(&self, view: ViewId) -> Option<(NodesId, Vec<InstanceStep>)> {
        let mut path = Vec::new();
        for c in &self.children {
            let found = match c {
                Child::Node(n) => n.site(view, &mut path),
                Child::Region(r) => r.site(view, &mut path),
            };
            if let Some(node) = found {
                return Some((node, path));
            }
        }
        None
    }
}

impl NodeInst {
    fn site(&self, view: ViewId, path: &mut Vec<InstanceStep>) -> Option<NodesId> {
        if self.view == view {
            return Some(self.node);
        }
        for c in &self.children {
            let found = match c {
                Child::Node(n) => n.site(view, path),
                Child::Region(r) => r.site(view, path),
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }
}

impl RegionInst {
    fn site(&self, view: ViewId, path: &mut Vec<InstanceStep>) -> Option<NodesId> {
        let walk = |roots: &[Child], path: &mut Vec<InstanceStep>| -> Option<NodesId> {
            for c in roots {
                let found = match c {
                    Child::Node(n) => n.site(view, path),
                    Child::Region(r) => r.site(view, path),
                };
                if found.is_some() {
                    return found;
                }
            }
            None
        };
        match &self.active {
            Active::Arm { roots, arm, .. } => {
                path.push(InstanceStep::Arm {
                    region: self.region,
                    arm: *arm,
                });
                if let Some(found) = walk(roots, path) {
                    return Some(found);
                }
                path.pop();
                None
            }
            Active::Rows { rows } => {
                for r in rows {
                    path.push(InstanceStep::Row {
                        region: self.region,
                        key: r.key.clone(),
                    });
                    if let Some(found) = walk(&r.roots, path) {
                        return Some(found);
                    }
                    path.pop();
                }
                None
            }
        }
    }
}
