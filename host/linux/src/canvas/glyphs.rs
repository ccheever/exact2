//! A paragraph's glyph runs (`GLYPHS`, op 9), each from one face at one
//! size, and the blur a numeric roll draws them through (`BLUR`, op 44:
//! `[44, sigma]` in local units, before the GLYPHS it blurs; LLP 1077 D15).
use super::{clip, Recorder, FONT, FONT_AXES, GLYPHS};
use crate::text::{Paragraph, RunPaint, TextEngine};
use std::sync::Arc;
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
            let optical = if run.optical_sizing {
                self.optical_size(&run.font, run.size)
            } else {
                None
            };
            let font = self.font(&file, run.weight, &run.synthesis, optical, &run.variations);
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

impl Recorder {
    /// The `opsz` a face at `size` was shaped at (`font-optical-sizing:
    /// auto`, `text::shaping::push_style`), or none for a face without the axis.
    fn optical_size(&mut self, font: &parley::FontData, size: f32) -> Option<f32> {
        let has = *self
            .optical
            .entry((font.data.id(), font.index))
            .or_insert_with(|| {
                use skrifa::MetadataProvider;
                skrifa::FontRef::from_index(font.data.data(), font.index).is_ok_and(|f| {
                    f.axes()
                        .iter()
                        .any(|a| a.tag() == skrifa::Tag::new(b"opsz"))
                })
            });
        has.then_some(size)
    }

    fn font(
        &mut self,
        file: &(Arc<str>, u32),
        weight: u16,
        synthesis: &fontique::Synthesis,
        optical: Option<f32>,
        variations: &exact_kernel::style::FontVariations,
    ) -> u32 {
        // Fontique sets at most three axes (`wdth`, `wght`, `ital` or
        // `slnt`); the shaper adds `opsz`, then `font-variation-settings`
        // (at most eight), each replacing an axis already set.
        let mut axes = [(0u32, 0u32); 12];
        let mut n = 0;
        let mut set = |tag: u32, value: f32| {
            if let Some(slot) = axes[..n].iter_mut().find(|(t, _)| *t == tag) {
                slot.1 = value.to_bits();
            } else if n < axes.len() {
                axes[n] = (tag, value.to_bits());
                n += 1;
            }
        };
        for (tag, value) in synthesis.variation_settings() {
            set(u32::from_be_bytes(tag.to_be_bytes()), *value);
        }
        if let Some(size) = optical {
            set(u32::from_be_bytes(*b"opsz"), size);
        }
        for (tag, value) in variations.axes() {
            set(*tag, *value);
        }
        let key = (file.0.clone(), file.1, weight, axes);
        if let Some(k) = self.fonts.get(&key) {
            return *k;
        }
        let k = self.fonts.len() as u32 + 1;
        self.fonts.insert(key, k);
        if n == 0 {
            self.ops.extend([FONT, k, file.1, u32::from(weight)]);
        } else {
            self.ops
                .extend([FONT_AXES, k, file.1, u32::from(weight), n as u32]);
            for (tag, value) in &axes[..n] {
                self.ops.extend([*tag, *value]);
            }
        }
        self.string(&file.0);
        k
    }
}
