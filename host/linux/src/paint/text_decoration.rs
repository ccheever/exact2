//! Solid currentcolor decorations stay paint-only, following shaped glyph advances.
use super::{Painter, Shape};
use crate::text::markup::{STRIKE, UNDERLINE};
use crate::text::{Paragraph, RunPaint};
use exact_kernel::{Kernel, PropId, TextDecorationLine, ViewId};
use tiny_skia::Transform;

impl Painter {
    pub(super) fn text_decorations(
        &mut self,
        kernel: &Kernel,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
        reveal: Option<ViewId>,
    ) {
        // CSS Text Decoration 3 §2.1: a box's lines are drawn through its
        // descendants' visible text in the box's own colour, whatever the
        // box's own visibility. Where boxes draw the same line, Chrome shows
        // the nearest box's.
        let flags: Vec<_> = palette
            .iter()
            .map(|run| {
                if !super::paints(kernel, run.source, reveal) {
                    return (None, None);
                }
                let mut current = Some(run.source);
                let (mut underline, mut strike) = (None, None);
                while let Some(id) = current {
                    let Some(node) = kernel.node(id) else {
                        break;
                    };
                    // A leaf's own colour is its run's, as presented; a
                    // Markdown paragraph's pieces take the paragraph's.
                    let color =
                        if id == run.source && node.props.str(PropId::Markup) != Some("markdown") {
                            run.color
                        } else {
                            super::rgba(node.text_color().resolve(self.dark))
                        };
                    let (under, through) = match node.style.text_decoration_line {
                        TextDecorationLine::Underline => (true, false),
                        TextDecorationLine::LineThrough => (false, true),
                        TextDecorationLine::UnderlineLineThrough => (true, true),
                        TextDecorationLine::None => (false, false),
                    };
                    if under && underline.is_none() {
                        underline = Some(color);
                    }
                    if through && strike.is_none() {
                        strike = Some(color);
                    }
                    current = node.parent;
                }
                (underline, strike)
            })
            .collect();
        if flags.iter().any(|f| f.0.is_some()) && !self.decoration_warning {
            self.decoration_warning = true;
            self.materials.1.push("CSS text-decoration: Linux uses solid font-size-based line metrics; underline text-decoration-skip-ink:auto is not implemented (LLP 1001)".into());
        }
        for (glyph, baseline, paint) in paragraph.paint_glyphs(palette) {
            // A Markdown piece's own: a followed link, `~~strike~~`.
            let mark = paragraph.runs().get(glyph.run()).map_or(0, |r| r.mark);
            let (underline, strike) = flags[glyph.run()];
            // The piece's own lines are nearest, so they show.
            let underline = if mark & UNDERLINE != 0 {
                Some(paint.color)
            } else {
                underline
            };
            let strike = if mark & STRIKE != 0 {
                Some(paint.color)
            } else {
                strike
            };
            let thickness = (glyph.font_size / 16.0).max(1.0);
            let x = origin.0 + glyph.x;
            if let Some(color) = underline {
                self.backend.fill(
                    &Shape::rect((x, origin.1 + baseline + thickness, glyph.w, thickness)),
                    color,
                    ts,
                );
            }
            if let Some(color) = strike {
                self.backend.fill(
                    &Shape::rect((
                        x,
                        origin.1 + baseline - glyph.font_size * 0.3,
                        glyph.w,
                        thickness,
                    )),
                    color,
                    ts,
                );
            }
        }
    }
}
