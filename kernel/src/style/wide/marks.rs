//! What holds a wide colour: each type that keeps `ColorValue`s visits
//! them into a [`WideMark`], for the sweep (`reclaim`). A type that comes to
//! hold a colour needs its visit here, or the sweep frees what it holds; a
//! stale read then says so (debug builds assert).

use super::WideMark;
use crate::generated::{StyleCodec, StyleId, StyleMask, StyleProps};
use crate::gradient::{BackgroundImage, Gradient};
use crate::style::symbols::SymbolPalette;
use crate::style::{BoxShadows, TextShadow};
use crate::style::{ColorValue, RowValue};
use crate::svg::filter::{FilterFn, FilterList};
use crate::svg::scene::ShapePaint;
use crate::svg::{Item, Kind, Scene};
use crate::svg::{Paint, PaintFallback};
use std::sync::LazyLock;

/// Visit each colour held.
pub trait Colors {
    /// Mark each `ColorValue` this holds.
    fn colors(&self, m: &mut WideMark);
}

impl Colors for ColorValue {
    fn colors(&self, m: &mut WideMark) {
        m.color(*self);
    }
}

impl<T: Colors> Colors for Option<T> {
    fn colors(&self, m: &mut WideMark) {
        if let Some(c) = self {
            c.colors(m);
        }
    }
}

impl<T: Colors> Colors for [T] {
    fn colors(&self, m: &mut WideMark) {
        for c in self {
            c.colors(m);
        }
    }
}

impl Colors for Paint {
    fn colors(&self, m: &mut WideMark) {
        match self {
            Paint::Color(c) | Paint::Url(_, PaintFallback::Color(c)) => m.color(*c),
            _ => {}
        }
    }
}

impl Colors for BoxShadows {
    fn colors(&self, m: &mut WideMark) {
        for s in &self.0 {
            m.color(s.color);
        }
    }
}

impl Colors for TextShadow {
    fn colors(&self, m: &mut WideMark) {
        if let Some(s) = self.shadow() {
            s.color.colors(m);
        }
    }
}

impl Colors for Gradient {
    fn colors(&self, m: &mut WideMark) {
        for s in &self.stops {
            m.color(s.color);
        }
    }
}

impl Colors for BackgroundImage {
    fn colors(&self, m: &mut WideMark) {
        self.layers().colors(m);
    }
}

impl Colors for FilterList {
    fn colors(&self, m: &mut WideMark) {
        for f in &self.0 {
            if let FilterFn::DropShadow(.., Some(c)) = f {
                m.color(*c);
            }
        }
    }
}

impl Colors for SymbolPalette {
    fn colors(&self, m: &mut WideMark) {
        self.0.colors(m);
    }
}

impl Colors for RowValue<'_> {
    fn colors(&self, m: &mut WideMark) {
        match self {
            RowValue::ColorValue(c) => m.color(*c),
            RowValue::Paint(p) => p.colors(m),
            RowValue::BoxShadow(s) => s.colors(m),
            RowValue::TextShadow(s) => s.colors(m),
            RowValue::BackgroundImage(i) | RowValue::MaskImage(i) => i.colors(m),
            RowValue::Filter(f) => f.colors(m),
            RowValue::SymbolPalette(p) => p.colors(m),
            _ => {}
        }
    }
}

/// The rows whose values can hold a colour.
static COLOURED: LazyLock<Vec<StyleId>> = LazyLock::new(|| {
    StyleId::ALL
        .into_iter()
        .filter(|id| {
            matches!(
                id.codec(),
                StyleCodec::ColorValue
                    | StyleCodec::KeywordColor
                    | StyleCodec::Paint
                    | StyleCodec::Filter
                    | StyleCodec::BackgroundImage
                    | StyleCodec::BoxShadow
                    | StyleCodec::TextShadow
                    | StyleCodec::MaskImage
                    | StyleCodec::SymbolPalette
            )
        })
        .collect()
});

/// The rows whose values can hold a colour, as a mask: what a copy that
/// lays out and never paints (the Linux host's lookahead) clears.
pub fn coloured() -> StyleMask {
    let mut mask = StyleMask::EMPTY;
    for &id in COLOURED.iter() {
        mask.set(id);
    }
    mask
}

impl Colors for StyleProps {
    fn colors(&self, m: &mut WideMark) {
        for &id in COLOURED.iter() {
            self.get(id).colors(m);
        }
    }
}

impl Colors for crate::wire::Op {
    fn colors(&self, m: &mut WideMark) {
        if let crate::wire::Op::SetStyle { patch, .. } = self {
            patch.colors(m);
        }
    }
}

impl Colors for ShapePaint {
    fn colors(&self, m: &mut WideMark) {
        m.color(self.color);
        if let Some(server) = &self.server {
            for s in &server.stops {
                m.color(s.color);
            }
        }
        if let Some(pattern) = &self.pattern {
            pattern.items.colors(m);
        }
    }
}

impl Colors for Item {
    fn colors(&self, m: &mut WideMark) {
        if let Some(mask) = &self.mask {
            mask.items.colors(m);
        }
        match &self.kind {
            Kind::Group(items)
            | Kind::Viewport {
                children: items, ..
            } => items.colors(m),
            Kind::Shape(s) => {
                s.fill.colors(m);
                s.stroke.colors(m);
                s.markers.colors(m);
            }
            Kind::Text(t) => {
                for chunk in &t.chunks {
                    for run in &chunk.runs {
                        run.fill.colors(m);
                        run.stroke.colors(m);
                    }
                }
            }
        }
    }
}

impl Colors for Scene {
    fn colors(&self, m: &mut WideMark) {
        self.items.colors(m);
    }
}

/// Replace each wide colour held with its 8-bit colour, or 8-bit
/// `light-dark()` pair (its sRGB clip, what a painting host resolves it to
/// anyway): what a host keeps of paint it retains and paints as 8-bit
/// colours (an svg scene), so it holds no id and still flips scheme (LLP
/// 1121 stage 4). Mirrors [`Colors`]; after it, a visit marks nothing. A
/// gradient's ramp needs its stops' exact colours, so it keeps their
/// [`Values`](super::Values) instead.
pub trait Settle {
    /// Settle each wide `ColorValue` this holds.
    fn settle(&mut self);
}

impl Settle for ColorValue {
    fn settle(&mut self) {
        if let ColorValue::Wide(_) = self {
            *self = self.fallback();
        }
    }
}

impl<T: Settle> Settle for Option<T> {
    fn settle(&mut self) {
        if let Some(c) = self {
            c.settle();
        }
    }
}

impl<T: Settle> Settle for [T] {
    fn settle(&mut self) {
        for c in self {
            c.settle();
        }
    }
}

impl Settle for Paint {
    fn settle(&mut self) {
        match self {
            Paint::Color(c) | Paint::Url(_, PaintFallback::Color(c)) => c.settle(),
            _ => {}
        }
    }
}

impl Settle for ShapePaint {
    fn settle(&mut self) {
        self.color.settle();
        if let Some(server) = &mut self.server {
            for s in &mut server.stops {
                s.color.settle();
            }
        }
        if let Some(pattern) = &mut self.pattern {
            pattern.items.settle();
        }
    }
}

impl Settle for Item {
    fn settle(&mut self) {
        if let Some(mask) = &mut self.mask {
            mask.items.settle();
        }
        match &mut self.kind {
            Kind::Group(items)
            | Kind::Viewport {
                children: items, ..
            } => items.settle(),
            Kind::Shape(s) => {
                s.fill.settle();
                s.stroke.settle();
                s.markers.settle();
            }
            Kind::Text(t) => {
                for chunk in &mut t.chunks {
                    for run in &mut chunk.runs {
                        run.fill.settle();
                        run.stroke.settle();
                    }
                }
            }
        }
    }
}

impl Settle for Scene {
    fn settle(&mut self) {
        self.items.settle();
    }
}
