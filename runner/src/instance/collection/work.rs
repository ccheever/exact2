//! Measured virtual-row operations, attributed to the owning collection site.
//! @ref LLP 1142 §13.4 — physical work is counted even if its commit is refused.
use super::*;

impl Collection {
    pub(super) fn count_row(
        &self,
        u: &mut Update<'_>,
        count: impl FnOnce(&mut crate::perf::SiteWork),
    ) {
        if let Some(site) = u.ids.site(self.view) {
            if let Some(w) = u.ids.work.sites.get_mut(site.0 as usize) {
                count(w);
            }
        }
    }

    pub(super) fn retire_row(&self, u: &mut Update<'_>, row: &Mounted) {
        if u.ids.work.sites.is_empty() {
            return;
        }
        self.count_row(u, |w| w.row_retired += 1);
        retire_children(u, &row.row.roots);
    }
}

pub(in crate::instance) fn retire_node(u: &mut Update<'_>, node: &NodeInst) {
    if u.ids.work.sites.is_empty() {
        return;
    }
    if let Some(c) = node.collection() {
        for row in &c.mounted {
            c.retire_row(u, row);
        }
    }
    retire_children(u, &node.children);
}

fn retire_children(u: &mut Update<'_>, children: &[Child]) {
    for child in children {
        match child {
            Child::Node(n) => retire_node(u, n),
            Child::Region(r) => match &r.active {
                Active::Arm { roots, .. } => retire_children(u, roots),
                Active::Rows { rows } => {
                    for row in rows {
                        retire_children(u, &row.roots);
                    }
                }
            },
        }
    }
}
