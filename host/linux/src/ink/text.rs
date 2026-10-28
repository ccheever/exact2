use super::*;
use exact_ink::glyph::{subpixel, Font};

impl Ink {
    pub(super) fn glyph_runs(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) -> Result<()> {
        let transform = self
            .device_transform(ts)
            .pre_scale(1. / self.scale, 1. / self.scale);
        for run in text.glyph_runs(paragraph, palette) {
            if run.paint.color[3] == 0 {
                continue;
            }
            let font = Font {
                id: [run.font.data.id(), u64::from(run.font.index)],
                data: run.font.data.data(),
                index: run.font.index,
                coords: &run.coords,
                skew: run.synthetic_italic,
            };
            for (id, x, y) in run.glyphs {
                let (x, phase) = subpixel(x.mul_add(self.scale, origin.0 * self.scale));
                let y = ((y + origin.1) * self.scale).trunc() as i32;
                let Some(glyph) = self.glyphs.get(
                    &mut self.device,
                    &font,
                    id as u16,
                    run.size * self.scale,
                    phase,
                    self.text_sigma * self.scale,
                )?
                else {
                    continue;
                };
                let mut i = Instance::new(
                    Kind::Glyph,
                    Brush::Solid,
                    [
                        (x + glyph.left) as f32,
                        (y - glyph.top) as f32,
                        glyph.size[0] as f32,
                        glyph.size[1] as f32,
                    ],
                );
                i.xform = affine(transform);
                i.uv = glyph.entry.uv;
                i.color = color(run.paint.color);
                let mut b = Bindings::default();
                if glyph.color {
                    i.flags |= flags::COLOR_GLYPH;
                    b.color = glyph.entry.texture;
                } else {
                    b.glyph = glyph.entry.texture;
                }
                self.emit(i, b)?;
            }
        }
        Ok(())
    }
}
