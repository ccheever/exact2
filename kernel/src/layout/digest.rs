//! The engine's state written out, for tests that something left it untouched
//! (`Kernel::measure_block`'s).
use super::*;

impl LayoutTree {
    /// The engine's state, written out, for a test that something left it
    /// untouched: every node's layout, unrounded layout and dirty bit, and
    /// the offers, invalidations and records the next layout decides by.
    pub(crate) fn state_digest(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let mut nodes: Vec<_> = self.slots.iter().map(|(n, s)| (*s, *n)).collect();
        nodes.sort_by_key(|(s, n)| (*s, u64::from(*n)));
        for (slot, node) in nodes {
            let _ = write!(
                out,
                "{slot}:{:?}|{:?}|{:?};",
                self.taffy.layout(node).ok(),
                self.taffy.unrounded_layout(node),
                self.taffy.dirty(node).ok()
            );
        }
        let mut offers: Vec<_> = self
            .offers
            .iter()
            .map(|(n, o)| (u64::from(*n), format!("{o:?}")))
            .collect();
        offers.sort();
        let mut deferred: Vec<_> = self
            .deferred
            .iter()
            .map(|(n, d)| (u64::from(*n), *d))
            .collect();
        deferred.sort();
        let _ = write!(
            out,
            "offers {offers:?} deferred {deferred:?} buttons {} hoists {} {:?} pass {}",
            self.button_records.len(),
            self.hoists_stale,
            self.absolute,
            self.pass
        );
        out
    }

    /// The engine's whole tree written out: each node's style, layout cache,
    /// saved inputs and measured answers. Some of it is in hash maps, so two
    /// trees compare only as one tree before and after.
    pub(crate) fn engine_digest(&self) -> String {
        format!("{:?}", self.taffy)
    }
}
