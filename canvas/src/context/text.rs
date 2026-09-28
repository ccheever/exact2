//! The recorder's text members (LLP 1056 D8, §3 stage 2): the text
//! attributes, `fillText`, `strokeText` and `measureText`. Measurement is
//! the environment's engine, on this thread; alignment, baselines and
//! `maxWidth` are resolved here, so the list carries a run's left end on its
//! alphabetic baseline and its squeeze.

use super::{Context2d, DomException, Inner};
use crate::font::{self, Estimate, Font, RawMetrics, TextMetrics, TextRun};
use crate::list::{text_operands, Op};
use std::sync::Arc;

/// `letterSpacing`/`wordSpacing` in px: a CSS length with a unit (Chrome
/// refuses a bare `0` and percentages); `em` is the current font's size.
fn spacing(v: &str, font_size: f64) -> Option<(String, f64)> {
    let t = v.trim().to_ascii_lowercase();
    let (n, unit) = t
        .find(|c: char| c.is_ascii_alphabetic())
        .map(|i| t.split_at(i))?;
    let px = if unit == "em" {
        exact_num::parse_f64(n).ok().map(|v| v * font_size)
    } else {
        font::length_px(&t, false)
    }?;
    if !px.is_finite() || n.is_empty() {
        return None;
    }
    let value = exact_num::parse_f64(n).ok()?;
    Some((format!("{}{unit}", font::js_number(value)), px))
}

impl Inner {
    fn rtl(&self) -> bool {
        match self.state.text.direction {
            0 => false,
            1 => true,
            _ => self.env.rtl,
        }
    }

    fn measure_raw(&self, text: &str) -> RawMetrics {
        let t = &self.state.text;
        let run = TextRun {
            font: &t.font,
            text,
            rtl: self.rtl(),
            letter_spacing: t.letter_spacing.1,
            word_spacing: t.word_spacing.1,
            kerning: t.kerning,
        };
        match &self.env.text {
            Some(engine) => engine.measure(&run),
            None => font::TextEngine::measure(&Estimate, &run),
        }
    }

    /// The `Font` record for the current text attributes, sent when the
    /// replayer's differs.
    fn send_font(&mut self) {
        let t = &self.state.text;
        let f = &t.font;
        let mut rec = vec![
            f.size,
            f.weight as f64,
            f.style as f64,
            f.stretch,
            f.caps as f64,
            t.kerning as f64,
            t.rendering as f64,
            t.letter_spacing.1,
            t.word_spacing.1,
        ];
        text_operands(&f.family_list(), &mut rec);
        if self.state.font_sent.as_deref() != Some(&rec[..]) {
            self.op(Op::Font, &rec);
            self.state.font_sent = Some(Arc::from(rec));
        }
    }

    fn draw_text(&mut self, op: Op, text: &str, x: f64, y: f64, max_width: Option<f64>) {
        if !(x.is_finite() && y.is_finite()) || !self.paints() {
            return;
        }
        if let Some(m) = max_width {
            if m.is_nan() || m <= 0.0 {
                return;
            }
        }
        let text = font::prepare(text);
        let raw = self.measure_raw(&text);
        let squeeze = max_width.filter(|m| *m < raw.width);
        let (width, scale) = match squeeze {
            Some(m) => (m, if raw.width > 0.0 { m / raw.width } else { 0.0 }),
            None => (raw.width, 1.0),
        };
        let rtl = self.rtl();
        let t = &self.state.text;
        let ox = x - width * font::align_fraction(t.align, rtl);
        let oy = y + font::baseline_shift(&raw, t.baseline);
        self.send_font();
        let mut rec = vec![ox, oy, scale, rtl as u8 as f64];
        text_operands(&text, &mut rec);
        self.op(op, &rec);
    }
}

impl Context2d {
    /// `font = v`: an unparseable value is ignored. It also sets
    /// `fontStretch` and `fontVariantCaps`, as the shorthand does.
    pub fn set_font(&self, v: &str) {
        if let Some(f) = font::parse(v) {
            self.g().state.text.font = f;
        }
    }

    /// `font`, as Chrome serialises it.
    pub fn font(&self) -> String {
        self.g().state.text.font.serialize()
    }

    /// The current font, parsed.
    pub fn font_value(&self) -> Font {
        self.g().state.text.font.clone()
    }

    fn set_enum(&self, v: &str, names: &[&str], slot: fn(&mut Inner) -> &mut u8) {
        if let Some(k) = names.iter().position(|n| *n == v) {
            *slot(&mut self.g()) = k as u8;
        }
    }

    /// `textAlign = v`.
    pub fn set_text_align(&self, v: &str) {
        self.set_enum(v, &font::ALIGN, |g| &mut g.state.text.align)
    }

    /// `textAlign`.
    pub fn text_align(&self) -> String {
        font::ALIGN[self.g().state.text.align as usize].into()
    }

    /// `textBaseline = v`.
    pub fn set_text_baseline(&self, v: &str) {
        self.set_enum(v, &font::BASELINE, |g| &mut g.state.text.baseline)
    }

    /// `textBaseline`.
    pub fn text_baseline(&self) -> String {
        font::BASELINE[self.g().state.text.baseline as usize].into()
    }

    /// `direction = v`.
    pub fn set_direction(&self, v: &str) {
        self.set_enum(v, &font::DIRECTION, |g| &mut g.state.text.direction)
    }

    /// `direction`: `inherit` reads as the canvas node's.
    pub fn direction(&self) -> String {
        let g = self.g();
        font::DIRECTION[if g.rtl() { 1 } else { 0 }].into()
    }

    /// `fontKerning = v`.
    pub fn set_font_kerning(&self, v: &str) {
        self.set_enum(v, &font::KERNING, |g| &mut g.state.text.kerning)
    }

    /// `fontKerning`.
    pub fn font_kerning(&self) -> String {
        font::KERNING[self.g().state.text.kerning as usize].into()
    }

    /// `textRendering = v`.
    pub fn set_text_rendering(&self, v: &str) {
        self.set_enum(v, &font::RENDERING, |g| &mut g.state.text.rendering)
    }

    /// `textRendering`.
    pub fn text_rendering(&self) -> String {
        font::RENDERING[self.g().state.text.rendering as usize].into()
    }

    /// `fontVariantCaps = v`.
    pub fn set_font_variant_caps(&self, v: &str) {
        self.set_enum(v, &font::CAPS, |g| &mut g.state.text.font.caps)
    }

    /// `fontVariantCaps`.
    pub fn font_variant_caps(&self) -> String {
        font::CAPS[self.g().state.text.font.caps as usize].into()
    }

    /// `fontStretch = v`: a keyword; anything else ignored.
    pub fn set_font_stretch(&self, v: &str) {
        if let Some((_, pct)) = font::STRETCH.iter().find(|(k, _)| *k == v) {
            self.g().state.text.font.stretch = *pct;
        }
    }

    /// `fontStretch`.
    pub fn font_stretch(&self) -> String {
        let s = self.g().state.text.font.stretch;
        font::STRETCH
            .iter()
            .find(|(_, p)| *p == s)
            .map_or("normal", |(k, _)| k)
            .into()
    }

    /// `letterSpacing = v`: a CSS length; anything else ignored.
    pub fn set_letter_spacing(&self, v: &str) {
        let mut g = self.g();
        if let Some(s) = spacing(v, g.state.text.font.size) {
            g.state.text.letter_spacing = s;
        }
    }

    /// `letterSpacing`.
    pub fn letter_spacing(&self) -> String {
        self.g().state.text.letter_spacing.0.clone()
    }

    /// `wordSpacing = v`: a CSS length; anything else ignored.
    pub fn set_word_spacing(&self, v: &str) {
        let mut g = self.g();
        if let Some(s) = spacing(v, g.state.text.font.size) {
            g.state.text.word_spacing = s;
        }
    }

    /// `wordSpacing`.
    pub fn word_spacing(&self) -> String {
        self.g().state.text.word_spacing.0.clone()
    }

    /// `fillText(text, x, y)`.
    pub fn fill_text(&self, text: &str, x: f64, y: f64) -> Result<(), DomException> {
        self.g().draw_text(Op::FillText, text, x, y, None);
        Ok(())
    }

    /// `fillText(text, x, y, maxWidth)`.
    pub fn fill_text_with_max_width(
        &self,
        text: &str,
        x: f64,
        y: f64,
        max_width: f64,
    ) -> Result<(), DomException> {
        self.g()
            .draw_text(Op::FillText, text, x, y, Some(max_width));
        Ok(())
    }

    /// `strokeText(text, x, y)`.
    pub fn stroke_text(&self, text: &str, x: f64, y: f64) -> Result<(), DomException> {
        self.g().draw_text(Op::StrokeText, text, x, y, None);
        Ok(())
    }

    /// `strokeText(text, x, y, maxWidth)`.
    pub fn stroke_text_with_max_width(
        &self,
        text: &str,
        x: f64,
        y: f64,
        max_width: f64,
    ) -> Result<(), DomException> {
        self.g()
            .draw_text(Op::StrokeText, text, x, y, Some(max_width));
        Ok(())
    }

    /// `measureText(text)`: measured by the host's engine on this thread, in
    /// CSS px, relative to the alignment point and baseline, never
    /// multiplied by the drawing matrix (LLP 1056 D8).
    pub fn measure_text(&self, text: &str) -> Result<TextMetrics, DomException> {
        let g = self.g();
        let text = font::prepare(text);
        let raw = g.measure_raw(&text);
        let t = &g.state.text;
        Ok(font::metrics(&raw, t.align, t.baseline, g.rtl()))
    }
}
