//! A paragraph's glyph runs (`GLYPHS`, op 9), each from one face at one
//! size, and the blur a numeric roll draws them through (`BLUR`, op 44:
//! `[44, sigma]` in local units, before the GLYPHS it blurs; LLP 1077 D15).
use super::{clip, Recorder, GLYPHS};
use crate::text::{Paragraph, RunPaint, TextEngine};
use tiny_skia::Transform;

pub(super) const BLUR: u32 = 44;

impl Recorder {
    pub(super) fn glyphs(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        // Glyphs can overhang their box a little (italics, accents), and a
        // blur spreads them three sigmas more.
        let reach = 2.0 + 3.0 * self.blur;
        let bounds = (
            origin.0 - reach,
            origin.1 - reach,
            paragraph.width + 2.0 * reach,
            paragraph.height + 2.0 * reach,
        );
        let bounds = clip::map(bounds, ts);
        if self.culled(bounds) {
            return;
        }
        self.need(bounds);
        self.transform(ts.pre_translate(origin.0, origin.1));
        for run in text.glyph_runs(paragraph, palette) {
            if run.paint.color[3] == 0 {
                continue;
            }
            let Some(file) = run.file.clone().or_else(|| self.face_file(&run.font)) else {
                continue;
            };
            let mut glyphs = run.glyphs;
            if self.blur > 0.0 {
                // A rolling glyph is its paragraph clipped to one glyph's
                // advance: the blur is not paid for the glyphs clipped away.
                let size = run.size;
                glyphs.retain(|&(_, x, y)| {
                    let near = (origin.0 + x - size * 0.5, origin.1 + y - size * 1.2);
                    !self.culled(clip::map((near.0, near.1, size * 2.0, size * 1.6), ts))
                });
                if glyphs.is_empty() {
                    continue;
                }
            }
            let font = self.font(&file, run.weight, &run.synthesis);
            if self.blur > 0.0 {
                self.ops.push(BLUR);
                self.f(self.blur);
            }
            self.ops.extend([GLYPHS, font]);
            self.f(run.size);
            self.ops.push(Self::color(run.paint.color));
            self.f(if run.synthetic_italic { -0.25 } else { 0.0 });
            self.ops.push(glyphs.len() as u32);
            for (id, x, y) in &glyphs {
                self.ops.push(*id);
                self.f(*x);
                self.f(*y);
            }
        }
    }
}
