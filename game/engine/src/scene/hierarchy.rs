//! Parented global poses: one full resolve (first use, load, cycles) and then
//! incremental propagation that visits only the pages whose Parent or Transform
//! rows were written, recomputing the subtrees under rows that really changed.
use super::{Parent, Transform};
use crate::{Affine3A, Entity, World, PAGE};

const NONE: u32 = u32::MAX;

#[derive(Default)]
pub(crate) struct Hierarchy {
    pub(super) nodes: Vec<Node>,
    pub(super) entities: Vec<Entity>,
    pub(super) path: Vec<Entity>,
    pub(super) broken: Vec<Entity>,
    pub(super) stamp: u32,
    // Incremental state, valid while `ready`: members are nodes with `done == stamp`.
    ready: bool,
    // Storage revisions the links and globals reflect.
    transforms: u64,
    parents: u64,
    links: Vec<Link>,
    // Slots that are members or parents of members; their local poses are cached.
    involved: Vec<u64>,
    members: usize,
    // Members whose Parent names a dead entity (stale parents act as roots).
    stale: usize,
    // Involved slots despawned since the last propagate.
    despawned: Vec<u32>,
    sources: Vec<u32>,
    relinked: Vec<u32>,
    stack: Vec<u32>,
    mark: u32,
    // Per page: the pose epoch at which a propagated global last changed.
    pub(crate) epochs: Vec<u64>,
    pub(crate) epoch: u64,
}
#[derive(Default)]
pub(super) struct Node {
    pub(super) entity: Entity,
    pub(super) parent: Option<Entity>,
    present: u32,
    visiting: u32,
    pub(super) done: u32,
    pub(super) global: Affine3A,
}
#[derive(Clone, Copy)]
struct Link {
    first: u32,
    next: u32,
    prev: u32,
    mark: u32,
    stale: bool,
    local: Option<[u32; 10]>,
}
impl Default for Link {
    fn default() -> Self {
        Self {
            first: NONE,
            next: NONE,
            prev: NONE,
            mark: 0,
            stale: false,
            local: None,
        }
    }
}
fn bits(t: &Transform) -> [u32; 10] {
    let (p, r, s) = (t.position, t.rotation, t.scale);
    [p.x, p.y, p.z, r.x, r.y, r.z, r.w, s.x, s.y, s.z].map(f32::to_bits)
}
fn affine(t: Option<&Transform>) -> Affine3A {
    t.map_or(Affine3A::IDENTITY, |t| t.affine())
}

impl Hierarchy {
    pub(super) fn ready(&self) -> bool {
        self.ready
    }
    #[cfg(test)]
    pub(super) fn force_full(&mut self, stamp: u32) {
        self.ready = false;
        self.stamp = stamp;
    }
    pub(super) fn involves(&self, index: u32) -> bool {
        self.involved
            .get(index as usize / 64)
            .is_some_and(|w| w & (1 << (index % 64)) != 0)
    }
    /// World::despawn reports involved slots before their components leave.
    pub(crate) fn despawning(&mut self, index: u32) {
        if self.ready && self.involves(index) {
            self.despawned.push(index);
        }
    }
    /// `e`'s direct children in entity order, while the links reflect every
    /// Parent row: no Parent written and no involved entity despawned since.
    pub(crate) fn children(&self, w: &World, e: Entity) -> Option<Vec<Entity>> {
        if !self.ready || !self.despawned.is_empty() || w.revision::<Parent>() != self.parents {
            return None;
        }
        let mut out = Vec::new();
        let mut c = self.links.get(e.index() as usize).map_or(NONE, |l| l.first);
        while c != NONE {
            let n = &self.nodes[c as usize];
            if n.parent == Some(e) {
                out.push(n.entity);
            }
            c = self.links[c as usize].next;
        }
        out.sort_by_key(|e| e.index());
        Some(out)
    }
    /// Visit `root` and, after a propagate, its linked descendants.
    pub(crate) fn subtree(&self, root: usize, visit: &mut dyn FnMut(usize)) {
        let mut stack = vec![root as u32];
        while let Some(i) = stack.pop() {
            visit(i as usize);
            let mut c = self.links.get(i as usize).map_or(NONE, |l| l.first);
            while c != NONE {
                stack.push(c);
                c = self.links[c as usize].next;
            }
        }
    }
    fn member(&self, index: usize) -> bool {
        self.nodes.get(index).is_some_and(|n| n.done == self.stamp)
    }
    fn member_entity(&self, e: Entity) -> bool {
        self.nodes
            .get(e.index() as usize)
            .is_some_and(|n| n.done == self.stamp && n.entity == e)
    }

    pub(super) fn resolve(&mut self, w: &World, reject: bool) -> Result<(), crate::DataError> {
        self.ready = false;
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.nodes.clear();
            self.stamp = 1;
        }
        let stamp = self.stamp;
        self.entities.clear();
        self.broken.clear();
        // Resolution runs under `&mut World`: one whole-column hold replaces a row
        // lease per node. @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
        let poses = w.pages::<Transform>();
        let local = |e: Entity| affine(poses.row(e.index() as usize));
        for (e, p) in w.query::<&Parent>().iter() {
            let i = e.index() as usize;
            if i >= self.nodes.len() {
                self.nodes.resize_with(i + 1, Node::default);
            }
            let n = &mut self.nodes[i];
            n.entity = e;
            n.parent = w.contains(p.0).then_some(p.0);
            n.present = stamp;
            self.entities.push(e);
        }
        for i in 0..self.entities.len() {
            let start = self.entities[i];
            if self.nodes[start.index() as usize].done == stamp {
                continue;
            }
            self.path.clear();
            let mut e = start;
            let mut base = loop {
                let Some(n) = self
                    .nodes
                    .get_mut(e.index() as usize)
                    .filter(|n| n.present == stamp)
                else {
                    break local(e);
                };
                if n.done == stamp {
                    break n.global;
                }
                if n.visiting == stamp {
                    let begin = self.path.iter().position(|&p| p == e).unwrap();
                    let root = *self.path[begin..].iter().max_by_key(|e| e.index()).unwrap();
                    if reject {
                        return Err(crate::DataError::new(format!(
                            "Parent cycle at #{}",
                            root.index()
                        )));
                    }
                    self.nodes[root.index() as usize].parent = None;
                    self.broken.push(root);
                    for e in self.path.drain(..) {
                        self.nodes[e.index() as usize].visiting = 0;
                    }
                    e = start;
                    continue;
                }
                n.visiting = stamp;
                self.path.push(e);
                if let Some(parent) = n.parent {
                    e = parent;
                } else {
                    break Affine3A::IDENTITY;
                }
            };
            while let Some(e) = self.path.pop() {
                base *= local(e);
                let n = &mut self.nodes[e.index() as usize];
                n.global = base;
                n.done = stamp;
            }
        }
        Ok(())
    }

    /// Index the full resolve for incremental propagation. Broken cycle edges
    /// have already lost their Parent component and leave membership.
    pub(super) fn rebuild(&mut self, w: &World) {
        for &e in &self.broken {
            self.nodes[e.index() as usize].done = 0;
        }
        let slots = w.alive_mask.len() * 64;
        self.links.clear();
        self.links
            .resize(slots.max(self.nodes.len()), Link::default());
        self.involved.clear();
        self.involved.resize(self.links.len().div_ceil(64), 0);
        self.despawned.clear();
        self.members = 0;
        self.stale = 0;
        self.epoch += 1;
        // A cycle-broken root is a root now: its global changed too.
        for k in 0..self.broken.len() {
            let i = self.broken[k].index() as usize;
            self.mark_page(i);
        }
        let poses = w.pages::<Transform>();
        for k in 0..self.entities.len() {
            let i = self.entities[k].index() as usize;
            if !self.member(i) {
                continue;
            }
            self.members += 1;
            self.involve(i, poses.row(i));
            self.mark_page(i);
            match self.nodes[i].parent {
                Some(p) => {
                    let p = p.index() as usize;
                    self.link(p, i);
                    self.involve(p, poses.row(p));
                }
                None => {
                    self.links[i].stale = true;
                    self.stale += 1;
                }
            }
        }
        self.transforms = w.revision::<Transform>();
        self.parents = w.revision::<Parent>();
        self.ready = true;
    }
    fn grow(&mut self, index: usize) {
        if index >= self.links.len() {
            self.links.resize(index + 1, Link::default());
            self.involved.resize((index + 1).div_ceil(64), 0);
        }
        if index >= self.nodes.len() {
            self.nodes.resize_with(index + 1, Node::default);
        }
    }
    fn involve(&mut self, i: usize, local: Option<&Transform>) {
        if self.involved[i / 64] & (1 << (i % 64)) == 0 {
            self.involved[i / 64] |= 1 << (i % 64);
            self.links[i].local = local.map(bits);
        }
    }
    fn release(&mut self, i: usize) {
        if !self.member(i) && self.links[i].first == NONE {
            self.involved[i / 64] &= !(1 << (i % 64));
            self.links[i].local = None;
        }
    }
    fn link(&mut self, parent: usize, child: usize) {
        let first = self.links[parent].first;
        self.links[child].prev = NONE;
        self.links[child].next = first;
        if first != NONE {
            self.links[first as usize].prev = child as u32;
        }
        self.links[parent].first = child as u32;
    }
    fn unlink(&mut self, child: usize) {
        let Some(parent) = self.nodes[child].parent else {
            return;
        };
        let parent = parent.index() as usize;
        let Link { prev, next, .. } = self.links[child];
        if prev == NONE {
            self.links[parent].first = next;
        } else {
            self.links[prev as usize].next = next;
        }
        if next != NONE {
            self.links[next as usize].prev = prev;
        }
        self.links[child].prev = NONE;
        self.links[child].next = NONE;
        self.nodes[child].parent = None;
        self.release(parent);
    }
    fn set_stale(&mut self, i: usize, stale: bool) {
        if self.links[i].stale != stale {
            self.links[i].stale = stale;
            if stale {
                self.stale += 1;
            } else {
                self.stale -= 1;
            }
        }
    }
    fn source(&mut self, i: usize) {
        if self.links[i].mark != self.mark {
            self.links[i].mark = self.mark;
            self.sources.push(i as u32);
        }
    }
    fn mark_page(&mut self, i: usize) {
        let page = i / PAGE;
        if page >= self.epochs.len() {
            self.epochs.resize(page + 1, 0);
        }
        self.epochs[page] = self.epoch;
    }
    fn next_mark(&mut self) {
        self.mark = self.mark.wrapping_add(1);
        if self.mark == 0 {
            for link in &mut self.links {
                link.mark = 0;
            }
            self.mark = 1;
        }
    }

    /// Orphans (living Parent rows naming a dead entity), in entity order, found
    /// from the slots despawned and the Parent pages written since the last
    /// propagate. None when only a full scan can know (not ready, or stale roots).
    pub(super) fn orphans(&self, w: &World, from: usize, out: &mut Vec<Entity>) -> Option<usize> {
        if !self.ready || self.stale != 0 {
            return None;
        }
        out.clear();
        let Some(storage) = w.storage::<Parent>() else {
            return Some(self.despawned.len());
        };
        let rows = w.pages::<Parent>();
        let orphan = |i: usize| rows.row(i).is_some_and(|p| !w.contains(p.0));
        for &d in &self.despawned[from..] {
            let mut c = self.links.get(d as usize).map_or(NONE, |l| l.first);
            while c != NONE {
                if orphan(c as usize) {
                    out.push(w.entity_at(c as usize));
                }
                c = self.links[c as usize].next;
            }
        }
        for page in storage.changed_pages(self.parents) {
            for i in page * PAGE..(page + 1) * PAGE {
                if orphan(i) {
                    out.push(w.entity_at(i));
                }
            }
        }
        out.sort_by_key(|e| e.index());
        out.dedup();
        Some(self.despawned.len())
    }

    /// Bring links and globals up to date. False asks for a full resolve: an
    /// edited edge may close a cycle, whose repair order the full resolve owns.
    pub(super) fn update(&mut self, w: &World) -> bool {
        let (transforms, parents) = (w.revision::<Transform>(), w.revision::<Parent>());
        if transforms == self.transforms && parents == self.parents && self.despawned.is_empty() {
            return true;
        }
        self.next_mark();
        self.epoch += 1;
        self.sources.clear();
        self.relinked.clear();
        let poses = w.pages::<Transform>();
        if let Some(storage) = w.storage::<Parent>().filter(|_| parents != self.parents) {
            let rows = w.pages::<Parent>();
            for page in storage.changed_pages(self.parents) {
                for i in page * PAGE..(page + 1) * PAGE {
                    let member = self.member(i);
                    let Some(target) = rows.row(i).map(|p| p.0) else {
                        if member {
                            // Its global is its local pose now, whether or not
                            // it parents anything.
                            self.mark_page(i);
                            self.unlink(i);
                            self.set_stale(i, false);
                            self.nodes[i].done = 0;
                            self.members -= 1;
                            self.release(i);
                            if self.involves(i as u32) {
                                self.source(i);
                            }
                        }
                        continue;
                    };
                    let e = w.entity_at(i);
                    let parent = w.contains(target).then_some(target);
                    if member && self.nodes[i].entity == e && self.nodes[i].parent == parent {
                        continue;
                    }
                    self.grow(i);
                    if member {
                        self.unlink(i);
                    } else {
                        self.members += 1;
                    }
                    let n = &mut self.nodes[i];
                    n.entity = e;
                    n.parent = parent;
                    n.done = self.stamp;
                    self.involve(i, poses.row(i));
                    self.set_stale(i, parent.is_none());
                    if let Some(p) = parent {
                        let p = p.index() as usize;
                        self.grow(p);
                        self.link(p, i);
                        self.involve(p, poses.row(p));
                    }
                    self.source(i);
                    self.relinked.push(i as u32);
                }
            }
        }
        // A despawned parent leaves its remaining children as stale roots.
        for k in 0..self.despawned.len() {
            let d = self.despawned[k] as usize;
            let mut c = self.links[d].first;
            while c != NONE {
                let next = self.links[c as usize].next;
                let child = c as usize;
                if self.nodes[child].parent.is_some_and(|p| !w.contains(p)) {
                    self.unlink(child);
                    self.set_stale(child, true);
                    self.source(child);
                }
                c = next;
            }
            self.release(d);
            if self.involves(d as u32) {
                self.links[d].local = poses.row(d).map(bits);
            }
        }
        self.despawned.clear();
        for k in 0..self.relinked.len() {
            let start = self.relinked[k] as usize;
            let mut at = start;
            for _ in 0..=self.members {
                match self.nodes[at].parent {
                    Some(p) if self.member_entity(p) => {
                        at = p.index() as usize;
                        if at == start {
                            return false;
                        }
                    }
                    _ => break,
                }
            }
            if self.nodes[at].parent.is_some_and(|p| self.member_entity(p)) {
                return false; // A cycle above, not through, this edge.
            }
        }
        if let Some(storage) = w
            .storage::<Transform>()
            .filter(|_| transforms != self.transforms)
        {
            for page in storage.changed_pages(self.transforms) {
                let words = page * PAGE / 64..((page + 1) * PAGE / 64).min(self.involved.len());
                for word in words {
                    let mut set = self.involved[word];
                    while set != 0 {
                        let i = word * 64 + set.trailing_zeros() as usize;
                        set &= set - 1;
                        let now = poses.row(i).map(bits);
                        if now != self.links[i].local {
                            self.links[i].local = now;
                            self.source(i);
                        }
                    }
                }
            }
        }
        for k in 0..self.sources.len() {
            let s = self.sources[k] as usize;
            // An ancestor already queued recomputes this subtree.
            let mut at = s;
            let mut covered = false;
            while let Some(p) = self.member(at).then(|| self.nodes[at].parent).flatten() {
                at = p.index() as usize;
                if self.links[at].mark == self.mark {
                    covered = true;
                    break;
                }
            }
            if covered {
                continue;
            }
            self.stack.clear();
            if self.member(s) {
                self.stack.push(s as u32);
            } else {
                let mut c = self.links[s].first;
                while c != NONE {
                    self.stack.push(c);
                    c = self.links[c as usize].next;
                }
            }
            while let Some(i) = self.stack.pop() {
                let i = i as usize;
                let base = match self.nodes[i].parent {
                    Some(p) if self.member_entity(p) => self.nodes[p.index() as usize].global,
                    Some(p) => affine(poses.row(p.index() as usize)),
                    None => Affine3A::IDENTITY,
                };
                self.nodes[i].global = base * affine(poses.row(i));
                self.mark_page(i);
                let mut c = self.links[i].first;
                while c != NONE {
                    self.stack.push(c);
                    c = self.links[c as usize].next;
                }
            }
        }
        self.transforms = transforms;
        self.parents = parents;
        true
    }
}
