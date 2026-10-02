//! SVG references by id (LLP 1055.000 D3).

use crate::id::ViewId;
use crate::kernel::Kernel;

impl Kernel {
    /// The node an SVG reference `#id` from `from` names (LLP 1055.000 D3):
    /// the one live node with that `id`; with several, the one sharing the
    /// deepest common ancestor with `from`, ties in tree order, so a
    /// component's instance finds its own definitions.
    pub fn resolve_id(&self, from: ViewId, id: &str) -> Option<ViewId> {
        let candidates = self.selectors.lookup_id(id);
        let live: Vec<u32> = candidates
            .iter()
            .copied()
            .filter(|s| self.arena().is_live(*s))
            .collect();
        match live.as_slice() {
            [] => None,
            [one] => Some(self.arena().local_id(*one)),
            _ => {
                let arena = self.arena();
                nearest(
                    &live,
                    arena.slot_of(from)?,
                    |slot| arena.parent(slot),
                    |parent, child| {
                        arena
                            .children(parent)
                            .iter()
                            .position(|s| *s == child)
                            .unwrap_or(usize::MAX)
                    },
                )
                .map(|slot| arena.local_id(slot))
            }
        }
    }
}

/// Of several nodes an id names (`candidates`), the one sharing the deepest
/// common ancestor with `from`, ties in tree order (each step's place among
/// its parent's children), then by key: [`Kernel::resolve_id`]'s rule, over
/// any tree that answers `parent` and `position` — the kernel's, or a
/// document a render writes without one (LLP 1048.004).
pub fn nearest<K: Copy + Ord>(
    candidates: &[K],
    from: K,
    parent: impl Fn(K) -> Option<K>,
    position: impl Fn(K, K) -> usize,
) -> Option<K> {
    let chain = |node: K| {
        let mut out = vec![node];
        let mut cur = parent(node);
        while let Some(p) = cur {
            out.push(p);
            cur = parent(p);
        }
        out.reverse();
        out
    };
    let from = chain(from);
    let order = |c: &[K]| -> Vec<usize> { c.windows(2).map(|w| position(w[0], w[1])).collect() };
    candidates
        .iter()
        .map(|node| {
            let c = chain(*node);
            let shared = c.iter().zip(&from).take_while(|(a, b)| a == b).count();
            (std::cmp::Reverse(shared), order(&c), *node)
        })
        .min()
        .map(|(_, _, node)| node)
}
