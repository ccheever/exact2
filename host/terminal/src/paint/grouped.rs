//! @ref LLP 1084 D7 — the platform's separator under a grouped list's row,
//! where the kernel places it (`Kernel::grouped_separator`): a light rule
//! on the row's last line, from its content edge, drawn whatever its
//! borders are, under the cells its children paint.

use super::{rgb, snap, Painted, Scene};
use crate::grid::{CellRect, Style};
use exact_kernel::style::cells::{COLUMN, ROW};
use exact_kernel::NodeRef;

/// The rule under `n`, whose frame is offset by `dx`, `dy` on the screen.
pub(super) fn separator(
    scene: &Scene<'_>,
    out: &mut Painted,
    n: &NodeRef<'_>,
    clip: CellRect,
    dx: f32,
    dy: f32,
) {
    let Some(line) = scene.kernel.grouped_separator(n.id) else {
        return;
    };
    let (left, top) = (n.frame.x - dx + line.rect.x, n.frame.y - dy + line.rect.y);
    let (x0, x1) = (snap(left, COLUMN), snap(left + line.rect.width, COLUMN));
    let y = snap(top + line.rect.height, ROW) - 1;
    let style = Style {
        fg: rgb(line.color.resolve(scene.dark)),
        ..Style::default()
    };
    for x in x0..x1 {
        out.grid.put(x, y, "─", 1, style, clip);
    }
}
