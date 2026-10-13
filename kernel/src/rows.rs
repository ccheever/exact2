//! @ref LLP 1115.000 D16 — a list's rows, for a host that draws them itself.
//!
//! Contract decides which lists' rows are cells and marks each (`listRows`,
//! `contract/lower/src/cells.rs`); a row is a child of such a list. What a
//! row then is belongs to its platform, and what its author wrote on it
//! still wins. This is the part every painted host reads the same way, so
//! it is said once: which node is a row, and whether the platform's
//! separator belongs under it. The web's sheet has to say the same in CSS
//! (`host/web/neutral.css`), since a page keeps no kernel: the same six
//! rows are held to the same answers for this (`contract/cli/tests/it/
//! list_rows.rs`) and for the sheet (`host/web/tests/neutral-look.test.mjs`).

use crate::generated::{NodeType, PropId, StyleId};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};

/// The node whose rows say what a row's author wrote: the row itself, or,
/// for the wrapper the runner puts round a windowed list's row (it carries
/// `listItemKey`), the row inside it.
fn authored<'a>(kernel: &'a Kernel, row: &NodeRef<'a>) -> Option<NodeRef<'a>> {
    if row.props.str(PropId::ListItemKey).is_none() {
        return kernel.node(row.id);
    }
    row.children().first().and_then(|c| kernel.node(*c))
}

fn wrote(node: &NodeRef<'_>, width: StyleId, line: StyleId) -> bool {
    node.style.written.has(width) || node.style.written.has(line)
}

impl Kernel {
    /// The list `id` is a row of: its parent, when that is a `list` whose
    /// rows are cells.
    pub fn row_list(&self, id: ViewId) -> Option<ViewId> {
        let parent = self.node(self.node(id)?.parent?)?;
        (parent.node_type == NodeType::List && parent.props.bool(PropId::ListRows) == Some(true))
            .then_some(parent.id)
    }

    /// The row `id` is in, itself included: the nearest node from it up
    /// that is a row of a list whose rows are cells. What presses inside a
    /// row answers as the row's platform does (a cell's highlight, the
    /// neutral look's wash), and never with the dim of a pressable that is
    /// no row.
    pub fn row_holding(&self, id: ViewId) -> Option<ViewId> {
        let mut at = self.node(id)?;
        loop {
            if self.row_list(at.id).is_some() {
                return Some(at.id);
            }
            at = self.node(at.parent?)?;
        }
    }

    /// Whether the platform's separator belongs under the row `id`: it is a
    /// row with a row after it; it is no spacer of a windowed list (hidden
    /// from assistive technology, as an author's own decorative row is);
    /// and its author wrote neither its bottom edge nor the next row's top
    /// edge (a width or a style, `none` included: the author's separator
    /// wins, and so does the author's "no separator here"), nor a
    /// `box-shadow` on it, which is where the web's sheet draws the line.
    pub fn row_separator(&self, id: ViewId) -> bool {
        let Some(list) = self.row_list(id).and_then(|l| self.node(l)) else {
            return false;
        };
        let rows = list.children();
        let Some(at) = rows.iter().position(|r| *r == id) else {
            return false;
        };
        let (Some(row), Some(next)) = (self.node(id), rows.get(at + 1).and_then(|n| self.node(*n)))
        else {
            return false;
        };
        if row.props.bool(PropId::AccessibilityElementsHidden) == Some(true)
            || row.style.written.has(StyleId::BoxShadow)
        {
            return false;
        }
        let own = authored(self, &row)
            .is_some_and(|n| wrote(&n, StyleId::BorderWidthBottom, StyleId::BorderStyleBottom));
        let after = authored(self, &next)
            .is_some_and(|n| wrote(&n, StyleId::BorderWidthTop, StyleId::BorderStyleTop));
        !own && !after
    }
}
