//! Grouped rows have a decorative rule, independent of authored borders.
use super::*;

pub(super) fn separator(
    scene: &Scene<'_>,
    out: &mut Painted,
    n: &NodeRef<'_>,
    clip: CellRect,
    dx: f32,
    dy: f32,
) {
    if !scene.kernel.grouped_row_separator(n.id) {
        return;
    }
    let f = n.frame;
    let [bt, br, bb, bl] = n.style.border_widths_in(&scene.env);
    let (padding, _, _, _) = scene.kernel.resolved_padding(n.key).unwrap_or_default();
    let inset = if n.style.mask.has(exact_kernel::StyleId::PaddingLeft) {
        padding
    } else {
        16.0
    };
    let rect = cells(
        f.x - dx + bl + inset.max(0.0),
        f.y - dy + bt,
        (f.width - bl - br - inset.max(0.0)).max(0.0),
        (f.height - bt - bb).max(0.0),
    );
    let ink = if n.style.mask.has(exact_kernel::StyleId::BorderColorBottom) {
        n.style.border_colors(n.text_color())[2].resolve(scene.dark)
    } else if scene.dark {
        Color::rgba(84, 84, 88, 128)
    } else {
        Color::rgba(60, 60, 67, 31)
    };
    if rect.h <= 0 {
        return;
    }
    let style = Style {
        fg: rgb(ink),
        ..Style::default()
    };
    for x in rect.x..rect.x + rect.w {
        out.grid.put(x, rect.y + rect.h - 1, "─", 1, style, clip);
    }
}
