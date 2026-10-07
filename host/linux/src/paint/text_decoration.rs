//! Solid currentcolor decorations stay paint-only, following shaped glyph advances.
use super::{Painter, Shape};
use crate::text::markup::{STRIKE, UNDERLINE};
use crate::text::{Paragraph, RunPaint};
use exact_kernel::{Kernel, TextDecorationLine, ViewId};
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
        let flags: Vec<_> = palette
            .iter()
            .map(|run| {
                let mut current = Some(run.source);
                let (mut underline, mut strike) = (false, false);
                while let Some(id) = current {
                    let Some(node) = kernel.node(id) else {
                        break;
                    };
                    // A hidden element's decoration is its own paint.
                    if !super::paints(kernel, id, reveal) {
                        current = node.parent;
                        continue;
                    }
                    match node.style.text_decoration_line {
                        TextDecorationLine::Underline => underline = true,
                        TextDecorationLine::LineThrough => strike = true,
                        TextDecorationLine::UnderlineLineThrough => {
                            underline = true;
                            strike = true;
                        }
                        TextDecorationLine::None => {}
                    }
                    current = node.parent;
                }
                (underline, strike)
            })
            .collect();
        if flags.iter().any(|f| f.0) && !self.decoration_warning {
            self.decoration_warning = true;
            self.materials.1.push("CSS text-decoration: Linux uses solid font-size-based line metrics; underline text-decoration-skip-ink:auto is not implemented (LLP 1001)".into());
        }
        for (glyph, baseline, paint) in paragraph.paint_glyphs(palette) {
            // A Markdown piece's own: a followed link, `~~strike~~`.
            let mark = paragraph.runs().get(glyph.run()).map_or(0, |r| r.mark);
            let (underline, strike) = flags[glyph.run()];
            let (underline, strike) = (
                underline || mark & UNDERLINE != 0,
                strike || mark & STRIKE != 0,
            );
            let thickness = (glyph.font_size / 16.0).max(1.0);
            let x = origin.0 + glyph.x;
            if underline {
                self.backend.fill(
                    &Shape::rect((x, origin.1 + baseline + thickness, glyph.w, thickness)),
                    paint.color,
                    ts,
                );
            }
            if strike {
                self.backend.fill(
                    &Shape::rect((
                        x,
                        origin.1 + baseline - glyph.font_size * 0.3,
                        glyph.w,
                        thickness,
                    )),
                    paint.color,
                    ts,
                );
            }
        }
    }
}
