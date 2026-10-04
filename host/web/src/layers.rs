//! Live paint isolation uses the kernel's sibling decisions (LLP 1083.000).
//! Dirty lists run deepest first; changed potentials propagate to the parent.

use exact_kernel::id::IdMap;
use exact_kernel::paint_order::{self, Child};
use exact_kernel::{PositionType, StyleId, ViewId};
use exact_runner::DataSource;

use super::Host;
use crate::batch::Batch;

/// `css` with the page's `isolation` when the node takes it.
pub fn with_isolation(mut css: String, isolated: bool) -> String {
    if isolated {
        css.push_str("isolation:isolate;");
    }
    css
}

/// Every live node's kernel paint record, by view.
#[derive(Default)]
pub(crate) struct Layers(IdMap<ViewId, Child>);

impl Layers {
    pub(crate) fn isolated(&self, id: ViewId) -> bool {
        self.0.get(&id).is_some_and(|f| f.isolated || f.own.policy)
    }

    pub(crate) fn forget(&mut self, id: ViewId) {
        self.0.remove(&id);
    }
}

impl<D: DataSource> Host<D> {
    /// Before a batch's creates and updates: the paint of every node it
    /// creates or updates (`changed`, the final tree's views), then each
    /// sibling list one of them may move, deepest first, so a parent reads
    /// its children's final facts. A node the batch neither creates nor
    /// updates whose isolation moved is restyled here; the rest take theirs
    /// in `create` and `update`.
    pub(super) fn relayer(&mut self, changed: &[ViewId], batch: &mut Batch) {
        let kernel = self.runner.kernel();
        // A sibling exclusion is one fact per list, not a sibling scan per
        // child (a theme update can touch thousands of children at once).
        let mut exclusions = std::collections::HashMap::new();
        let mut own = |id| {
            let node = kernel.node(id).expect("live node");
            let beside_exclusion = node.parent.is_some_and(|parent| {
                *exclusions.entry(parent).or_insert_with(|| {
                    let parent = kernel.node(parent).expect("live parent");
                    kernel.arena().children(parent.key.index).iter().any(|&c| {
                        let style = kernel.arena().style(c);
                        style.position_type == PositionType::Absolute
                            && style.wrap_flow == exact_kernel::WrapFlow::Both
                    })
                })
            });
            paint_order::own_from(paint_order::Facts {
                style: node.style,
                props: node.props,
                kind: node.node_type,
                root: node.is_root,
                parent_display: node
                    .parent
                    .and_then(|p| kernel.node(p))
                    .map(|p| p.style.display),
                beside_exclusion,
                holds_layout_transition: kernel
                    .arena()
                    .children(node.key.index)
                    .iter()
                    .any(|&c| kernel.arena().style(c).mask.has(StyleId::LayoutTransition)),
            })
        };
        let mut lists = std::collections::BinaryHeap::new();
        let mut queued = std::collections::HashSet::new();
        let depth = |id: Option<ViewId>| {
            let mut n = 0u32;
            let mut at = id.and_then(|id| kernel.node(id));
            while let Some(node) = at {
                n += 1;
                at = node.parent.and_then(|p| kernel.node(p));
            }
            n
        };
        let mut queue = |id: Option<ViewId>, lists: &mut std::collections::BinaryHeap<_>| {
            if queued.insert(id) {
                lists.push((depth(id), id));
            }
        };
        for id in changed {
            let Some(node) = kernel.node(*id) else {
                continue;
            };
            // Children, parent display and structural facts can change even
            // when this node's own stacking values have not changed.
            queue(Some(*id), &mut lists);
            queue(node.parent, &mut lists);
        }
        let changed: std::collections::HashSet<ViewId> = changed.iter().copied().collect();
        let mut restyle = Vec::new();
        while let Some((_, list)) = lists.pop() {
            let children = match list {
                Some(id) => kernel.node(id).map(|n| n.children()).unwrap_or_default(),
                None => self.page_roots(),
            };
            let records: Vec<_> = children
                .iter()
                .map(|id| {
                    let own = own(*id);
                    let potentials = self
                        .layers
                        .0
                        .get(id)
                        .map(|f| f.potentials)
                        .unwrap_or_default();
                    (own, potentials)
                })
                .collect();
            // Roots are independent stacking contexts, not one sibling list.
            let isolation = if list.is_some() {
                paint_order::decide(&records)
            } else {
                vec![false; children.len()]
            };
            let mut decided = Vec::with_capacity(children.len());
            for ((child, (own, potentials)), isolated) in
                children.into_iter().zip(records).zip(isolation)
            {
                let was = self.layers.isolated(child);
                let facts = Child {
                    own,
                    isolated,
                    potentials,
                };
                self.layers.0.insert(child, facts);
                if was != self.layers.isolated(child) && !changed.contains(&child) {
                    restyle.push(child);
                }
                decided.push(facts);
            }
            let Some(id) = list else { continue };
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let potentials = paint_order::potentials(&decided);
            let facts = self.layers.0.entry(id).or_default();
            if facts.potentials != potentials || facts.own != own(id) {
                facts.potentials = potentials;
                queue(node.parent, &mut lists);
            }
        }
        for id in restyle {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let css = self.view_css(&node);
            let Some(m) = self.mirror.get_mut(&id) else {
                continue;
            };
            if css != m.css {
                batch.style(id, &css);
                m.css = css;
            }
        }
    }
}
