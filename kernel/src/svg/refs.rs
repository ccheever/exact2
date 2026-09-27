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
                let chain = |slot: u32| {
                    let mut out = vec![slot];
                    let mut cur = self.arena().parent(slot);
                    while let Some(p) = cur {
                        out.push(p);
                        cur = self.arena().parent(p);
                    }
                    out.reverse();
                    out
                };
                let from = chain(self.arena().slot_of(from)?);
                let order = |c: &[u32]| -> Vec<usize> {
                    c.windows(2)
                        .map(|w| {
                            self.arena()
                                .children(w[0])
                                .iter()
                                .position(|s| *s == w[1])
                                .unwrap_or(usize::MAX)
                        })
                        .collect()
                };
                live.iter()
                    .map(|s| {
                        let c = chain(*s);
                        let shared = c.iter().zip(&from).take_while(|(a, b)| a == b).count();
                        (std::cmp::Reverse(shared), order(&c), *s)
                    })
                    .min()
                    .map(|(_, _, s)| self.arena().local_id(s))
            }
        }
    }
}
