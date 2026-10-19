//! @ref LLP 1084 D7 — the platform's separator under a grouped list's row:
//! decoration over the row's own ground and under its children, where and
//! in the colour the kernel says (`Kernel::grouped_separator`).

use super::{Backend, Rect4, Shape};
use exact_kernel::{Kernel, NodeRef};
use tiny_skia::Transform;

/// The separator under `node`, drawn in its box `rect` (the box as shown).
pub(super) fn separator(
    backend: &mut dyn Backend,
    kernel: &Kernel,
    node: &NodeRef<'_>,
    rect: Rect4,
    ts: Transform,
    dark: bool,
) {
    let Some(line) = kernel.grouped_separator(node.id) else {
        return;
    };
    let (x, y, ..) = rect;
    let r = line.rect;
    // On whole device pixels, as a browser snaps a box's edge, where the
    // row is drawn upright.
    let (top, height) = if ts.kx == 0.0 && ts.ky == 0.0 && ts.sy > 0.0 {
        let thick = (r.height * ts.sy).round().max(1.0);
        let at = (ts.sy * (y + r.y) + ts.ty).round();
        ((at - ts.ty) / ts.sy, thick / ts.sy)
    } else {
        (y + r.y, r.height)
    };
    backend.fill(
        &Shape::rect((x + r.x, top, r.width, height)),
        super::rgba(line.color.resolve(dark)),
        ts,
    );
}
