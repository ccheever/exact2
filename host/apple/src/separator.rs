//! @ref LLP 1084 D7 — the separator under a grouped list's row, as the
//! presenter's derived prop `groupedSeparator`: where the kernel places it
//! (`Kernel::grouped_separator`) and its colour as a style row sends one.
//! iOS draws its grouped list as UIKit's own, whose separator is UIKit's,
//! so it is sent there for no row; tvOS and the Mac draw the sheet.

use super::*;

/// The derived prop's name.
pub(crate) const PROP: &str = "groupedSeparator";

/// `{"rect":[x,y,w,h],"color":…}` in the row's own space; `None` where the
/// row has none.
pub(crate) fn json(kernel: &exact_kernel::Kernel, id: ViewId) -> Option<String> {
    if cfg!(target_os = "ios") {
        return None;
    }
    let line = kernel.grouped_separator(id)?;
    let r = line.rect;
    let mut out = format!(
        "{{\"rect\":[{},{},{},{}],\"color\":",
        r.x, r.y, r.width, r.height
    );
    crate::style::push_color_value(&mut out, line.color);
    out.push('}');
    Some(out)
}

impl<D: DataSource> Host<D> {
    /// Sends the separators a change to `id` may move without moving any
    /// box: those of every row of the card that holds it (a row's own edge,
    /// or the next row's top edge, decides the one above it), `id` being a
    /// row or a `delete=` row's content.
    pub(crate) fn emit_separators_beside(&mut self, id: ViewId, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let row = |id: ViewId| {
            kernel
                .node(id)
                .filter(|n| n.props.bool(PropId::GroupedRowSeparator) == Some(true))
        };
        let parent = |id: ViewId| kernel.node(id).and_then(|n| n.parent);
        let card = row(id)
            .and_then(|r| r.parent)
            .or_else(|| parent(id).and_then(row).and_then(|r| r.parent));
        if let Some(card) = card {
            self.emit_separators(card, batch);
        }
    }

    /// Sends `id`'s separator where it moved, and those of the grouped rows
    /// it holds: a row's own layout moves its line, and a group's says
    /// which of its rows is now the last.
    pub(crate) fn emit_separators(&mut self, id: ViewId, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let Some(node) = kernel.node(id) else {
            return;
        };
        let mut rows: Vec<ViewId> = node
            .children()
            .into_iter()
            .filter(|c| {
                kernel
                    .node(*c)
                    .is_some_and(|n| n.props.bool(PropId::GroupedRowSeparator) == Some(true))
            })
            .collect();
        if node.props.bool(PropId::GroupedRowSeparator) == Some(true) {
            rows.push(id);
        }
        for row in rows {
            let line = json(self.runner.kernel(), row);
            let Some(m) = self.mirror.get_mut(&row) else {
                continue;
            };
            if m.props.get(PROP) == line.as_ref() {
                continue;
            }
            match line {
                Some(line) => {
                    batch.props(row, &[(PROP, line.clone())], &[]);
                    m.props.insert(PROP, line);
                }
                None => {
                    batch.props(row, &[], &[PROP]);
                    m.props.remove(PROP);
                }
            }
        }
    }
}
