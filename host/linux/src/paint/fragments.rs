//! @ref LLP 1093 D7, D8, D12 — this host's part in multi-column layout. The
//! kernel publishes every box in a flow at its column's place and, for a box
//! that straddles columns, its union frame and fragments. A paragraph is laid
//! out once, at its unfragmented width, and painted in each fragment at the
//! fragment's offset, clipped to it, so the lines it shows are the ones the
//! kernel cut there. Column rules are rectangles centred in the gaps. A hit
//! or a tap sees fragments, never a union's gap.

use super::*;

impl Painter {
    /// A `Text` node: its runs' backgrounds, decorations and glyphs, once,
    /// or once in each of its fragments.
    pub(super) fn text_node(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        geometry: &BoxGeometry,
        rect: Rect4,
        ts: Transform,
    ) {
        let content = geometry.content;
        let s = node.style;
        let kernel = walk.scene.kernel;
        let frags = kernel.fragments(node.key).unwrap_or(&[]);
        // The width its lines broke at: a fragmented box's own, not its
        // union's; a multi-column `text`'s column width.
        let width = match frags.first() {
            Some(g) if kernel.columns(node.key).is_some() => g.width,
            Some(g) => g.width - (rect.2 - content.2),
            None => content.2,
        };
        // The kernel measures a Text subtree as one paragraph. Inline
        // descendants deliberately have zero frames, not paint boxes.
        let build = || {
            let mut spec = text_spec(&node.computed_style(StyleMask::INHERITED), "");
            spec.runs = node
                .text_runs()
                .iter()
                .map(|run| Run::from_style(&run.text, run.style))
                .collect();
            spec.shown(node.props.str(PropId::Markup) == Some("markdown"))
        };
        let paragraph = if let Some(stamp) = node.paragraph_stamp() {
            // @ref LLP 1043.000 §3 D7 — ordinary text takes the same
            // retained identity path; flow only adds exclusion geometry.
            self.text.borrow_mut().flow_identified(
                &stamp,
                width,
                &node
                    .flow_shapes()
                    .iter()
                    .map(|s| s.translate(-geometry.inset.0, -geometry.inset.1))
                    .collect::<Vec<_>>(),
                self.accepted_text.get(&node.key),
                build,
            )
        } else {
            let spec = build();
            (!spec.is_empty()).then(|| self.text.borrow_mut().paragraph(&spec, Some(width)))
        };
        let Some(paragraph) = paragraph else {
            return;
        };
        let mut palette = Vec::new();
        text_palette(kernel, node, self.dark, &mut palette);
        presented_text_colors(walk, node, &paragraph, &mut palette);
        walk.text.insert(node.key, paragraph.clone());
        // CSS `text-overflow: ellipsis` in a clipping box: an over-wide line
        // ends in "…" (LLP 1053 G5; paint only).
        let shown = (s.text_overflow == exact_kernel::TextOverflow::Ellipsis
            && effective_overflow(node).0 != Overflow::Visible)
            .then(|| paragraph.ellipsized(width))
            .flatten()
            .unwrap_or_else(|| paragraph.clone());
        let mut backgrounds = Vec::new();
        text_backgrounds(kernel, node, None, self.dark, &mut backgrounds);
        let at = |painter: &mut Self, origin: (f32, f32)| {
            // Inline backgrounds, under the glyphs, per line fragment.
            for (r, color) in shown.run_backgrounds(&backgrounds) {
                let shape = Shape::rect((origin.0 + r.0, origin.1 + r.1, r.2, r.3));
                painter.backend.fill(&shape, color, ts);
            }
            painter.text_decorations(kernel, &shown, &palette, origin, ts);
            painter.text_paint(node, kernel, &shown, &palette, origin, rect, ts);
        };
        if frags.is_empty() {
            return at(self, (content.0, content.1));
        }
        for g in frags {
            let clip = Shape::rect((rect.0 + g.x, rect.1 + g.y, g.width, g.height));
            self.backend.push_clip(&clip, ts);
            at(self, (content.0 + g.dx, content.1 + g.dy));
            self.backend.pop_clip();
        }
    }

    /// A multi-column container's `column-rule`: centred in each gap between
    /// two columns that both hold a box, as tall as the columns, under the
    /// columns' contents (D7). A rule wider than its gap moves no column.
    pub(super) fn column_rules(
        &mut self,
        kernel: &Kernel,
        node: &NodeRef<'_>,
        rect: Rect4,
        ts: Transform,
    ) {
        let s = node.style;
        let width = s.column_rule_width;
        if s.column_rule_style != exact_kernel::BorderStyle::Solid || width <= 0.0 {
            return;
        }
        let Some(columns) = kernel.columns(node.key) else {
            return;
        };
        let current = node
            .computed_style(StyleMask::of(StyleId::TextColor))
            .text_color;
        let color = rgba(s.column_rule_color.unwrap_or(current).resolve(self.dark));
        for pair in columns.columns.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            if !(a.holds && b.holds) {
                continue;
            }
            // The gap's middle, whichever side the next column is on.
            let (left, right) = if b.x >= a.x {
                (a.x + a.width, b.x)
            } else {
                (b.x + b.width, a.x)
            };
            let x = rect.0 + (left + right) / 2.0 - width / 2.0;
            let shape = Shape::rect((x, rect.1 + a.y, width, a.height.max(b.height)));
            self.backend.fill(&shape, color, ts);
        }
    }
}

/// Whether a point (viewport points) inside a painted box hits it: a box
/// with fragments only inside one, never in its union's gap (D8).
pub(crate) fn hits(kernel: &Kernel, b: &PaintedBox, x: f32, y: f32) -> bool {
    let Some(node) = kernel.node(b.id) else {
        return true;
    };
    kernel
        .fragments(node.key)
        .is_none_or(|frags| frags.iter().any(|g| g.contains(x - b.rect.0, y - b.rect.1)))
}

/// Where the agent's `tap` aims on a box with fragments: its first
/// fragment's centre, not the union's, which may be a gap (D12).
pub(crate) fn center(kernel: &Kernel, b: &PaintedBox) -> Option<(f32, f32)> {
    let g = kernel.fragments(kernel.node(b.id)?.key)?.first()?;
    Some((
        b.rect.0 + g.x + g.width / 2.0,
        b.rect.1 + g.y + g.height / 2.0,
    ))
}
