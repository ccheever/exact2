//! Which mounted rows have shown in the list's port (LLP 1055 D13).
//!
//! A list mounts rows ahead of its port, so an animation that starts when
//! its node is inserted has run (or finished) before its row arrives. A node
//! with `-exact-animation-trigger: view` waits instead: the kernel holds its
//! animations while the row above it is one this list mounted out of the
//! port, until a report puts the row in it. The list only says which rows
//! those are; whether anything below them waits is the kernel's to know.
use super::{Collection, Mounted};
use crate::instance::Update;
use crate::vm::Frame;
use exact_kernel::PropId;
use exact_plan::{BindingKind, Plan, RegionKind, RegionsId};

/// Rows mounted out of their port and rows that showed, for the commit that
/// carries the batch ([`exact_kernel::Kernel::await_view`],
/// [`exact_kernel::CommitReceipt::revealed`]).
#[derive(Debug, Default)]
pub struct RowsShown {
    /// Wrappers of rows mounted (or bound again) out of the port.
    pub awaiting: Vec<exact_kernel::ViewId>,
    /// Wrappers of rows that waited and now show.
    pub revealed: Vec<exact_kernel::ViewId>,
}

impl RowsShown {
    /// Add another update's rows.
    pub fn extend(&mut self, other: RowsShown) {
        self.awaiting.extend(other.awaiting);
        self.revealed.extend(other.revealed);
    }
}

impl Collection {
    /// Whether the row at `position` overlaps the port the host last
    /// reported. Before any report no row is known to: the first rows are
    /// built before one, and those the first report finds in the port start
    /// with it.
    fn shows(&self, position: usize) -> bool {
        let Some(g) = &self.geometry else {
            return false;
        };
        let start = self.index.prefix(position).unwrap_or(0.0);
        let size = self.index.height(position).unwrap_or(0.0);
        start < g.offset + g.port_main && start + size > g.offset
    }

    /// A row just built or bound to another item: it waits when it is out
    /// of the port, and a row that waited and is bound where it shows no
    /// longer does. Nothing is tracked in a plan without `@keyframes`.
    pub(super) fn mounted_shown(&mut self, u: &mut Update<'_>, mounted: &mut Mounted) {
        if u.env.plan.keyframes.is_empty() {
            return;
        }
        let shows = self.shows(mounted.position);
        if !shows {
            u.shown.awaiting.push(mounted.wrapper);
            self.any_awaiting = true;
        } else if mounted.awaiting {
            u.shown.revealed.push(mounted.wrapper);
        }
        mounted.awaiting = !shows;
    }

    /// The rows that waited and overlap the port were it at `offset`: a host
    /// that leaves its reports for later (it waits for more travel before
    /// it mounts rows) still says where the view is, so what waits for a
    /// row to show starts as it does. Nothing else moves: the window, the
    /// geometry and the rows are the last report's.
    pub(super) fn reveal_at(&mut self, u: &mut Update<'_>, offset: f64) {
        if !self.any_awaiting {
            return;
        }
        let Some(port) = self.geometry.as_ref().map(|g| g.port_main) else {
            return;
        };
        let mut left = false;
        for i in 0..self.mounted.len() {
            if !self.mounted[i].awaiting {
                continue;
            }
            let position = self.mounted[i].position;
            let start = self.index.prefix(position).unwrap_or(0.0);
            let size = self.index.height(position).unwrap_or(0.0);
            if start < offset + port && start + size > offset {
                self.mounted[i].awaiting = false;
                u.shown.revealed.push(self.mounted[i].wrapper);
            } else {
                left = true;
            }
        }
        self.any_awaiting = left;
    }

    /// After a report: the rows that waited and now overlap the port.
    pub(super) fn reveal_shown(&mut self, u: &mut Update<'_>) {
        if !self.any_awaiting {
            return;
        }
        let mut left = false;
        for i in 0..self.mounted.len() {
            if !self.mounted[i].awaiting {
                continue;
            }
            if self.shows(self.mounted[i].position) {
                self.mounted[i].awaiting = false;
                u.shown.revealed.push(self.mounted[i].wrapper);
            } else {
                left = true;
            }
        }
        self.any_awaiting = left;
    }
}

/// Whether this scope is a virtualized list's row: an inner list's (LLP
/// 1070), whose first rows are bounded by its own literal size.
pub(super) fn in_collection_row(plan: &Plan, frames: &[Frame]) -> bool {
    frames.iter().filter_map(|f| f.region).any(|region| {
        let region = plan.region(RegionsId(region));
        region.kind == RegionKind::Each
            && region.parent.is_some_and(|node| {
                plan.node(node)
                    .bindings
                    .iter()
                    .map(|b| plan.binding(b))
                    .any(|b| b.kind == BindingKind::Prop && b.id == PropId::Virtualized as u16)
            })
    })
}
