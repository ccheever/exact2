//! What a change of inputs would mount, found without changing the tree.
//!
//! A host warms its caches (shaped text, mostly) for a screen a press is
//! about to open: the press's action is evaluated, its writes are laid over
//! the slots, and [`Tree::foresee`] walks the tree as an update would, but
//! reads only. Where a `when`/`match` would show another arm, that arm is
//! realized into the update's ops with ids of its own ([`Ids::after`]) and
//! dropped; nothing is destroyed, no instance is kept, and the tree, its
//! `seen` inputs and the runner's ids are as they were. The ops are for a
//! scratch kernel, never the runner's.
//!
//! It is a guess, and may be short: derives are not settled over the
//! writes, an `each` is walked as last keyed, a virtualized list is not
//! entered. A miss costs what no warm costs.

use super::region::init_owned;
use super::*;

/// An arm a what-if would show.
#[derive(Debug, Clone, PartialEq)]
pub struct Mount {
    /// The live node it would be mounted under.
    pub parent: ViewId,
    /// Its root views, created by the update's ops.
    pub roots: Vec<ViewId>,
}

impl Ids {
    /// An allocator for views that are never committed, numbered after every
    /// view `live` has given out: none is a live view's id.
    pub fn after(live: &Ids) -> Ids {
        Ids {
            next: live.next,
            ..Default::default()
        }
    }
}

impl Tree {
    /// The arms `u.env` would newly show, each realized into `u.ops`.
    pub fn foresee(&self, u: &mut Update<'_>) -> Result<Vec<Mount>, InstanceError> {
        u.changed = Some(u.sites.deps.changed(&self.seen, &u.env));
        let mut out = Vec::new();
        let walked = walk(u, &self.children, &[], None, &mut out);
        u.changed = None;
        walked.map(|()| out)
    }
}

fn walk(
    u: &mut Update<'_>,
    children: &[Child],
    frames: &[Frame],
    parent: Option<ViewId>,
    out: &mut Vec<Mount>,
) -> Result<(), InstanceError> {
    let sites = u.sites;
    for c in children {
        match c {
            Child::Node(n) => {
                if n.collection.is_none() && u.stale(&sites.deps.nodes[n.node.0 as usize]) {
                    walk(u, &n.children, frames, Some(n.view), out)?;
                }
            }
            Child::Region(r) => {
                if u.stale(&sites.deps.regions[r.region.0 as usize]) {
                    region(u, r, frames, parent, out)?;
                }
            }
        }
    }
    Ok(())
}

fn region(
    u: &mut Update<'_>,
    r: &RegionInst,
    frames: &[Frame],
    parent: Option<ViewId>,
    out: &mut Vec<Mount>,
) -> Result<(), InstanceError> {
    let plan = u.env.plan;
    let sites = u.sites;
    let index = r.region.0 as usize;
    let row = plan.region(r.region);
    let kind_error = InstanceError::SubjectKind { region: r.region };
    match &r.active {
        Active::Rows { rows } => {
            let body = &sites.deps.bodies[index];
            for row in rows {
                let saved = u.enter(0, Some(&row.slots));
                let walked = if u.stale(body) {
                    walk(
                        u,
                        &row.roots,
                        &with_frame(frames, row.frame.clone()),
                        parent,
                        out,
                    )
                } else {
                    Ok(())
                };
                u.leave(saved);
                walked?;
            }
            Ok(())
        }
        Active::Arm { arm, frame, roots } => {
            let (want, mut new_frame) = if !u.stale(&sites.deps.subjects[index]) {
                (*arm, frame.clone())
            } else {
                match (&row.kind, u.eval(row.subject, frames)?) {
                    (RegionKind::When, Value::Bool(true)) => (Some(0), Frame::default()),
                    (RegionKind::When, Value::Bool(false)) => {
                        ((row.arms.len > 1).then_some(1), Frame::default())
                    }
                    (RegionKind::Match, Value::Option(Some(v))) => (
                        Some(0),
                        Frame {
                            item: None,
                            bound: Some((*v).clone()),
                            ..Default::default()
                        },
                    ),
                    (RegionKind::Match, Value::Option(None)) => {
                        ((row.arms.len > 1).then_some(1), Frame::default())
                    }
                    _ => return Err(kind_error),
                }
            };
            if *arm == want {
                // The arm stays: what is inside it may still switch.
                let dirty = crate::compare::changed_fields(&frame.bound, &new_frame.bound);
                let mut inner = frame.clone();
                if dirty != 0 {
                    inner.bound = new_frame.bound;
                }
                let saved = u.enter(dirty, frame.row.as_ref());
                let walked = walk(u, roots, &with_frame(frames, inner), parent, out);
                u.leave(saved);
                return walked;
            }
            let (Some(i), Some(parent)) = (want, parent) else {
                return Ok(());
            };
            let arm_id = plan.region(r.region).arms.iter().nth(i);
            if let Some(id) = arm_id.filter(|id| plan.slots.iter().any(|s| s.owner == Some(*id))) {
                let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
                new_frame.region = Some(r.region.0);
                new_frame.row = Some(slots.clone());
                init_owned(u, id, &slots, &with_frame(frames, new_frame.clone()))?;
            }
            let made = realize(u, None, arm_id, &with_frame(frames, new_frame))?;
            let roots = roots_of(&made);
            if !roots.is_empty() {
                out.push(Mount { parent, roots });
            }
            Ok(())
        }
    }
}
