//! Which containing block holds each absolutely positioned box that is not
//! its parent's (LLP 1074 T1).
use super::{LayoutTree, MeasureContext};
use crate::id::IdMap;
use taffy::{NodeId, TaffyTree};

impl LayoutTree {
    // @ref LLP 1074 T1 — an absolutely positioned box's containing block is its
    // nearest ancestor that is not `static`, or the top of its tree. The engine
    // lays each box out from that ancestor (vendored Taffy, EXACT PATCH 20) and
    // is told here which ones are not the box's own parent. Every change that
    // can move a box between containing blocks marks the old and the new one
    // dirty already: it restyles or re-parents a box on the path between them.
    pub(super) fn refresh_hoists(&mut self) {
        if !std::mem::take(&mut self.hoists_stale) {
            return;
        }
        let is_static = |taffy: &TaffyTree<MeasureContext>, node| {
            taffy
                .style(node)
                .is_ok_and(|s| s.position == taffy::Position::Static)
        };
        let mut owners: IdMap<NodeId, Vec<NodeId>> = IdMap::default();
        let mut absolutes: Vec<NodeId> = self.absolutes.iter().copied().collect();
        absolutes.sort_unstable_by_key(|node| u64::from(*node));
        std::mem::swap(&mut self.hoist_paths, &mut self.hoist_paths_prior);
        self.hoist_paths.clear();
        for node in absolutes {
            let Some(parent) = self.taffy.parent(node) else {
                continue;
            };
            if !is_static(&self.taffy, parent) {
                continue; // Its parent contains it.
            }
            let mut owner = parent;
            while let Some(above) = self.taffy.parent(owner) {
                self.hoist_paths.insert(owner);
                owner = above;
                if !is_static(&self.taffy, owner) {
                    break;
                }
            }
            owners.entry(owner).or_default().push(node);
        }
        self.taffy.set_hoisted_absolutes(owners);
    }
}
